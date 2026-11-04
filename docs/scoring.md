# How "beautiful" is decided

Three layers, coarse to fine. Code: `crates/core/src/analysis/` (measurements) and `crates/core/src/score.rs`
(decisions).

## 1. Curation

Sources only return curated photos (Commons Featured / Quality / Valued, Unsplash with likes). This does most of the
work. `curation` feature: featured 1.0, quality 0.7, valued 0.5; Unsplash `log10(likes+1)/3` capped at 1.

## 2. Measurements

All measured on a working copy with long side ≤ 512 px (from the ~960 px preview), **after** planning the crop — we
judge what will actually be on screen.

| Measurement | How |
|---|---|
| crop | slide a screen-shaped window along the long axis, maximise Sobel energy share − 0.08 × distance from centre |
| tone | luma histogram: **dark** if median ≤ 0.30 and p75 ≤ 0.50; **light** if median ≥ 0.60 and p25 ≥ 0.40; else mid |
| contrast | σ of luma (0..0.5) |
| colourfulness | Hasler–Süsstrunk, 0..255 scale |
| sharpness | variance of 4-neighbour Laplacian (luma 0..255) on the central 60% |
| status-bar calm | mean Sobel magnitude in the top 7% of the cropped frame |
| dock calm | same for 84–96% of height |
| clipping | share of pixels with luma ≥ 0.98 / ≤ 0.02 |
| entropy | Shannon entropy of a 64-bin luma histogram (bits) |
| palette | k-means (k=5) in OKLab on ≤ 4k pixels, deterministic farthest-point seeding; spread = mean distance to centroid |
| dHash | 9×8 difference hash of the whole photo |

## 3. Hard rejects

| Reject | Rule |
|---|---|
| wrong tone | tone filter is dark/light and the photo isn't |
| too small | cropped area would be upscaled > 1.25× to cover the screen |
| excessive crop | less than 20% of the photo's area survives |
| blurry | Laplacian variance < 12 |
| blown highlights | > 20% of pixels clipped white |
| crushed shadows | > 60% clipped black (night photos are fine; pure black frames are not) |
| duplicate | dHash within 8 bits of a recently shown photo |

## 4. Ranking

Ten features, each mapped to 0..1 (higher = better):

| Feature | Mapping | Prior weight |
|---|---|---|
| curation | see above | 2.0 |
| sharpness | ln(1+var) / ln(1001) | 1.0 |
| contrast | σ / 0.25 | 0.6 |
| colorfulness | C / 100 | 0.6 |
| top_band_calmness | 1 − busy / 0.12 | 0.8 |
| dock_calmness | 1 − busy / 0.12 | 0.4 |
| crop_keep | (keep − 0.2) / 0.8 | 0.6 |
| palette_coherence | 1 − spread / 0.15 | 0.4 |
| entropy | bits / 6 | 0.3 |
| exposure | 1 − (4·highlights + shadows) | 0.8 |

Bias −3.0. `p(like) = σ(w·x + b)`.

**Learning.** Like = 1, skip = 0, one SGD step (lr 0.08) per event. L2 regularisation (0.02) pulls weights towards the
*prior* rather than zero, so a few clicks adjust taste without erasing what curation knows. Weights live in the local
store; nothing leaves the device.

**Selection.** ε-greedy with ε = 0.15: usually the top-scored photo, sometimes a random one from the top half — enough
exploration for the model to learn, never the worst candidates.

## Calibration status

Thresholds are first guesses checked on synthetic images (unit tests). They are to be re-checked on real photos with
the CLI demo (`examples/pick.rs`, prints every measurement) — see the "Calibration" section below once filled in.
