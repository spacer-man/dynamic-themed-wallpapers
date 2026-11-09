//! From measurements to a decision: hard rejects, a feature vector, a tiny logistic model
//! that learns from likes and skips on the device, and ε-greedy selection.
//!
//! Formulas and thresholds: docs/scoring.md.

use serde::{Deserialize, Serialize};

use crate::analysis::{hash, Analysis, Tone};
use crate::source::Candidate;

// --- preferences ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToneFilter {
    #[default]
    Any,
    Dark,
    Light,
}

impl ToneFilter {
    pub fn accepts(self, tone: Tone) -> bool {
        match self {
            ToneFilter::Any => true,
            ToneFilter::Dark => tone == Tone::Dark,
            ToneFilter::Light => tone == Tone::Light,
        }
    }
}

// --- hard rejects ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reject {
    WrongTone,
    TooSmall,
    ExcessiveCrop,
    Blurry,
    BlownHighlights,
    CrushedShadows,
    Duplicate,
}

pub const MIN_CROP_KEEP: f32 = 0.20;
pub const MIN_SHARPNESS: f32 = 12.0;
pub const MAX_HIGHLIGHT_CLIP: f32 = 0.20;
pub const MAX_SHADOW_CLIP: f32 = 0.60;
/// The photo may be upscaled by at most this factor to cover the screen.
pub const MAX_UPSCALE: f32 = 1.25;

/// The most the photo may be upscaled: [`MAX_UPSCALE`], or the inverse of the upscale
/// fraction when the user allows smaller photos.
pub fn max_upscale(allow_upscale: bool) -> f32 {
    if allow_upscale {
        MAX_UPSCALE.max(1.0 / crate::source::UPSCALE_MIN_FRACTION)
    } else {
        MAX_UPSCALE
    }
}

/// `recent_hashes`: dHashes of photos shown recently.
pub fn check(
    c: &Candidate,
    a: &Analysis,
    screen: (u32, u32),
    tone: ToneFilter,
    recent_hashes: &[u64],
    allow_upscale: bool,
) -> Result<(), Reject> {
    if !tone.accepts(a.tone) {
        return Err(Reject::WrongTone);
    }
    // Pixels of the original that end up on screen, along each axis.
    let crop_w = c.width as f32 * a.crop.w;
    let crop_h = c.height as f32 * a.crop.h;
    let up = max_upscale(allow_upscale);
    if crop_w * up < screen.0 as f32 || crop_h * up < screen.1 as f32 {
        return Err(Reject::TooSmall);
    }
    if a.crop_keep < MIN_CROP_KEEP {
        return Err(Reject::ExcessiveCrop);
    }
    if a.sharpness < MIN_SHARPNESS {
        return Err(Reject::Blurry);
    }
    if a.highlight_clip > MAX_HIGHLIGHT_CLIP {
        return Err(Reject::BlownHighlights);
    }
    if a.shadow_clip > MAX_SHADOW_CLIP {
        return Err(Reject::CrushedShadows);
    }
    if recent_hashes
        .iter()
        .any(|&h| hash::is_near_duplicate(h, a.dhash))
    {
        return Err(Reject::Duplicate);
    }
    Ok(())
}

// --- features -------------------------------------------------------------------------------

pub const N_FEATURES: usize = 10;

pub const FEATURE_NAMES: [&str; N_FEATURES] = [
    "curation",
    "sharpness",
    "contrast",
    "colorfulness",
    "top_band_calmness",
    "dock_calmness",
    "crop_keep",
    "palette_coherence",
    "entropy",
    "exposure",
];

/// All features in 0..1, higher = more beautiful as a wallpaper.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Features(pub [f32; N_FEATURES]);

fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

impl Features {
    pub fn of(c: &Candidate, a: &Analysis) -> Self {
        Features([
            unit(c.curation.score()),
            unit((1.0 + a.sharpness).ln() / (1.0f32 + 1000.0).ln()),
            unit(a.contrast / 0.25),
            unit(a.colorfulness / 100.0),
            unit(1.0 - a.top_band_busyness / 0.12),
            unit(1.0 - a.dock_busyness / 0.12),
            unit((a.crop_keep - MIN_CROP_KEEP) / (1.0 - MIN_CROP_KEEP)),
            unit(1.0 - a.palette_spread / 0.15),
            unit(a.entropy / 6.0),
            unit(1.0 - (a.highlight_clip * 4.0 + a.shadow_clip)),
        ])
    }

    pub fn named(&self) -> impl Iterator<Item = (&'static str, f32)> + '_ {
        FEATURE_NAMES.iter().copied().zip(self.0.iter().copied())
    }
}

// --- model ----------------------------------------------------------------------------------

/// Hand-set starting weights (logit scale). Curation dominates, then the things that make a
/// wallpaper usable: sharpness, a calm status-bar area, decent exposure.
pub const PRIOR_WEIGHTS: [f32; N_FEATURES] = [2.0, 1.0, 0.6, 0.6, 0.8, 0.4, 0.6, 0.4, 0.3, 0.8];
pub const PRIOR_BIAS: f32 = -3.0;

/// Logistic regression `p(like) = σ(w·x + b)`, trained online with SGD. L2 regularisation
/// pulls towards the prior rather than towards zero, so a handful of likes can't erase what
/// curation already knows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub weights: [f32; N_FEATURES],
    pub bias: f32,
    pub updates: u32,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            weights: PRIOR_WEIGHTS,
            bias: PRIOR_BIAS,
            updates: 0,
        }
    }
}

pub const LEARNING_RATE: f32 = 0.08;
pub const L2_TO_PRIOR: f32 = 0.02;

fn sigmoid(z: f32) -> f32 {
    1.0 / (1.0 + (-z).exp())
}

impl Model {
    pub fn predict(&self, f: &Features) -> f32 {
        let z = self.bias
            + self
                .weights
                .iter()
                .zip(f.0.iter())
                .map(|(w, x)| w * x)
                .sum::<f32>();
        sigmoid(z)
    }

    /// One SGD step on a like (`true`) or skip (`false`).
    pub fn learn(&mut self, f: &Features, liked: bool) {
        let y = if liked { 1.0 } else { 0.0 };
        let err = y - self.predict(f);
        for ((w, prior), x) in self.weights.iter_mut().zip(PRIOR_WEIGHTS).zip(f.0) {
            *w += LEARNING_RATE * (err * x - L2_TO_PRIOR * (*w - prior));
        }
        self.bias += LEARNING_RATE * (err - L2_TO_PRIOR * (self.bias - PRIOR_BIAS));
        self.updates += 1;
    }
}

// --- selection ------------------------------------------------------------------------------

/// Share of picks that explore instead of exploiting, so the model keeps seeing variety.
pub const EPSILON: f32 = 0.15;

/// Index of the chosen item given model scores. With probability `epsilon` pick uniformly
/// among the top half (exploration that still avoids the worst); otherwise the best.
pub fn choose(scores: &[f32], epsilon: f32, rng: &mut fastrand::Rng) -> Option<usize> {
    if scores.is_empty() {
        return None;
    }
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    if rng.f32() < epsilon {
        let half = order.len().div_ceil(2);
        Some(order[rng.usize(..half)])
    } else {
        Some(order[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{CropRect, Tone};
    use crate::source::{Curation, License, SourceKind};

    fn analysis() -> Analysis {
        Analysis {
            crop: CropRect {
                x: 0.35,
                y: 0.0,
                w: 0.3,
                h: 1.0,
            },
            crop_keep: 0.3,
            tone: Tone::Dark,
            luma_median: 0.2,
            luma_p10: 0.05,
            luma_p90: 0.5,
            contrast: 0.2,
            colorfulness: 40.0,
            sharpness: 300.0,
            top_band_busyness: 0.01,
            dock_busyness: 0.05,
            highlight_clip: 0.01,
            shadow_clip: 0.05,
            entropy: 5.0,
            palette: vec![],
            palette_spread: 0.05,
            dhash: 0xF0F0_F0F0_F0F0_F0F0,
        }
    }

    fn candidate(w: u32, h: u32) -> Candidate {
        Candidate {
            source: SourceKind::Commons,
            id: "commons:1".into(),
            title: "t".into(),
            description: None,
            page_url: "p".into(),
            image_url: "i".into(),
            preview_url: "v".into(),
            width: w,
            height: h,
            author: "a".into(),
            author_url: None,
            license: License {
                short_name: "CC0".into(),
                url: None,
                attribution_required: false,
            },
            curation: Curation {
                featured: true,
                ..Default::default()
            },
            coords: None,
            place_label: "x".into(),
        }
    }

    const SCREEN: (u32, u32) = (1080, 2400);

    #[test]
    fn rejects() {
        let c = candidate(6000, 4000);
        let a = analysis();
        assert_eq!(check(&c, &a, SCREEN, ToneFilter::Dark, &[], false), Ok(()));
        assert_eq!(
            check(&c, &a, SCREEN, ToneFilter::Light, &[], false),
            Err(Reject::WrongTone)
        );
        assert_eq!(
            check(
                &candidate(3000, 1800),
                &a,
                SCREEN,
                ToneFilter::Any,
                &[],
                false
            ),
            Err(Reject::TooSmall)
        );
        let blurry = Analysis {
            sharpness: 3.0,
            ..analysis()
        };
        assert_eq!(
            check(&c, &blurry, SCREEN, ToneFilter::Any, &[], false),
            Err(Reject::Blurry)
        );
        let pano = Analysis {
            crop_keep: 0.15,
            ..analysis()
        };
        assert_eq!(
            check(&c, &pano, SCREEN, ToneFilter::Any, &[], false),
            Err(Reject::ExcessiveCrop)
        );
        let blown = Analysis {
            highlight_clip: 0.3,
            ..analysis()
        };
        assert_eq!(
            check(&c, &blown, SCREEN, ToneFilter::Any, &[], false),
            Err(Reject::BlownHighlights)
        );
        let near = a.dhash ^ 0b111; // 3 bits off
        assert_eq!(
            check(&c, &a, SCREEN, ToneFilter::Any, &[near], false),
            Err(Reject::Duplicate)
        );
    }

    #[test]
    fn features_are_unit_range() {
        let f = Features::of(&candidate(6000, 4000), &analysis());
        assert!(f.0.iter().all(|v| (0.0..=1.0).contains(v)), "{f:?}");
        assert_eq!(f.0[0], 1.0);
        let weird = Analysis {
            sharpness: f32::NAN,
            contrast: -1.0,
            ..analysis()
        };
        let f = Features::of(&candidate(6000, 4000), &weird);
        assert!(f.0.iter().all(|v| (0.0..=1.0).contains(v)), "{f:?}");
    }

    #[test]
    fn model_learns_from_feedback() {
        let mut m = Model::default();
        let colourful = Features([0.5, 0.8, 0.5, 1.0, 0.8, 0.5, 0.5, 0.5, 0.5, 0.9]);
        let grey = Features([0.5, 0.8, 0.5, 0.0, 0.8, 0.5, 0.5, 0.5, 0.5, 0.9]);
        let before = m.predict(&colourful) - m.predict(&grey);
        for _ in 0..30 {
            m.learn(&colourful, true);
            m.learn(&grey, false);
        }
        let after = m.predict(&colourful) - m.predict(&grey);
        assert!(after > before + 0.1, "{before} → {after}");
        assert_eq!(m.updates, 60);
        // Regularisation keeps unrelated weights near the prior.
        assert!((m.weights[0] - PRIOR_WEIGHTS[0]).abs() < 1.0);
    }

    #[test]
    fn prior_prefers_featured_over_uncurated() {
        let m = Model::default();
        let a = analysis();
        let mut plain = candidate(6000, 4000);
        plain.curation = Curation::default();
        assert!(
            m.predict(&Features::of(&candidate(6000, 4000), &a))
                > m.predict(&Features::of(&plain, &a))
        );
    }

    #[test]
    fn selection() {
        let mut rng = fastrand::Rng::with_seed(1);
        assert_eq!(choose(&[], 0.15, &mut rng), None);
        assert_eq!(choose(&[0.1, 0.9, 0.5], 0.0, &mut rng), Some(1));
        // Pure exploration never picks from the bottom half.
        for _ in 0..200 {
            let i = choose(&[0.1, 0.9, 0.5, 0.2], 1.0, &mut rng).unwrap();
            assert!(i == 1 || i == 2, "{i}");
        }
    }
}
