//! Glue: candidates → previews → analysis → rejects → ranking → final render.
//!
//! Used by the CLI demo, the Tauri commands and (through JNI) the Android background worker.

use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};

use crate::analysis::Analysis;
use crate::error::{Error, Result};
use crate::http::{HttpClient, MAX_IMAGE_BYTES};
use crate::score::{self, Features, Model, Reject, ToneFilter};
use crate::source::{Candidate, Source};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
}

impl Screen {
    pub fn parse(s: &str) -> Option<Self> {
        let (w, h) = s.split_once(['x', 'X', '×'])?;
        let (width, height) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
        (width > 0 && height > 0 && width <= 16384 && height <= 16384)
            .then_some(Self { width, height })
    }

    pub fn as_tuple(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// A candidate after analysis: either rejected (with the reason) or scored.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evaluated {
    pub candidate: Candidate,
    pub analysis: Analysis,
    pub features: Features,
    pub verdict: std::result::Result<f32, Reject>,
}

pub fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| Error::Image(e.to_string()))?;
    // Guard against decompression bombs.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_000);
    limits.max_image_height = Some(16_000);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|e| Error::Image(e.to_string()))
}

/// Download the preview and analyse it.
pub fn analyse(http: &dyn HttpClient, c: &Candidate, screen: Screen) -> Result<Analysis> {
    let bytes = http.get(&c.preview_url, MAX_IMAGE_BYTES)?;
    let img = decode(&bytes)?;
    Ok(Analysis::of(&img, screen.width, screen.height))
}

type AnalysisOutcome = (Vec<(Candidate, Analysis)>, Vec<(String, Error)>);

/// Analyse candidates on `threads` worker threads, keeping input order. Failures are
/// returned separately as `(candidate id, error)`: one bad download must not sink the batch.
pub fn analyse_all(
    http: &dyn HttpClient,
    candidates: Vec<Candidate>,
    screen: Screen,
    threads: usize,
) -> AnalysisOutcome {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    let threads = threads.clamp(1, 8);
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<Result<Analysis>>>> =
        candidates.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(c) = candidates.get(i) else { break };
                *results[i].lock().unwrap() = Some(analyse(http, c, screen));
            });
        }
    });
    let mut ok = Vec::new();
    let mut failed = Vec::new();
    for (c, r) in candidates.into_iter().zip(results) {
        match r.into_inner().unwrap() {
            Some(Ok(a)) => ok.push((c, a)),
            Some(Err(e)) => failed.push((c.id, e)),
            None => {}
        }
    }
    (ok, failed)
}

pub fn evaluate(
    analysed: Vec<(Candidate, Analysis)>,
    model: &Model,
    screen: Screen,
    tone: ToneFilter,
    recent_hashes: &[u64],
    allow_upscale: bool,
) -> Vec<Evaluated> {
    analysed
        .into_iter()
        .map(|(candidate, analysis)| {
            let features = Features::of(&candidate, &analysis);
            let verdict = score::check(
                &candidate,
                &analysis,
                screen.as_tuple(),
                tone,
                recent_hashes,
                allow_upscale,
            )
            .map(|()| model.predict(&features));
            Evaluated {
                candidate,
                analysis,
                features,
                verdict,
            }
        })
        .collect()
}

/// ε-greedy pick among the accepted candidates; returns an index into `evaluated`.
pub fn choose(evaluated: &[Evaluated], epsilon: f32, rng: &mut fastrand::Rng) -> Option<usize> {
    let accepted: Vec<(usize, f32)> = evaluated
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.verdict.ok().map(|s| (i, s)))
        .collect();
    let scores: Vec<f32> = accepted.iter().map(|&(_, s)| s).collect();
    score::choose(&scores, epsilon, rng).map(|k| accepted[k].0)
}

/// Download the right-sized image, crop, scale to the exact screen size and encode JPEG.
pub fn render(
    http: &dyn HttpClient,
    source: &dyn Source,
    e: &Evaluated,
    screen: Screen,
) -> Result<Vec<u8>> {
    let crop = e.analysis.crop;
    // Size of the full image needed so that the crop alone covers the screen.
    let need_w = (screen.width as f32 / crop.w).ceil() as u32;
    let need_h = (screen.height as f32 / crop.h).ceil() as u32;
    let url = source.sized_url(&e.candidate, need_w, need_h);
    let img = decode(&http.get(&url, MAX_IMAGE_BYTES)?)?;
    let (w, h) = img.dimensions();
    let (x0, y0, x1, y1) = crop.to_pixels(w as usize, h as usize);
    let cropped = img.crop_imm(x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32);
    let fitted = cropped.resize_to_fill(screen.width, screen.height, FilterType::Lanczos3);
    let mut out = Vec::with_capacity((screen.width * screen.height / 4) as usize);
    JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&fitted.to_rgb8())
        .map_err(|e| Error::Image(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_parsing() {
        assert_eq!(
            Screen::parse("1080x2400"),
            Some(Screen {
                width: 1080,
                height: 2400
            })
        );
        assert_eq!(
            Screen::parse("1080 × 2400"),
            Some(Screen {
                width: 1080,
                height: 2400
            })
        );
        assert_eq!(Screen::parse("0x10"), None);
        assert_eq!(Screen::parse("big"), None);
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode(b"not an image").is_err());
    }
}
