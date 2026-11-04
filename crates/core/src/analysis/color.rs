//! Colour: Hasler–Süsstrunk colourfulness, OKLab conversion, k-means palette.

use serde::{Deserialize, Serialize};

use super::Prepared;

/// One palette entry.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Swatch {
    pub rgb: [u8; 3],
    /// Share of pixels closest to this colour, 0..1.
    pub weight: f32,
}

impl Swatch {
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.rgb[0], self.rgb[1], self.rgb[2])
    }
}

/// Hasler & Süsstrunk (2003), on the 0..255 scale.
pub fn colorfulness(p: &Prepared) -> f32 {
    if p.rgb.is_empty() {
        return 0.0;
    }
    let n = p.rgb.len() as f32;
    let (mut srg, mut syb, mut srg2, mut syb2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for c in &p.rgb {
        let (r, g, b) = (c[0] * 255.0, c[1] * 255.0, c[2] * 255.0);
        let rg = r - g;
        let yb = 0.5 * (r + g) - b;
        srg += rg;
        syb += yb;
        srg2 += rg * rg;
        syb2 += yb * yb;
    }
    let (mrg, myb) = (srg / n, syb / n);
    let var_rg = (srg2 / n - mrg * mrg).max(0.0);
    let var_yb = (syb2 / n - myb * myb).max(0.0);
    (var_rg + var_yb).sqrt() + 0.3 * (mrg * mrg + myb * myb).sqrt()
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB (0..1, gamma-encoded) → OKLab.
pub fn srgb_to_oklab(c: [f32; 3]) -> [f32; 3] {
    let (r, g, b) = (
        srgb_to_linear(c[0]),
        srgb_to_linear(c[1]),
        srgb_to_linear(c[2]),
    );
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// OKLab → sRGB (0..1, gamma-encoded, clamped).
pub fn oklab_to_srgb(lab: [f32; 3]) -> [f32; 3] {
    let l = lab[0] + 0.396_337_78 * lab[1] + 0.215_803_76 * lab[2];
    let m = lab[0] - 0.105_561_346 * lab[1] - 0.063_854_17 * lab[2];
    let s = lab[0] - 0.089_484_18 * lab[1] - 1.291_485_5 * lab[2];
    let (l, m, s) = (l * l * l, m * m * m, s * s * s);
    [
        linear_to_srgb(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        linear_to_srgb(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
        linear_to_srgb(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
    ]
}

pub fn to_u8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// k-means in OKLab on up to ~4k sampled pixels. Returns swatches sorted by weight (desc) and
/// the weighted mean distance of pixels to their centroid ("spread": low = coherent palette).
/// Deterministic: k-means++-style farthest-point seeding, no randomness.
pub fn palette(p: &Prepared, k: usize) -> (Vec<Swatch>, f32) {
    let step = (p.rgb.len() / 4096).max(1);
    let pts: Vec<[f32; 3]> = p
        .rgb
        .iter()
        .step_by(step)
        .map(|&c| srgb_to_oklab(c))
        .collect();
    if pts.is_empty() || k == 0 {
        return (Vec::new(), 0.0);
    }
    let k = k.min(pts.len());

    // Seeding: start at the mean, then repeatedly take the farthest point.
    let mean = pts
        .iter()
        .fold([0.0; 3], |a, c| [a[0] + c[0], a[1] + c[1], a[2] + c[2]]);
    let n = pts.len() as f32;
    let mut centers = vec![[mean[0] / n, mean[1] / n, mean[2] / n]];
    while centers.len() < k {
        let far = pts
            .iter()
            .max_by(|a, b| {
                let da = centers
                    .iter()
                    .map(|c| dist2(**a, *c))
                    .fold(f32::MAX, f32::min);
                let db = centers
                    .iter()
                    .map(|c| dist2(**b, *c))
                    .fold(f32::MAX, f32::min);
                da.total_cmp(&db)
            })
            .copied()
            .unwrap();
        if centers.iter().any(|c| dist2(*c, far) < 1e-8) {
            break; // fewer distinct colours than k
        }
        centers.push(far);
    }

    let mut assign = vec![0usize; pts.len()];
    for _ in 0..12 {
        let mut changed = false;
        for (i, pt) in pts.iter().enumerate() {
            let best = (0..centers.len())
                .min_by(|&a, &b| dist2(*pt, centers[a]).total_cmp(&dist2(*pt, centers[b])))
                .unwrap();
            if assign[i] != best {
                assign[i] = best;
                changed = true;
            }
        }
        let mut sums = vec![[0.0f32; 4]; centers.len()];
        for (pt, &a) in pts.iter().zip(&assign) {
            sums[a][0] += pt[0];
            sums[a][1] += pt[1];
            sums[a][2] += pt[2];
            sums[a][3] += 1.0;
        }
        for (c, s) in centers.iter_mut().zip(&sums) {
            if s[3] > 0.0 {
                *c = [s[0] / s[3], s[1] / s[3], s[2] / s[3]];
            }
        }
        if !changed {
            break;
        }
    }

    let mut counts = vec![0usize; centers.len()];
    let mut spread = 0.0f32;
    for (pt, &a) in pts.iter().zip(&assign) {
        counts[a] += 1;
        spread += dist2(*pt, centers[a]).sqrt();
    }
    let mut swatches: Vec<Swatch> = centers
        .iter()
        .zip(&counts)
        .filter(|(_, &c)| c > 0)
        .map(|(c, &cnt)| Swatch {
            rgb: to_u8(oklab_to_srgb(*c)),
            weight: cnt as f32 / n,
        })
        .collect();
    swatches.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    (swatches, spread / n)
}

#[cfg(test)]
mod tests {
    use super::super::testimg::*;
    use super::super::{Prepared, ANALYSIS_SIDE};
    use super::*;

    #[test]
    fn oklab_round_trip() {
        for c in [
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.8, 0.2, 0.4],
            [0.1, 0.5, 0.9],
        ] {
            let back = oklab_to_srgb(srgb_to_oklab(c));
            for i in 0..3 {
                assert!((back[i] - c[i]).abs() < 2e-3, "{c:?} → {back:?}");
            }
        }
        let white = srgb_to_oklab([1.0, 1.0, 1.0]);
        assert!((white[0] - 1.0).abs() < 1e-3 && white[1].abs() < 1e-3);
    }

    #[test]
    fn grey_is_not_colourful() {
        let p = Prepared::from_image(&solid(64, 64, [128, 128, 128]), ANALYSIS_SIDE);
        assert!(colorfulness(&p) < 1.0);
        let stripes = from_fn(
            64,
            64,
            |x, _| if x % 2 == 0 { [255, 0, 0] } else { [0, 0, 255] },
        );
        let p = Prepared::from_image(&stripes, ANALYSIS_SIDE);
        assert!(colorfulness(&p) > 100.0, "{}", colorfulness(&p));
    }

    #[test]
    fn palette_finds_two_colours() {
        let img = from_fn(100, 100, |x, _| {
            if x < 70 {
                [20, 40, 120]
            } else {
                [240, 200, 40]
            }
        });
        let p = Prepared::from_image(&img, ANALYSIS_SIDE);
        let (sw, spread) = palette(&p, 5);
        assert_eq!(sw.len(), 2, "{sw:?}");
        assert_eq!(sw[0].rgb, [20, 40, 120]);
        assert!((sw[0].weight - 0.7).abs() < 0.02);
        assert_eq!(sw[1].hex(), "#f0c828");
        assert!(spread < 1e-3);
    }
}
