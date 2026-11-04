//! Where to cut a photo to fit the screen.
//!
//! Slide a screen-shaped window along the long axis and keep the position that contains the
//! most visual energy (Sobel gradients), with a mild pull towards the centre so that
//! uniform photos are cropped symmetrically.

use serde::{Deserialize, Serialize};

use super::{stats::gradient_map, Prepared};

/// Normalised rectangle: all values are fractions (0..1) of the photo's width/height.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CropRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl CropRect {
    pub const FULL: CropRect = CropRect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };

    /// Pixel bounds `(x0, y0, x1, y1)` for an image of `w`×`h`, never empty.
    pub fn to_pixels(&self, w: usize, h: usize) -> (usize, usize, usize, usize) {
        let px = |f: f32, n: usize| ((f.clamp(0.0, 1.0) * n as f32).round() as usize).min(n);
        let x0 = px(self.x, w).min(w.saturating_sub(1));
        let y0 = px(self.y, h).min(h.saturating_sub(1));
        let x1 = px(self.x + self.w, w).max(x0 + 1).min(w.max(1));
        let y1 = px(self.y + self.h, h).max(y0 + 1).min(h.max(1));
        (x0, y0, x1, y1)
    }
}

/// Centre pull: losing this much energy share is worth one full "edge-to-centre" shift.
const CENTER_BIAS: f32 = 0.08;

pub fn plan(p: &Prepared, target_aspect: f32) -> CropRect {
    if p.width == 0 || p.height == 0 || !target_aspect.is_finite() || target_aspect <= 0.0 {
        return CropRect::FULL;
    }
    let aspect = p.width as f32 / p.height as f32;
    if (aspect - target_aspect).abs() / target_aspect < 0.01 {
        return CropRect::FULL;
    }
    let grad = gradient_map(p);
    if aspect > target_aspect {
        // Too wide: choose x.
        let frac = target_aspect / aspect;
        let profile: Vec<f32> = (0..p.width)
            .map(|x| (0..p.height).map(|y| grad[y * p.width + x]).sum())
            .collect();
        let start = best_window(&profile, frac);
        CropRect {
            x: start,
            y: 0.0,
            w: frac,
            h: 1.0,
        }
    } else {
        // Too tall: choose y.
        let frac = aspect / target_aspect;
        let profile: Vec<f32> = (0..p.height)
            .map(|y| grad[y * p.width..(y + 1) * p.width].iter().sum())
            .collect();
        let start = best_window(&profile, frac);
        CropRect {
            x: 0.0,
            y: start,
            w: 1.0,
            h: frac,
        }
    }
}

/// Start (as a fraction) of the window of relative size `frac` maximising energy share
/// minus a centre-distance penalty.
fn best_window(profile: &[f32], frac: f32) -> f32 {
    let n = profile.len();
    let win = ((frac * n as f32).round() as usize).clamp(1, n);
    if win >= n {
        return 0.0;
    }
    let total: f32 = profile.iter().sum::<f32>().max(1e-6);
    let mut sum: f32 = profile[..win].iter().sum();
    let max_start = n - win;
    let score = |start: usize, sum: f32| {
        let center = (start as f32 + win as f32 / 2.0) / n as f32;
        let dist = (center - 0.5).abs() / (0.5 - frac / 2.0).max(1e-6);
        sum / total - CENTER_BIAS * dist
    };
    let (mut best, mut best_score) = (0usize, score(0, sum));
    for start in 1..=max_start {
        sum += profile[start + win - 1] - profile[start - 1];
        let s = score(start, sum);
        if s > best_score + 1e-6 {
            best = start;
            best_score = s;
        }
    }
    (best as f32 / n as f32).min(1.0 - frac)
}

#[cfg(test)]
mod tests {
    use super::super::testimg::*;
    use super::super::{Prepared, ANALYSIS_SIDE};
    use super::*;

    fn prep(img: &image::DynamicImage) -> Prepared {
        Prepared::from_image(img, ANALYSIS_SIDE)
    }

    #[test]
    fn uniform_photo_is_cropped_in_the_centre() {
        let r = plan(&prep(&solid(1500, 1000, [50, 90, 160])), 1080.0 / 2400.0);
        assert!((r.w - 0.3).abs() < 0.01, "{r:?}");
        assert!((r.x + r.w / 2.0 - 0.5).abs() < 0.01, "{r:?}");
        assert_eq!((r.y, r.h), (0.0, 1.0));
    }

    #[test]
    fn crop_follows_the_detail() {
        // Detail only in the right fifth.
        let img = from_fn(1500, 1000, |x, y| {
            if x > 1200 {
                let n = noise(x, y, 3);
                [n, n, n]
            } else {
                [128, 128, 128]
            }
        });
        let r = plan(&prep(&img), 1080.0 / 2400.0);
        assert!(r.x + r.w > 0.95, "{r:?}");
    }

    #[test]
    fn tall_photo_crops_vertically() {
        let r = plan(&prep(&solid(1000, 4000, [0, 0, 0])), 1080.0 / 2400.0);
        assert_eq!((r.x, r.w), (0.0, 1.0));
        assert!((r.h - 0.25 / 0.45).abs() < 0.01, "{r:?}");
    }

    #[test]
    fn matching_aspect_is_untouched() {
        assert_eq!(
            plan(&prep(&solid(450, 1000, [9, 9, 9])), 0.45),
            CropRect::FULL
        );
    }

    #[test]
    fn pixel_bounds_never_empty() {
        let r = CropRect {
            x: 0.999,
            y: 0.999,
            w: 0.0,
            h: 0.0,
        };
        let (x0, y0, x1, y1) = r.to_pixels(10, 10);
        assert!(x1 > x0 && y1 > y0 && x1 <= 10 && y1 <= 10);
    }
}
