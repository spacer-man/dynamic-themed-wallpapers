//! Photo sources. A source turns a place into a list of [`Candidate`]s; it does not download
//! images or judge beauty beyond what curation already tells us.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::geo::GeoPoint;
use crate::place::{Place, Scope, SubPlace};

pub mod commons;
pub mod unsplash;

pub use commons::CommonsSource;
pub use unsplash::UnsplashSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Commons,
    Unsplash,
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
    /// Unsplash likes.
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
            // 1000 likes ≈ featured-level popularity on Unsplash.
            s = s.max(((likes as f32 + 1.0).log10() / 3.0).min(1.0));
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub source: SourceKind,
    /// Stable id, unique across sources: "commons:<pageid>" or "unsplash:<id>".
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
            SourceKind::Unsplash => format!("Photo: {} on Unsplash", self.author),
            SourceKind::Commons => format!("Photo: {}, {}", self.author, self.license.short_name),
        }
    }
}

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
}

impl CandidateRequest {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            screen_width,
            screen_height,
            limit: 60,
            sub_places: Vec::new(),
            seed: 0,
        }
    }

    /// Which result page geographic searches should take (rotates with the seed).
    pub fn page(&self) -> u32 {
        (self.seed % 4) as u32
    }

    /// Smallest image height that can still fill the screen without visible upscaling.
    /// A little upscaling (≈ 20%) is invisible on a phone, so we allow it.
    pub fn min_image_height(&self) -> u32 {
        (self.screen_height as f32 * 0.8) as u32
    }
}

pub trait Source: Send + Sync {
    fn kind(&self) -> SourceKind;

    fn candidates(
        &self,
        place: &Place,
        scope: &Scope,
        req: &CandidateRequest,
    ) -> Result<Vec<Candidate>>;

    /// URL of a version of the image at least `min_width` × `min_height` pixels, as small as
    /// possible. Falls back to the original.
    fn sized_url(&self, c: &Candidate, min_width: u32, min_height: u32) -> String;

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
