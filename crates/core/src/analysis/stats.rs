//! Luma statistics: histogram, tone class, sharpness, gradients.

use super::{Prepared, Tone};

pub struct Histogram {
    bins: [u32; 256],
    total: u32,
}

impl Histogram {
    pub fn of(luma: &[f32]) -> Self {
        let mut bins = [0u32; 256];
        for &l in luma {
            bins[(l.clamp(0.0, 1.0) * 255.0).round() as usize] += 1;
        }
        Self {
            bins,
            total: luma.len() as u32,
        }
    }

    /// Luma value (0..1) below which `q` of the pixels fall.
    pub fn percentile(&self, q: f32) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        let target = (q.clamp(0.0, 1.0) * self.total as f32).ceil().max(1.0) as u32;
        let mut acc = 0;
        for (i, &n) in self.bins.iter().enumerate() {
            acc += n;
            if acc >= target {
                return i as f32 / 255.0;
            }
        }
        1.0
    }

    pub fn fraction_at_least(&self, v: f32) -> f32 {
        let from = (v * 255.0).round() as usize;
        self.fraction(from..256)
    }

    pub fn fraction_at_most(&self, v: f32) -> f32 {
        let to = (v * 255.0).round() as usize;
        self.fraction(0..to + 1)
    }

    fn fraction(&self, r: std::ops::Range<usize>) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        self.bins[r].iter().sum::<u32>() as f32 / self.total as f32
    }

    /// Shannon entropy in bits with the histogram re-binned to `bins` buckets.
    pub fn entropy_bits(&self, bins: usize) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        let per = 256 / bins;
        let mut e = 0.0f32;
        for chunk in self.bins.chunks(per) {
            let p = chunk.iter().sum::<u32>() as f32 / self.total as f32;
            if p > 0.0 {
                e -= p * p.log2();
            }
        }
        e
    }
}

/// Tone thresholds. "Dark" must be dark across most of the frame, not just on average —
/// a night skyline with a bright moon is still dark, a dim photo with a white sky is not.
pub const DARK_MEDIAN_MAX: f32 = 0.30;
pub const DARK_P75_MAX: f32 = 0.50;
pub const LIGHT_MEDIAN_MIN: f32 = 0.60;
pub const LIGHT_P25_MIN: f32 = 0.40;

pub fn classify_tone(h: &Histogram) -> Tone {
    let median = h.percentile(0.5);
    if median <= DARK_MEDIAN_MAX && h.percentile(0.75) <= DARK_P75_MAX {
        Tone::Dark
    } else if median >= LIGHT_MEDIAN_MIN && h.percentile(0.25) >= LIGHT_P25_MIN {
        Tone::Light
    } else {
        Tone::Mid
    }
}

pub fn std_dev(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let n = v.len() as f32;
    let mean = v.iter().sum::<f32>() / n;
    (v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n).sqrt()
}

/// Variance of the 4-neighbour Laplacian, luma on the 0..255 scale. Classic blur measure.
pub fn laplacian_variance(p: &Prepared) -> f32 {
    let (w, h) = (p.width, p.height);
    if w < 3 || h < 3 {
        return 0.0;
    }
    let l = |x: usize, y: usize| p.luma[y * w + x] * 255.0;
    let mut vals = Vec::with_capacity((w - 2) * (h - 2));
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            vals.push(l(x - 1, y) + l(x + 1, y) + l(x, y - 1) + l(x, y + 1) - 4.0 * l(x, y));
        }
    }
    std_dev(&vals).powi(2)
}

/// Sobel gradient magnitude per pixel (luma 0..1 scale), border pixels are 0.
pub fn gradient_map(p: &Prepared) -> Vec<f32> {
    let (w, h) = (p.width, p.height);
    let mut out = vec![0.0; w * h];
    if w < 3 || h < 3 {
        return out;
    }
    let l = |x: usize, y: usize| p.luma[y * w + x];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let gx = l(x + 1, y - 1) + 2.0 * l(x + 1, y) + l(x + 1, y + 1)
                - l(x - 1, y - 1)
                - 2.0 * l(x - 1, y)
                - l(x - 1, y + 1);
            let gy = l(x - 1, y + 1) + 2.0 * l(x, y + 1) + l(x + 1, y + 1)
                - l(x - 1, y - 1)
                - 2.0 * l(x, y - 1)
                - l(x + 1, y - 1);
            out[y * w + x] = (gx * gx + gy * gy).sqrt() / 4.0;
        }
    }
    out
}

/// Mean gradient magnitude over the interior. ~0 for a clear sky, 0.1+ for foliage or crowds.
pub fn mean_gradient(p: &Prepared) -> f32 {
    if p.width < 3 || p.height < 3 {
        return 0.0;
    }
    let g = gradient_map(p);
    let n = ((p.width - 2) * (p.height - 2)) as f32;
    g.iter().sum::<f32>() / n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles() {
        let v: Vec<f32> = (0..=100).map(|i| i as f32 / 100.0).collect();
        let h = Histogram::of(&v);
        assert!((h.percentile(0.5) - 0.5).abs() < 0.01);
        assert!((h.percentile(0.1) - 0.1).abs() < 0.01);
        assert_eq!(Histogram::of(&[]).percentile(0.5), 0.0);
    }

    #[test]
    fn entropy_bounds() {
        let flat = Histogram::of(&[0.5; 100]);
        assert_eq!(flat.entropy_bits(64), 0.0);
        let uniform: Vec<f32> = (0..256 * 4).map(|i| (i % 256) as f32 / 255.0).collect();
        assert!((Histogram::of(&uniform).entropy_bits(64) - 6.0).abs() < 0.05);
    }
}
