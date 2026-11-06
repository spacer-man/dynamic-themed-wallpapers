//! Unsplash: opt-in, with the user's own API key (never shipped with the app).
//!
//! Unsplash has no geographic search in its public API, so the place label is used as the
//! query. Their API guidelines require: hotlinking their image URLs, attributing the
//! photographer with links carrying `utm_source`/`utm_medium=referral`, and pinging the
//! download endpoint when a photo is actually used. The utm parameters name the app, not the
//! user.

use serde::Deserialize;

use super::{Candidate, CandidateRequest, Curation, License, Search, Source, SourceKind};
use crate::error::{Error, Result};
use crate::http::{build_url, HttpClient, MAX_JSON_BYTES};
use crate::text::truncate;

const API: &str = "https://api.unsplash.com";
const UTM: &str = "utm_source=dynamic_themed_wallpapers&utm_medium=referral";

pub struct UnsplashSource<'a> {
    http: &'a dyn HttpClient,
    auth: String,
}

impl<'a> UnsplashSource<'a> {
    /// `access_key` is the user's own Unsplash "Access Key".
    pub fn new(http: &'a dyn HttpClient, access_key: &str) -> Result<Self> {
        let key = access_key.trim();
        let plausible = (20..=128).contains(&key.len())
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !plausible {
            return Err(Error::Config("Unsplash access key looks malformed".into()));
        }
        Ok(Self {
            http,
            auth: format!("Client-ID {key}"),
        })
    }

    fn get(&self, url: &str) -> Result<Vec<u8>> {
        self.http.get_with(
            url,
            &[("Authorization", &self.auth), ("Accept-Version", "v1")],
            MAX_JSON_BYTES,
        )
    }
}

impl Source for UnsplashSource<'_> {
    fn kind(&self) -> SourceKind {
        SourceKind::Unsplash
    }

    fn candidates(&self, search: &Search<'_>, req: &CandidateRequest) -> Result<Vec<Candidate>> {
        // No geo search in the public API: a place is searched by its label.
        let label = search.text();
        let orientation = req.orientation();
        // Rotate through the first few result pages so refills don't repeat themselves.
        let page = (req.seed % 3 + 1).to_string();
        let per_page = req.limit.clamp(1, 30).to_string();
        let url = build_url(
            &format!("{API}/search/photos"),
            &[
                ("query", label),
                ("per_page", &per_page),
                ("page", &page),
                ("orientation", orientation),
                ("content_filter", "high"),
                ("order_by", "relevant"),
            ],
        );
        let body = self.get(&url)?;
        let resp: SearchResponse =
            serde_json::from_slice(&body).map_err(|e| Error::parse("unsplash search", e))?;
        let min_h = req.min_image_height();
        Ok(resp
            .results
            .into_iter()
            .filter(|p| p.height >= min_h || p.width >= min_h)
            .map(|p| p.into_candidate(label))
            .collect())
    }

    fn check(&self) -> Result<()> {
        let url = build_url(
            &format!("{API}/search/photos"),
            &[("query", "sky"), ("per_page", "1")],
        );
        self.http
            .get_quick(
                &url,
                &[("Authorization", &self.auth), ("Accept-Version", "v1")],
                MAX_JSON_BYTES,
            )
            .map(|_| ())
    }

    fn sized_url(&self, c: &Candidate, min_width: u32, min_height: u32) -> String {
        let need = min_width.max((min_height as f32 * c.aspect()).ceil() as u32);
        if need >= c.width {
            return c.image_url.clone();
        }
        // imgix parameters on the raw URL.
        let sep = if c.image_url.contains('?') { '&' } else { '?' };
        format!("{}{sep}w={need}&fm=jpg&q=85", c.image_url)
    }

    fn report_used(&self, c: &Candidate) -> Result<()> {
        let id = c.id.strip_prefix("unsplash:").unwrap_or(&c.id);
        let url = build_url(&format!("{API}/photos/{id}/download"), &[]);
        self.get(&url).map(|_| ())
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<Photo>,
}

#[derive(Deserialize)]
struct Photo {
    id: String,
    width: u32,
    height: u32,
    description: Option<String>,
    alt_description: Option<String>,
    urls: Urls,
    links: Links,
    #[serde(default)]
    likes: u32,
    user: User,
}

#[derive(Deserialize)]
struct Urls {
    raw: String,
    regular: String,
}

#[derive(Deserialize)]
struct Links {
    html: String,
}

#[derive(Deserialize)]
struct User {
    name: String,
    links: Links,
}

fn with_utm(url: &str) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}{UTM}")
}

impl Photo {
    fn into_candidate(self, place_label: &str) -> Candidate {
        let title = self
            .description
            .clone()
            .or(self.alt_description.clone())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| place_label.to_owned());
        Candidate {
            source: SourceKind::Unsplash,
            id: format!("unsplash:{}", self.id),
            title: truncate(title.trim(), 120),
            description: self
                .alt_description
                .filter(|d| Some(d) != self.description.as_ref()),
            page_url: with_utm(&self.links.html),
            image_url: self.urls.raw,
            preview_url: self.urls.regular,
            width: self.width,
            height: self.height,
            author: truncate(&self.user.name, 80),
            author_url: Some(with_utm(&self.user.links.html)),
            license: License {
                short_name: "Unsplash License".into(),
                url: Some("https://unsplash.com/license".into()),
                attribution_required: false,
            },
            curation: Curation {
                likes: Some(self.likes),
                ..Default::default()
            },
            coords: None,
            place_label: place_label.to_owned(),
        }
    }
}
