//! UI theme derived from the wallpaper's palette ("photo-forward" design).
//!
//! The photo provides the hue; lightness is forced into ranges that guarantee WCAG contrast:
//! body text ≥ 7:1 on the background, accent text ≥ 4.5:1.

use serde::{Deserialize, Serialize};

use crate::analysis::color::{oklab_to_srgb, srgb_to_oklab, to_u8};
use crate::analysis::Swatch;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub dark: bool,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub text_muted: String,
    pub accent: String,
    pub on_accent: String,
}

/// WCAG relative luminance of an sRGB colour.
pub fn relative_luminance(rgb: [u8; 3]) -> f32 {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2])
}

pub fn contrast_ratio(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// Same hue/chroma, given OKLab lightness; chroma scaled down by `chroma` (0..1).
fn with_lightness(rgb: [u8; 3], l: f32, chroma: f32) -> [u8; 3] {
    let lab = srgb_to_oklab(rgb.map(|c| c as f32 / 255.0));
    to_u8(oklab_to_srgb([l, lab[1] * chroma, lab[2] * chroma]))
}

fn chroma(rgb: [u8; 3]) -> f32 {
    let lab = srgb_to_oklab(rgb.map(|c| c as f32 / 255.0));
    (lab[1] * lab[1] + lab[2] * lab[2]).sqrt()
}

/// Move lightness away from `bg` until `fg` reaches `min` contrast with it.
fn ensure_contrast(fg: [u8; 3], bg: [u8; 3], min: f32, dark: bool) -> [u8; 3] {
    let lab = srgb_to_oklab(fg.map(|c| c as f32 / 255.0));
    let mut l = lab[0];
    let mut out = fg;
    for _ in 0..40 {
        if contrast_ratio(out, bg) >= min {
            return out;
        }
        l = if dark {
            (l + 0.025).min(1.0)
        } else {
            (l - 0.025).max(0.0)
        };
        out = to_u8(oklab_to_srgb([l, lab[1], lab[2]]));
    }
    if dark {
        [255, 255, 255]
    } else {
        [0, 0, 0]
    }
}

/// `dark`: whether the UI should be dark (follow the photo's tone or the system setting).
pub fn from_palette(palette: &[Swatch], dark: bool) -> Theme {
    let dominant = palette.first().map(|s| s.rgb).unwrap_or([40, 40, 48]);
    let accent_src = palette
        .iter()
        .filter(|s| s.weight >= 0.03)
        .max_by(|a, b| {
            (chroma(a.rgb) * a.weight.sqrt()).total_cmp(&(chroma(b.rgb) * b.weight.sqrt()))
        })
        .map(|s| s.rgb)
        .unwrap_or(dominant);

    let (bg_l, surf_l) = if dark { (0.18, 0.24) } else { (0.97, 0.93) };
    let background = with_lightness(dominant, bg_l, 0.5);
    let surface = with_lightness(dominant, surf_l, 0.6);
    let text = ensure_contrast(
        with_lightness(dominant, if dark { 0.95 } else { 0.2 }, 0.2),
        background,
        7.0,
        dark,
    );
    let text_muted = ensure_contrast(
        with_lightness(dominant, if dark { 0.78 } else { 0.42 }, 0.3),
        background,
        4.5,
        dark,
    );
    let accent = ensure_contrast(accent_src, background, 4.5, dark);
    let on_accent = if contrast_ratio([255, 255, 255], accent) >= contrast_ratio([0, 0, 0], accent)
    {
        [255, 255, 255]
    } else {
        [0, 0, 0]
    };

    Theme {
        dark,
        background: hex(background),
        surface: hex(surface),
        text: hex(text),
        text_muted: hex(text_muted),
        accent: hex(accent),
        on_accent: hex(on_accent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(h: &str) -> [u8; 3] {
        let v = u32::from_str_radix(&h[1..], 16).unwrap();
        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
    }

    #[test]
    fn wcag_reference_values() {
        assert!((contrast_ratio([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 0.01);
        assert!((contrast_ratio([119, 119, 119], [255, 255, 255]) - 4.48).abs() < 0.02);
    }

    #[test]
    fn themes_meet_contrast_for_awkward_palettes() {
        let palettes: Vec<Vec<Swatch>> = vec![
            vec![],
            vec![Swatch {
                rgb: [255, 255, 0],
                weight: 1.0,
            }],
            vec![
                Swatch {
                    rgb: [10, 10, 10],
                    weight: 0.9,
                },
                Swatch {
                    rgb: [200, 30, 30],
                    weight: 0.1,
                },
            ],
            vec![
                Swatch {
                    rgb: [240, 240, 250],
                    weight: 0.6,
                },
                Swatch {
                    rgb: [30, 60, 200],
                    weight: 0.4,
                },
            ],
        ];
        for p in &palettes {
            for dark in [true, false] {
                let t = from_palette(p, dark);
                let bg = parse(&t.background);
                assert!(contrast_ratio(parse(&t.text), bg) >= 7.0, "{t:?}");
                assert!(contrast_ratio(parse(&t.text_muted), bg) >= 4.5, "{t:?}");
                assert!(contrast_ratio(parse(&t.accent), bg) >= 4.5, "{t:?}");
                assert!(
                    contrast_ratio(parse(&t.on_accent), parse(&t.accent)) >= 4.5,
                    "{t:?}"
                );
            }
        }
    }
}
