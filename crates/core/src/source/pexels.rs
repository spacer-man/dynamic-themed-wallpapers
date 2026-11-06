//! Pexels: free photos, user's own API key (`Authorization: <key>`, no prefix). The Pexels
//! licence asks for credit to Pexels and the photographer; the caption provides both.

use serde::Deserialize;

use super::{Candidate, CandidateRequest, Curation, License, Search, Source, SourceKind};
use crate::error::{Error, Result};
use crate::http::{build_url, HttpClient, MAX_JSON_BYTES};
use crate::text::truncate;

const API: &str = "https://api.pexels.com/v1/search";

pub struct PexelsSource<'a> {
    http: &'a dyn HttpClient,
    key: String,
}

impl<'a> PexelsSource<'a> {
    pub fn new(http: &'a dyn HttpClient, key: &str) -> Result<Self> {
        Ok(Self {
            http,
            key: SourceKind::Pexels.validate_key(key)?,
        })
    }
}

impl Source for PexelsSource<'_> {
    fn kind(&self) -> SourceKind {
        SourceKind::Pexels
    }

    fn candidates(&self, search: &Search<'_>, req: &CandidateRequest) -> Result<Vec<Candidate>> {
        let label = search.text();
        let page = (req.seed % 3 + 1).to_string();
        let per_page = req.limit.clamp(1, 40).to_string();
        let url = build_url(
            API,
            &[
                ("query", label),
                ("orientation", req.orientation()),
                ("size", "large"),
                ("per_page", &per_page),
                ("page", &page),
            ],
        );
        let body = self
            .http
            .get_with(&url, &[("Authorization", &self.key)], MAX_JSON_BYTES)?;
        let resp: Response =
            serde_json::from_slice(&body).map_err(|e| Error::parse("pexels search", e))?;
        let mut out: Vec<Candidate> = resp
            .photos
            .into_iter()
            .filter_map(|p| p.into_candidate(label, req))
            .collect();
        super::dedup(&mut out);
        out.truncate(req.limit as usize);
        Ok(out)
    }

    fn check(&self) -> Result<()> {
        let url = build_url(API, &[("query", "sky"), ("per_page", "1")]);
        self.http
            .get_quick(&url, &[("Authorization", &self.key)], MAX_JSON_BYTES)
            .map(|_| ())
    }

    fn sized_url(&self, c: &Candidate, _min_width: u32, _min_height: u32) -> String {
        c.image_url.clone()
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    photos: Vec<Photo>,
}

#[derive(Deserialize)]
struct Photo {
    id: u64,
    width: u32,
    height: u32,
    url: String,
    photographer: String,
    photographer_url: Option<String>,
    alt: Option<String>,
    src: Src,
}

#[derive(Deserialize)]
struct Src {
    original: String,
}

impl Photo {
    fn into_candidate(self, label: &str, req: &CandidateRequest) -> Option<Candidate> {
        if !req.covers_screen(self.width, self.height) || !self.src.original.starts_with("https://")
        {
            return None;
        }
        let title = self
            .alt
            .filter(|a| !a.trim().is_empty())
            .unwrap_or_else(|| label.to_owned());
        // The ready-made `large` rendition is a landscape crop; a width-only resize keeps the
        // whole frame, which is what scoring needs to see.
        let sep = if self.src.original.contains('?') {
            '&'
        } else {
            '?'
        };
        let preview_url = format!("{}{sep}auto=compress&cs=tinysrgb&w=1000", self.src.original);
        Some(Candidate {
            source: SourceKind::Pexels,
            id: format!("pexels:{}", self.id),
            title: truncate(title.trim(), 120),
            description: None,
            page_url: self.url,
            image_url: self.src.original,
            preview_url,
            width: self.width,
            height: self.height,
            author: truncate(&self.photographer, 80),
            author_url: self.photographer_url,
            license: License {
                short_name: "Pexels License".into(),
                url: Some("https://www.pexels.com/license/".into()),
                attribution_required: true,
            },
            curation: Curation::default(),
            coords: None,
            place_label: label.to_owned(),
        })
    }
}
