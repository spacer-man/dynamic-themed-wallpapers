//! Image analysis on a small copy of the photo (long side ≤ [`ANALYSIS_SIDE`]).
//!
//! Everything here is cheap, deterministic and dependency-free beyond decoding/resizing.
//! Thresholds and formulas are documented in docs/scoring.md.

use image::{imageops::FilterType, DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};

pub mod color;
pub mod crop;
pub mod hash;
pub mod stats;

pub use color::Swatch;
pub use crop::CropRect;

/// Long side of the working copy. Big enough for sharpness, small enough to be ~free.
pub const ANALYSIS_SIDE: u32 = 512;

/// Linear-ish working copy: sRGB values in 0..1 plus luma.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<[f32; 3]>,
    pub luma: Vec<f32>,
}

impl Prepared {
    pub fn from_image(img: &DynamicImage, max_side: u32) -> Self {
        let (w, h) = img.dimensions();
        let small = if w.max(h) > max_side {
            img.resize(max_side, max_side, FilterType::Triangle)
        } else {
            img.clone()
        };
        let rgb8 = small.to_rgb8();
        let (w, h) = (rgb8.width() as usize, rgb8.height() as usize);
        let mut rgb = Vec::with_capacity(w * h);
        let mut luma = Vec::with_capacity(w * h);
        for p in rgb8.pixels() {
            let c = [
                p[0] as f32 / 255.0,
                p[1] as f32 / 255.0,
                p[2] as f32 / 255.0,
            ];
            luma.push(0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]);
            rgb.push(c);
        }
        Self {
            width: w,
            height: h,
            rgb,
            luma,
        }
    }

    /// Copy of a normalised sub-rectangle (at least 1×1).
    pub fn region(&self, r: CropRect) -> Prepared {
        let (x0, y0, x1, y1) = r.to_pixels(self.width, self.height);
        let (w, h) = (x1 - x0, y1 - y0);
        let mut rgb = Vec::with_capacity(w * h);
        let mut luma = Vec::with_capacity(w * h);
        for y in y0..y1 {
            let row = y * self.width;
            rgb.extend_from_slice(&self.rgb[row + x0..row + x1]);
            luma.extend_from_slice(&self.luma[row + x0..row + x1]);
        }
        Prepared {
            width: w,
            height: h,
            rgb,
            luma,
        }
    }

    /// Horizontal band between `top` and `bottom` (fractions of height).
    pub fn band(&self, top: f32, bottom: f32) -> Prepared {
        self.region(CropRect {
            x: 0.0,
            y: top,
            w: 1.0,
            h: (bottom - top).max(0.0),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Dark,
    Mid,
    Light,
}

/// Everything we measure about one photo, as it will look on the screen (i.e. after cropping).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub crop: CropRect,
    /// Fraction of the photo's area that survives the crop.
    pub crop_keep: f32,
    pub tone: Tone,
    pub luma_median: f32,
    pub luma_p10: f32,
    pub luma_p90: f32,
    /// Standard deviation of luma, 0..0.5.
    pub contrast: f32,
    /// Hasler–Süsstrunk colourfulness on 0..255 scale (≈ 0 grey … 110+ extremely colourful).
    pub colorfulness: f32,
    /// Variance of the Laplacian on the central part, 0..255 luma scale.
    pub sharpness: f32,
    /// Mean gradient magnitude (luma 0..1) under the status bar.
    pub top_band_busyness: f32,
    /// Mean gradient magnitude in the dock / bottom icon row.
    pub dock_busyness: f32,
    /// Fraction of pixels blown out (luma ≥ 0.98) / crushed (≤ 0.02).
    pub highlight_clip: f32,
    pub shadow_clip: f32,
    /// Shannon entropy of the 64-bin luma histogram, bits (0..6).
    pub entropy: f32,
    pub palette: Vec<Swatch>,
    /// Weighted mean OKLab distance of pixels to their palette colour.
    pub palette_spread: f32,
    /// 64-bit difference hash of the whole photo (for near-duplicate detection).
    pub dhash: u64,
}

/// Screen zones, as fractions of screen height.
pub const STATUS_BAR_BAND: (f32, f32) = (0.0, 0.07);
pub const DOCK_BAND: (f32, f32) = (0.84, 0.96);

impl Analysis {
    /// Analyse `img` for a screen of `screen_w`×`screen_h`.
    pub fn of(img: &DynamicImage, screen_w: u32, screen_h: u32) -> Self {
        let full = Prepared::from_image(img, ANALYSIS_SIDE);
        let target_aspect = screen_w as f32 / screen_h.max(1) as f32;
        let crop = crop::plan(&full, target_aspect);
        let view = full.region(crop);

        let hist = stats::Histogram::of(&view.luma);
        let tone = stats::classify_tone(&hist);
        let (palette, palette_spread) = color::palette(&view, 5);
        let center = view.region(CropRect {
            x: 0.2,
            y: 0.2,
            w: 0.6,
            h: 0.6,
        });

        Self {
            crop,
            crop_keep: crop.w * crop.h,
            tone,
            luma_median: hist.percentile(0.5),
            luma_p10: hist.percentile(0.1),
            luma_p90: hist.percentile(0.9),
            contrast: stats::std_dev(&view.luma),
            colorfulness: color::colorfulness(&view),
            sharpness: stats::laplacian_variance(&center),
            top_band_busyness: stats::mean_gradient(
                &view.band(STATUS_BAR_BAND.0, STATUS_BAR_BAND.1),
            ),
            dock_busyness: stats::mean_gradient(&view.band(DOCK_BAND.0, DOCK_BAND.1)),
            highlight_clip: hist.fraction_at_least(0.98),
            shadow_clip: hist.fraction_at_most(0.02),
            entropy: hist.entropy_bits(64),
            palette,
            palette_spread,
            dhash: hash::dhash(img),
        }
    }
}

#[cfg(test)]
pub(crate) mod testimg {
    use image::{DynamicImage, Rgb, RgbImage};

    pub fn solid(w: u32, h: u32, c: [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb(c)))
    }

    pub fn from_fn(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| Rgb(f(x, y))))
    }

    /// Deterministic pseudo-noise in 0..=255.
    pub fn noise(x: u32, y: u32, seed: u32) -> u8 {
        let mut v =
            x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ seed.wrapping_mul(2246822519);
        v = (v ^ (v >> 13)).wrapping_mul(1274126177);
        (v >> 24) as u8
    }

    /// Textured image: noise of the given amplitude around a base grey level.
    pub fn textured(w: u32, h: u32, base: u8, amp: u8, seed: u32) -> DynamicImage {
        from_fn(w, h, |x, y| {
            let n = noise(x, y, seed) as i32 * amp as i32 / 255 - amp as i32 / 2;
            let v = (base as i32 + n).clamp(0, 255) as u8;
            [v, v, v]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::testimg::*;
    use super::*;

    #[test]
    fn dark_and_light_photos_are_classified() {
        let dark = Analysis::of(&textured(800, 600, 40, 40, 1), 1080, 2400);
        assert_eq!(dark.tone, Tone::Dark);
        let light = Analysis::of(&textured(800, 600, 215, 40, 2), 1080, 2400);
        assert_eq!(light.tone, Tone::Light);
        let mid = Analysis::of(&textured(800, 600, 128, 60, 3), 1080, 2400);
        assert_eq!(mid.tone, Tone::Mid);
    }

    #[test]
    fn flat_image_is_not_sharp_and_noise_is() {
        let flat = Analysis::of(&solid(800, 600, [120, 120, 120]), 1080, 2400);
        assert!(flat.sharpness < 1.0, "{}", flat.sharpness);
        assert!(flat.contrast < 1e-3, "{}", flat.contrast);
        let busy = Analysis::of(&textured(800, 600, 128, 200, 4), 1080, 2400);
        assert!(busy.sharpness > 100.0, "{}", busy.sharpness);
    }

    #[test]
    fn calm_sky_band_is_detected() {
        // Top 30% smooth sky, the rest busy.
        let img = from_fn(600, 1200, |x, y| {
            if y < 360 {
                [90, 140, 220]
            } else {
                let n = noise(x, y, 9);
                [n, n / 2, 255 - n]
            }
        });
        let a = Analysis::of(&img, 1080, 2400);
        assert!(a.top_band_busyness < 0.01, "{}", a.top_band_busyness);
        // Downscaling averages the noise, so "busy" here is ~0.05.
        assert!(a.dock_busyness > 0.03, "{}", a.dock_busyness);
    }

    #[test]
    fn clipping_is_measured() {
        let img = from_fn(400, 400, |x, _| {
            if x < 100 {
                [255, 255, 255]
            } else {
                [100, 100, 100]
            }
        });
        let a = Analysis::of(&img, 400, 400);
        assert!(
            (a.highlight_clip - 0.25).abs() < 0.03,
            "{}",
            a.highlight_clip
        );
        assert_eq!(a.shadow_clip, 0.0);
    }

    #[test]
    fn analysis_is_serializable() {
        let a = Analysis::of(&textured(300, 200, 100, 50, 5), 1080, 2400);
        let json = serde_json::to_string(&a).unwrap();
        let back: Analysis = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }
}
