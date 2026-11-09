//! Photo sources. A source turns a place into a list of [`Candidate`]s; it does not download
//! images or judge beauty beyond what curation already tells us.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::geo::GeoPoint;
use crate::http::HttpClient;
use crate::place::{Place, Scope, SubPlace};
use crate::subject::SubjectKind;

pub mod commons;
pub mod openverse;
pub mod pexels;
pub mod pinterest;
pub mod pixabay;
pub mod unsplash;
pub mod wallhaven;

pub use commons::CommonsSource;
pub use openverse::OpenverseSource;
pub use pexels::PexelsSource;
pub use pinterest::PinterestSource;
pub use pixabay::PixabaySource;
pub use unsplash::UnsplashSource;
pub use wallhaven::WallhavenSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Commons,
    Unsplash,
    Openverse,
    Wallhaven,
    Pexels,
    Pixabay,
    Pinterest,
}

impl SourceKind {
    pub const ALL: [SourceKind; 7] = [
        SourceKind::Commons,
        SourceKind::Openverse,
        SourceKind::Wallhaven,
        SourceKind::Unsplash,
        SourceKind::Pexels,
        SourceKind::Pixabay,
        SourceKind::Pinterest,
    ];

    pub fn from_id(id: &str) -> Option<SourceKind> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// Same string as on the wire (`"commons"`, `"pixabay"`…).
    pub fn id(self) -> &'static str {
        match self {
            SourceKind::Commons => "commons",
            SourceKind::Unsplash => "unsplash",
            SourceKind::Openverse => "openverse",
            SourceKind::Wallhaven => "wallhaven",
            SourceKind::Pexels => "pexels",
            SourceKind::Pixabay => "pixabay",
            SourceKind::Pinterest => "pinterest",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SourceKind::Commons => "Wikimedia Commons",
            SourceKind::Unsplash => "Unsplash",
            SourceKind::Openverse => "Openverse",
            SourceKind::Wallhaven => "Wallhaven",
            SourceKind::Pexels => "Pexels",
            SourceKind::Pixabay => "Pixabay",
            SourceKind::Pinterest => "Pinterest",
        }
    }

    /// The user has to bring their own API key.
    pub fn needs_key(self) -> bool {
        matches!(
            self,
            SourceKind::Unsplash | SourceKind::Pexels | SourceKind::Pixabay
        )
    }

    /// Can this source serve a subject of that kind? (A place is served either by geography
    /// or, for key-word sources, by its label.) A Pinterest board is served by Pinterest only,
    /// and Pinterest serves nothing else (it has no search).
    pub fn supports(self, subject: SubjectKind) -> bool {
        match subject {
            SubjectKind::Place | SubjectKind::Query => self != SourceKind::Pinterest,
            SubjectKind::Board => self == SourceKind::Pinterest,
        }
    }

    /// Check the *shape* of a key (length, characters). It says nothing about whether the
    /// service accepts it; that is what `check_source` is for.
    pub fn validate_key(self, key: &str) -> Result<String> {
        let key = key.trim();
        let plausible = (16..=128).contains(&key.len())
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !plausible {
            return Err(Error::Config(format!(
                "{} API key looks malformed",
                self.name()
            )));
        }
        Ok(key.to_owned())
    }
}

/// Build the source `kind`. Key sources fail with [`Error::Config`] when `key` is missing.
pub fn build<'a>(
    kind: SourceKind,
    http: &'a dyn HttpClient,
    key: Option<&str>,
) -> Result<Box<dyn Source + 'a>> {
    let need_key = || {
        key.filter(|k| !k.trim().is_empty())
            .ok_or_else(|| Error::Config(format!("{} needs an API key", kind.name())))
    };
    Ok(match kind {
        SourceKind::Commons => Box::new(CommonsSource::new(http)),
        SourceKind::Openverse => Box::new(OpenverseSource::new(http)),
        SourceKind::Wallhaven => Box::new(WallhavenSource::new(http)),
        SourceKind::Unsplash => Box::new(UnsplashSource::new(http, need_key()?)?),
        SourceKind::Pexels => Box::new(PexelsSource::new(http, need_key()?)?),
        SourceKind::Pixabay => Box::new(PixabaySource::new(http, need_key()?)?),
        SourceKind::Pinterest => Box::new(PinterestSource::new(http)),
    })
}

/// What to look for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Search<'a> {
    Place {
        place: &'a Place,
        scope: &'a Scope,
    },
    Query(&'a str),
    /// A Pinterest board: its RSS address and the name shown to the user.
    Board {
        url: &'a str,
        name: &'a str,
    },
}

impl Search<'_> {
    pub fn kind(&self) -> SubjectKind {
        match self {
            Search::Place { .. } => SubjectKind::Place,
            Search::Query(_) => SubjectKind::Query,
            Search::Board { .. } => SubjectKind::Board,
        }
    }

    /// Plain text for sources that only understand key words: the place label or the query.
    pub fn text(&self) -> &str {
        match self {
            Search::Place { place, .. } => &place.label,
            Search::Query(q) => q,
            Search::Board { name, .. } => name,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct License {
    /// e.g. "CC BY-SA 4.0", "Public domain", "Unsplash License".
    pub short_name: String,
    pub url: Option<String>,
    pub attribution_required: bool,
}

/// What humans already said about the photo. The strongest beauty signal we have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Curation {
    pub featured: bool,
    pub quality: bool,
    pub valued: bool,
    /// Likes / favourites on the source (Unsplash, Pixabay, Wallhaven).
    pub likes: Option<u32>,
}

impl Curation {
    /// 0..=1, used as the `curation_bonus` feature.
    pub fn score(&self) -> f32 {
        let mut s: f32 = 0.0;
        if self.featured {
            s = s.max(1.0);
        }
        if self.quality {
            s = s.max(0.7);
        }
        if self.valued {
            s = s.max(0.5);
        }
        if let Some(likes) = self.likes {
            // 1000 likes ≈ featured-level popularity.
            s = s.max(((likes as f32 + 1.0).log10() / 3.0).min(1.0));
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub source: SourceKind,
    /// Stable id, unique across sources: "commons:<pageid>", "unsplash:<id>", "pexels:<id>"…
    pub id: String,
    /// Short human title, plain text.
    pub title: String,
    /// Longer plain-text description, if any.
    pub description: Option<String>,
    /// Page where the user can see the photo, author and license.
    pub page_url: String,
    /// Full-resolution image.
    pub image_url: String,
    /// ~1000 px wide preview, used for scoring and for the in-app preview.
    pub preview_url: String,
    pub width: u32,
    pub height: u32,
    /// Plain-text author name.
    pub author: String,
    pub author_url: Option<String>,
    pub license: License,
    pub curation: Curation,
    pub coords: Option<GeoPoint>,
    /// Name of the place this photo was found for (the sub-place in hierarchy mode).
    pub place_label: String,
}

impl Candidate {
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }

    /// One-line credit, e.g. "Photo: Basile Morin, CC BY-SA 4.0".
    pub fn credit_line(&self) -> String {
        match self.source {
            SourceKind::Unsplash | SourceKind::Pexels | SourceKind::Pixabay => {
                format!("Photo: {} on {}", self.author, self.source.name())
            }
            SourceKind::Commons | SourceKind::Openverse => {
                format!("Photo: {}, {}", self.author, self.license.short_name)
            }
            SourceKind::Pinterest => match self.author.as_str() {
                "" | "Pinterest" => "Pinterest".to_owned(),
                author => format!("Pin by {author} on Pinterest"),
            },
            // Wallhaven's search API names neither author nor licence; the uploader is filled in
            // by `Source::enrich_chosen` once the photo is chosen.
            SourceKind::Wallhaven => match self.author.as_str() {
                "" | "Wallhaven" => format!(
                    "Wallpaper: Wallhaven #{}",
                    self.id.strip_prefix("wallhaven:").unwrap_or(&self.id)
                ),
                author => format!("Wallhaven: {author}"),
            },
        }
    }
}

/// With "allow upscaling" on, a photo whose best rendition is at least this fraction of the
/// needed size is accepted (and upscaled when rendered). The single place for this threshold.
pub const UPSCALE_MIN_FRACTION: f32 = 0.7;

/// What the caller needs from candidates, before any image is downloaded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CandidateRequest {
    /// Target screen in physical pixels (portrait for phones).
    pub screen_width: u32,
    pub screen_height: u32,
    /// Upper bound on how many candidates to return.
    pub limit: u32,
    /// Sub-places of the place, used by [`Scope::Hierarchy`]. Resolved by the caller (and
    /// cached), so sources don't need their own Wikidata access.
    pub sub_places: Vec<SubPlace>,
    /// Varies which sub-places are sampled; rotate it between refills.
    pub seed: u64,
    /// The user accepts photos smaller than the screen; see [`UPSCALE_MIN_FRACTION`].
    pub allow_upscale: bool,
}

impl CandidateRequest {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            screen_width,
            screen_height,
            limit: 60,
            sub_places: Vec::new(),
            seed: 0,
            allow_upscale: false,
        }
    }

    /// Fraction of the screen size an image must reach: 1 normally, 0.7 when upscaling is allowed.
    pub fn min_fraction(&self) -> f32 {
        if self.allow_upscale {
            UPSCALE_MIN_FRACTION
        } else {
            1.0
        }
    }

    /// Smallest width/height to ask a source's API for (the screen, scaled by [`Self::min_fraction`]).
    pub fn min_size(&self) -> (u32, u32) {
        let f = self.min_fraction();
        (
            (self.screen_width as f32 * f).ceil() as u32,
            (self.screen_height as f32 * f).ceil() as u32,
        )
    }

    /// Which result page geographic searches should take (rotates with the seed).
    pub fn page(&self) -> u32 {
        (self.seed % 4) as u32
    }

    /// The image covers the screen after cropping to its aspect ratio, without upscaling (or,
    /// with `allow_upscale`, with at most ~1/0.7 of it). Used by the sources that search by key
    /// words (no fuzzy 20% allowance there).
    pub fn covers_screen(&self, width: u32, height: u32) -> bool {
        let (w, h) = self.min_size();
        width >= w && height >= h
    }

    /// "portrait" for phones, "landscape" otherwise.
    pub fn orientation(&self) -> &'static str {
        if self.screen_height >= self.screen_width {
            "portrait"
        } else {
            "landscape"
        }
    }

    /// Smallest image height that can still fill the screen without visible upscaling.
    /// A little upscaling (≈ 20%) is invisible on a phone, so we allow it.
    pub fn min_image_height(&self) -> u32 {
        let f = if self.allow_upscale {
            UPSCALE_MIN_FRACTION
        } else {
            0.8
        };
        (self.screen_height as f32 * f) as u32
    }
}

pub trait Source: Send + Sync {
    fn kind(&self) -> SourceKind;

    fn candidates(&self, search: &Search<'_>, req: &CandidateRequest) -> Result<Vec<Candidate>>;

    /// One lightweight request with a short timeout: is the service reachable, and does it
    /// accept our key? Costs as little of the source's rate limit as possible.
    fn check(&self) -> Result<()>;

    /// URL of a version of the image at least `min_width` × `min_height` pixels, as small as
    /// possible. Falls back to the original.
    fn sized_url(&self, c: &Candidate, min_width: u32, min_height: u32) -> String;

    /// Called when a candidate is about to become the wallpaper: fetch details that are too
    /// costly to get for every candidate (Wallhaven's uploader) and return the improved
    /// candidate, or `None` to keep it as it is. The caller ignores errors.
    fn enrich_chosen(&self, _c: &Candidate) -> Result<Option<Candidate>> {
        Ok(None)
    }

    /// Called once when a candidate actually becomes the wallpaper. Unsplash's API terms
    /// require pinging their download endpoint; other sources do nothing.
    fn report_used(&self, _c: &Candidate) -> Result<()> {
        Ok(())
    }
}

/// Take one item from each group in turn: [a1 a2 a3] [b1 b2] → a1 b1 a2 b2 a3.
pub fn interleave<T>(groups: Vec<Vec<T>>) -> Vec<T> {
    let mut iters: Vec<_> = groups.into_iter().map(Vec::into_iter).collect();
    let mut out = Vec::new();
    loop {
        let before = out.len();
        for it in &mut iters {
            if let Some(x) = it.next() {
                out.push(x);
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// Keep the first occurrence of each id.
pub fn dedup(candidates: &mut Vec<Candidate>) {
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.id.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_screen_strict_and_upscaled() {
        let mut req = CandidateRequest::new(1000, 2000);
        assert!(req.covers_screen(1000, 2000));
        assert!(!req.covers_screen(800, 1600));
        assert_eq!(req.min_image_height(), 1600);
        req.allow_upscale = true;
        assert!(req.covers_screen(700, 1400));
        assert!(!req.covers_screen(690, 1400));
        assert_eq!(req.min_size(), (700, 1400));
        assert_eq!(req.min_image_height(), 1400);
    }

    #[test]
    fn interleave_round_robin() {
        assert_eq!(
            interleave(vec![vec![1, 2, 3], vec![10, 20], vec![], vec![100]]),
            [1, 10, 100, 2, 20, 3]
        );
        assert!(interleave::<u8>(vec![]).is_empty());
    }

    #[test]
    fn curation_score_ordering() {
        let f = Curation {
            featured: true,
            ..Default::default()
        };
        let q = Curation {
            quality: true,
            ..Default::default()
        };
        let none = Curation::default();
        assert!(f.score() > q.score() && q.score() > none.score());
        let popular = Curation {
            likes: Some(999),
            ..Default::default()
        };
        assert!((popular.score() - 1.0).abs() < 1e-3);
        let meh = Curation {
            likes: Some(9),
            ..Default::default()
        };
        assert!((meh.score() - 1.0 / 3.0).abs() < 1e-3);
    }
}
