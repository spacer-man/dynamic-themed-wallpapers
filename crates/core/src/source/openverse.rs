//! Openverse: a search engine over openly licensed images (Flickr, museums, Wikimedia…).
//! No key. Anonymous use is rate limited hard, so one refill costs one request.
//!
//! Images are hosted by the original providers, so the wallpaper download goes to whatever
//! host Openverse names (https only). Licence and author come with every result.

use serde::Deserialize;

use super::{Candidate, CandidateRequest, Curation, License, Search, Source, SourceKind};
use crate::error::{Error, Result};
use crate::http::{build_url, HttpClient, MAX_JSON_BYTES};
use crate::text::truncate;

const API: &str = "https://api.openverse.org/v1/images/";

pub struct OpenverseSource<'a> {
    http: &'a dyn HttpClient,
}

impl<'a> OpenverseSource<'a> {
    pub fn new(http: &'a dyn HttpClient) -> Self {
        Self { http }
    }
}

impl Source for OpenverseSource<'_> {
    fn kind(&self) -> SourceKind {
        SourceKind::Openverse
    }

    fn candidates(&self, search: &Search<'_>, req: &CandidateRequest) -> Result<Vec<Candidate>> {
        let label = search.text();
        let page = (req.seed % 3 + 1).to_string();
        let url = build_url(
            API,
            &[
                ("q", label),
                // Licences that allow cropping (no "NoDerivatives").
                ("license_type", "modification"),
                (
                    "aspect_ratio",
                    if req.orientation() == "portrait" {
                        "tall"
                    } else {
                        "wide"
                    },
                ),
                ("size", "large"),
                ("category", "photograph"),
                ("extension", "jpg"),
                ("mature", "false"),
                ("page_size", "20"),
                ("page", &page),
            ],
        );
        let body = self.http.get(&url, MAX_JSON_BYTES)?;
        let resp: Response =
            serde_json::from_slice(&body).map_err(|e| Error::parse("openverse search", e))?;
        let mut out: Vec<Candidate> = resp
            .results
            .into_iter()
            .filter_map(|r| r.into_candidate(label, req))
            .collect();
        super::dedup(&mut out);
        out.truncate(req.limit as usize);
        Ok(out)
    }

    fn check(&self) -> Result<()> {
        let url = build_url(API, &[("q", "sky"), ("page_size", "1")]);
        self.http.get_quick(&url, &[], MAX_JSON_BYTES).map(|_| ())
    }

    fn sized_url(&self, c: &Candidate, _min_width: u32, _min_height: u32) -> String {
        c.image_url.clone()
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    results: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    id: String,
    title: Option<String>,
    url: Option<String>,
    thumbnail: Option<String>,
    foreign_landing_url: Option<String>,
    creator: Option<String>,
    creator_url: Option<String>,
    license: Option<String>,
    license_version: Option<String>,
    license_url: Option<String>,
    filetype: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

/// "by-sa" + "4.0" → "CC BY-SA 4.0"; "cc0" → "CC0 1.0"; "pdm" → "Public Domain Mark 1.0".
fn license_name(code: &str, version: Option<&str>) -> String {
    let v = |default: &str| {
        version
            .filter(|v| !v.is_empty())
            .unwrap_or(default)
            .to_owned()
    };
    match code.to_ascii_lowercase().as_str() {
        "cc0" => format!("CC0 {}", v("1.0")),
        "pdm" => format!("Public Domain Mark {}", v("1.0")),
        other => match version.filter(|v| !v.is_empty()) {
            Some(ver) => format!("CC {} {ver}", other.to_ascii_uppercase()),
            None => format!("CC {}", other.to_ascii_uppercase()),
        },
    }
}

impl Item {
    fn into_candidate(self, label: &str, req: &CandidateRequest) -> Option<Candidate> {
        let image_url = self.url.filter(|u| u.starts_with("https://"))?;
        let (width, height) = (self.width?, self.height?);
        if !req.covers_screen(width, height) {
            return None;
        }
        if let Some(t) = &self.filetype {
            if !["jpg", "jpeg", "png"].contains(&t.to_ascii_lowercase().as_str()) {
                return None;
            }
        }
        let code = self.license.filter(|l| !l.is_empty())?;
        let attribution_required = !matches!(code.to_ascii_lowercase().as_str(), "cc0" | "pdm");
        let title = self
            .title
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| label.to_owned());
        let author = self
            .creator
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| "Unknown".into());
        let preview_url = self
            .thumbnail
            .filter(|t| t.starts_with("https://"))
            .unwrap_or_else(|| image_url.clone());
        Some(Candidate {
            source: SourceKind::Openverse,
            id: format!("openverse:{}", self.id),
            title: truncate(title.trim(), 120),
            description: None,
            page_url: self
                .foreign_landing_url
                .unwrap_or_else(|| format!("https://openverse.org/image/{}", self.id)),
            image_url,
            preview_url,
            width,
            height,
            author: truncate(author.trim(), 80),
            author_url: self.creator_url.filter(|u| u.starts_with("https://")),
            license: License {
                short_name: license_name(&code, self.license_version.as_deref()),
                url: self.license_url,
                attribution_required,
            },
            curation: Curation::default(),
            coords: None,
            place_label: label.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn licence_names() {
        assert_eq!(license_name("by-sa", Some("4.0")), "CC BY-SA 4.0");
        assert_eq!(license_name("cc0", Some("1.0")), "CC0 1.0");
        assert_eq!(license_name("pdm", None), "Public Domain Mark 1.0");
        assert_eq!(license_name("by", None), "CC BY");
    }
}
