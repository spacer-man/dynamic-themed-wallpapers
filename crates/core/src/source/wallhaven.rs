//! Wallhaven: community wallpapers, no key. **SFW only and General category only** — the
//! `purity` and `categories` parameters are constants on purpose, not settings.
//!
//! The search API names neither author nor licence (the uploader is only in the per-wallpaper
//! endpoint), so candidates credit Wallhaven itself and say that no licence is stated. Only
//! the photo that becomes the wallpaper costs one extra request ([`Source::enrich_chosen`]) to
//! learn its uploader and source link. Likes on Wallhaven (favourites) feed the curation score.

use serde::Deserialize;

use super::{Candidate, CandidateRequest, Curation, License, Search, Source, SourceKind};
use crate::error::{Error, Result};
use crate::http::{build_url, HttpClient, MAX_JSON_BYTES};
use crate::text::truncate;

const API: &str = "https://wallhaven.cc/api/v1/search";
const DETAIL_API: &str = "https://wallhaven.cc/api/v1/w";
/// 100 = General only (no Anime, no People).
const CATEGORIES: &str = "100";
/// 100 = SFW only. Never configurable.
const PURITY: &str = "100";

pub struct WallhavenSource<'a> {
    http: &'a dyn HttpClient,
}

impl<'a> WallhavenSource<'a> {
    pub fn new(http: &'a dyn HttpClient) -> Self {
        Self { http }
    }
}

impl Source for WallhavenSource<'_> {
    fn kind(&self) -> SourceKind {
        SourceKind::Wallhaven
    }

    fn candidates(&self, search: &Search<'_>, req: &CandidateRequest) -> Result<Vec<Candidate>> {
        let label = search.text();
        let (min_w, min_h) = req.min_size();
        let atleast = format!("{min_w}x{min_h}");
        // With `sorting=random` the seed keeps pages consistent; a new seed is a new shuffle.
        let seed = format!("{:06x}", req.seed & 0xff_ffff);
        let url = build_url(
            API,
            &[
                ("q", label),
                ("categories", CATEGORIES),
                ("purity", PURITY),
                ("atleast", &atleast),
                ("ratios", req.orientation()),
                ("sorting", "random"),
                ("seed", &seed),
            ],
        );
        let body = self.http.get(&url, MAX_JSON_BYTES)?;
        let resp: Response =
            serde_json::from_slice(&body).map_err(|e| Error::parse("wallhaven search", e))?;
        let mut out: Vec<Candidate> = resp
            .data
            .into_iter()
            .filter_map(|w| w.into_candidate(label, req))
            .collect();
        super::dedup(&mut out);
        out.truncate(req.limit as usize);
        Ok(out)
    }

    fn check(&self) -> Result<()> {
        let url = build_url(
            API,
            &[("q", "sky"), ("categories", CATEGORIES), ("purity", PURITY)],
        );
        self.http.get_quick(&url, &[], MAX_JSON_BYTES).map(|_| ())
    }

    fn sized_url(&self, c: &Candidate, _min_width: u32, _min_height: u32) -> String {
        c.image_url.clone()
    }

    fn enrich_chosen(&self, c: &Candidate) -> Result<Option<Candidate>> {
        let id = c.id.strip_prefix("wallhaven:").unwrap_or(&c.id);
        let url = build_url(&format!("{DETAIL_API}/{id}"), &[]);
        let body = self.http.get(&url, MAX_JSON_BYTES)?;
        let detail: DetailResponse =
            serde_json::from_slice(&body).map_err(|e| Error::parse("wallhaven wallpaper", e))?;
        let mut out = c.clone();
        let mut changed = false;
        if let Some(name) = detail
            .data
            .uploader
            .map(|u| u.username.trim().to_owned())
            .filter(|n| !n.is_empty())
        {
            out.author_url = Some(format!(
                "https://wallhaven.cc/user/{}",
                encode_segment(&name)
            ));
            out.author = truncate(&name, 80);
            changed = true;
        }
        if let Some(src) = detail
            .data
            .source
            .map(|s| s.trim().to_owned())
            .filter(|s| s.starts_with("https://") || s.starts_with("http://"))
        {
            out.page_url = src;
            changed = true;
        }
        Ok(changed.then_some(out))
    }
}

#[derive(Deserialize)]
struct DetailResponse {
    data: Detail,
}

#[derive(Deserialize)]
struct Detail {
    uploader: Option<Uploader>,
    source: Option<String>,
}

#[derive(Deserialize)]
struct Uploader {
    #[serde(default)]
    username: String,
}

/// Percent-encode everything but unreserved characters (user names are plain, but be safe).
fn encode_segment(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    data: Vec<Wallpaper>,
}

#[derive(Deserialize)]
struct Wallpaper {
    id: String,
    url: Option<String>,
    path: String,
    #[serde(default)]
    favorites: u32,
    dimension_x: u32,
    dimension_y: u32,
    file_type: Option<String>,
    #[serde(default)]
    thumbs: Thumbs,
}

#[derive(Deserialize, Default)]
struct Thumbs {
    large: Option<String>,
    original: Option<String>,
}

impl Wallpaper {
    fn into_candidate(self, label: &str, req: &CandidateRequest) -> Option<Candidate> {
        if !req.covers_screen(self.dimension_x, self.dimension_y)
            || !self.path.starts_with("https://")
        {
            return None;
        }
        if let Some(t) = &self.file_type {
            if t != "image/jpeg" && t != "image/png" {
                return None;
            }
        }
        // `original` is the biggest thumbnail Wallhaven offers; `large` is only ~300 px.
        let preview_url = self
            .thumbs
            .original
            .or(self.thumbs.large)
            .filter(|u| u.starts_with("https://"))
            .unwrap_or_else(|| self.path.clone());
        let page_url = self
            .url
            .unwrap_or_else(|| format!("https://wallhaven.cc/w/{}", self.id));
        Some(Candidate {
            source: SourceKind::Wallhaven,
            id: format!("wallhaven:{}", self.id),
            title: label.to_owned(),
            description: None,
            page_url,
            image_url: self.path,
            preview_url,
            width: self.dimension_x,
            height: self.dimension_y,
            author: "Wallhaven".into(),
            author_url: None,
            license: License {
                short_name: "No licence stated".into(),
                url: None,
                attribution_required: false,
            },
            curation: Curation {
                likes: Some(self.favorites),
                ..Default::default()
            },
            coords: None,
            place_label: label.to_owned(),
        })
    }
}
