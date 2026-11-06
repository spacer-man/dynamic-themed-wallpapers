//! Pixabay: free images, user's own API key passed as `key=` in the query string.
//!
//! Pixabay forbids permanent hotlinking; this app downloads and caches the picture anyway and
//! never shows a Pixabay URL as an image source. The key travels in the URL, so every error
//! that could carry the URL is scrubbed before it leaves this module.
//!
//! Full-size URLs (`imageURL`, `fullHDURL`) exist only for accounts with full API access; the
//! default `largeImageURL` is at most 1280 px on its long side. Candidates carry the size of
//! the best rendition we can actually fetch, so the minimum-size filter stays honest.

use serde::Deserialize;

use super::{Candidate, CandidateRequest, Curation, License, Search, Source, SourceKind};
use crate::error::{Error, Result};
use crate::http::{build_url, HttpClient, MAX_JSON_BYTES};
use crate::text::truncate;

const API: &str = "https://pixabay.com/api/";
const LARGE_LONG_SIDE: u32 = 1280;
const FULL_HD_LONG_SIDE: u32 = 1920;

pub struct PixabaySource<'a> {
    http: &'a dyn HttpClient,
    key: String,
}

impl<'a> PixabaySource<'a> {
    pub fn new(http: &'a dyn HttpClient, key: &str) -> Result<Self> {
        Ok(Self {
            http,
            key: SourceKind::Pixabay.validate_key(key)?,
        })
    }

    /// Remove the key from anything that could echo the request URL.
    fn scrub(&self, e: Error) -> Error {
        match e {
            Error::Status { status, .. } => Error::Status {
                status,
                url: API.into(),
            },
            Error::Network(m) => Error::Network(m.replace(&self.key, "***")),
            other => other,
        }
    }

    fn get(&self, url: &str, quick: bool) -> Result<Vec<u8>> {
        let r = if quick {
            self.http.get_quick(url, &[], MAX_JSON_BYTES)
        } else {
            self.http.get(url, MAX_JSON_BYTES)
        };
        r.map_err(|e| self.scrub(e))
    }
}

impl Source for PixabaySource<'_> {
    fn kind(&self) -> SourceKind {
        SourceKind::Pixabay
    }

    fn candidates(&self, search: &Search<'_>, req: &CandidateRequest) -> Result<Vec<Candidate>> {
        // Pixabay accepts at most 100 characters of query.
        let label = search.text();
        let q: String = label.chars().take(100).collect();
        let page = (req.seed % 3 + 1).to_string();
        let per_page = req.limit.clamp(3, 60).to_string();
        let (min_w, min_h) = (req.screen_width.to_string(), req.screen_height.to_string());
        let url = build_url(
            API,
            &[
                ("key", &self.key),
                ("q", &q),
                ("image_type", "photo"),
                (
                    "orientation",
                    if req.orientation() == "portrait" {
                        "vertical"
                    } else {
                        "horizontal"
                    },
                ),
                ("min_width", &min_w),
                ("min_height", &min_h),
                ("safesearch", "true"),
                ("per_page", &per_page),
                ("page", &page),
            ],
        );
        let body = self.get(&url, false)?;
        let resp: Response =
            serde_json::from_slice(&body).map_err(|e| Error::parse("pixabay search", e))?;
        let mut out: Vec<Candidate> = resp
            .hits
            .into_iter()
            .filter_map(|h| h.into_candidate(label, req))
            .collect();
        super::dedup(&mut out);
        out.truncate(req.limit as usize);
        Ok(out)
    }

    fn check(&self) -> Result<()> {
        let url = build_url(API, &[("key", &self.key), ("q", "sky"), ("per_page", "3")]);
        self.get(&url, true).map(|_| ())
    }

    fn sized_url(&self, c: &Candidate, _min_width: u32, _min_height: u32) -> String {
        c.image_url.clone()
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    hits: Vec<Hit>,
}

#[derive(Deserialize)]
struct Hit {
    id: u64,
    #[serde(rename = "pageURL")]
    page_url: String,
    #[serde(default)]
    tags: String,
    #[serde(rename = "largeImageURL")]
    large_image_url: Option<String>,
    #[serde(rename = "fullHDURL")]
    full_hd_url: Option<String>,
    #[serde(rename = "imageURL")]
    image_url: Option<String>,
    #[serde(rename = "imageWidth")]
    image_width: u32,
    #[serde(rename = "imageHeight")]
    image_height: u32,
    #[serde(default)]
    likes: u32,
    #[serde(default)]
    user: String,
    #[serde(default)]
    user_id: u64,
}

/// Size after scaling so the long side is at most `long`.
fn fit_long_side(w: u32, h: u32, long: u32) -> (u32, u32) {
    let m = w.max(h);
    if m <= long {
        return (w, h);
    }
    let k = long as f32 / m as f32;
    ((w as f32 * k).round() as u32, (h as f32 * k).round() as u32)
}

impl Hit {
    fn into_candidate(self, label: &str, req: &CandidateRequest) -> Option<Candidate> {
        let https = |u: &Option<String>| u.clone().filter(|u| u.starts_with("https://"));
        // Largest rendition first; its real size is what counts.
        let (image_url, (width, height)) = if let Some(u) = https(&self.image_url) {
            (u, (self.image_width, self.image_height))
        } else if let Some(u) = https(&self.full_hd_url) {
            (
                u,
                fit_long_side(self.image_width, self.image_height, FULL_HD_LONG_SIDE),
            )
        } else {
            (
                https(&self.large_image_url)?,
                fit_long_side(self.image_width, self.image_height, LARGE_LONG_SIDE),
            )
        };
        if !req.covers_screen(width, height) {
            return None;
        }
        let preview_url = https(&self.large_image_url).unwrap_or_else(|| image_url.clone());
        let tags = self.tags.trim();
        let title = if tags.is_empty() { label } else { tags };
        let author = if self.user.is_empty() {
            "Unknown".to_owned()
        } else {
            self.user.clone()
        };
        let plain_name = self
            .user
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        let author_url = (!self.user.is_empty() && plain_name && self.user_id != 0)
            .then(|| format!("https://pixabay.com/users/{}-{}/", self.user, self.user_id));
        Some(Candidate {
            source: SourceKind::Pixabay,
            id: format!("pixabay:{}", self.id),
            title: truncate(title, 120),
            description: None,
            page_url: self.page_url,
            image_url,
            preview_url,
            width,
            height,
            author: truncate(&author, 80),
            author_url,
            license: License {
                short_name: "Pixabay Content License".into(),
                url: Some("https://pixabay.com/service/license-summary/".into()),
                attribution_required: false,
            },
            curation: Curation {
                likes: Some(self.likes),
                ..Default::default()
            },
            coords: None,
            place_label: label.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_side_fit() {
        assert_eq!(fit_long_side(4000, 6000, 1280), (853, 1280));
        assert_eq!(fit_long_side(800, 1000, 1280), (800, 1000));
    }
}
