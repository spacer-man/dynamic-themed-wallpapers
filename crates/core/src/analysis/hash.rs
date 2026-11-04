//! Perceptual difference hash: robust to resizing and recompression, so the same photo seen
//! via two different URLs (or as a near-identical frame from the same shoot) is caught.

use image::{imageops::FilterType, DynamicImage};

pub fn dhash(img: &DynamicImage) -> u64 {
    let small = img.resize_exact(9, 8, FilterType::Triangle).to_luma8();
    let mut h = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            let l = small.get_pixel(x, y)[0];
            let r = small.get_pixel(x + 1, y)[0];
            h = (h << 1) | u64::from(l > r);
        }
    }
    h
}

pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Two hashes this close are treated as the same picture.
pub const NEAR_DUPLICATE_BITS: u32 = 8;

pub fn is_near_duplicate(a: u64, b: u64) -> bool {
    hamming(a, b) <= NEAR_DUPLICATE_BITS
}

#[cfg(test)]
mod tests {
    use super::super::testimg::*;
    use super::*;

    #[test]
    fn resized_copy_matches_and_other_image_does_not() {
        let a = from_fn(800, 600, |x, y| {
            let v = ((x as f32 / 40.0).sin() * 60.0 + (y as f32 / 25.0).cos() * 60.0 + 128.0) as u8;
            [v, v / 2, 255 - v]
        });
        let a_small = a.resize_exact(400, 300, FilterType::Triangle);
        let b = from_fn(800, 600, |x, y| {
            let v = ((x as f32 / 13.0).cos() * 90.0 + (y as f32 / 70.0).sin() * 30.0 + 128.0) as u8;
            [v, v, v]
        });
        assert!(is_near_duplicate(dhash(&a), dhash(&a_small)));
        assert!(
            !is_near_duplicate(dhash(&a), dhash(&b)),
            "{}",
            hamming(dhash(&a), dhash(&b))
        );
    }
}
