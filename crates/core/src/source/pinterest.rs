//! Pinterest boards, through the public RSS feed of a board (`…/<user>/<board>.rss`): no key, no
//! login, no search by words. The feed holds only the latest ~25 pins of the board.
//!
//! Dimensions are not in the feed, so each pin's image is probed with a ranged download of its
//! first bytes (the image header is enough). The wallpaper uses the `originals` rendition and
//! anything that does not cover the screen is dropped — no upscaling. Originals that cannot be
//! fetched or read (404, WebP/GIF, which the decoder does not handle) fall back to `/1200x/`,
//! then `/736x/`. `/736x/` is also the preview used for scoring.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Mutex;

use quick_xml::events::Event;
use quick_xml::Reader;

use super::{Candidate, CandidateRequest, Curation, License, Search, Source, SourceKind};
use crate::error::{Error, Result};
use crate::http::{HttpClient, MAX_JSON_BYTES};
use crate::text::truncate;

const HOST: &str = "https://www.pinterest.com";
const IMAGE_HOST: &str = "https://i.pinimg.com";
/// Enough for the dimensions of any JPEG/PNG whose header carries a few KB of EXIF/ICC data.
const HEADER_BYTES: usize = 64 * 1024;
const PROBE_THREADS: usize = 6;
const MAX_PINS: usize = 50;

/// Where the wallpaper is fetched from, best first.
const SIZES: [&str; 3] = ["originals", "1200x", "736x"];
const PREVIEW_SIZE: &str = "736x";

/// First path segments that are Pinterest pages, not users.
const RESERVED: [&str; 14] = [
    "pin", "search", "ideas", "today", "explore", "settings", "business", "login", "news_hub",
    "videos", "topics", "_", "about", "help",
];

/// A board as the user pasted it, normalised.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardRef {
    pub user: String,
    pub board: String,
    /// `https://www.pinterest.com/<user>/<board>.rss`
    pub rss_url: String,
}

/// Accepts `pinterest.com/<user>/<board>/`, with or without scheme, `www.` or a country
/// domain/subdomain (`ru.pinterest.com`, `pinterest.co.uk`, `pinterest.de`…), a trailing section
/// or query, or the `.rss` URL itself. Short `pin.it` links are not followed (that needs a
/// request and often leads to a single pin): the error asks for the full address.
pub fn parse_board_url(input: &str) -> Result<BoardRef> {
    let bad = |why: &str| Error::Config(format!("not a Pinterest board address: {why}"));
    let s = input.trim();
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_ascii_lowercase();
    if host == "pin.it" || host.ends_with(".pin.it") {
        return Err(bad(
            "short pin.it links are not followed, open it and paste the address of the board",
        ));
    }
    if !is_pinterest_host(&host) {
        return Err(bad("the host is not pinterest"));
    }
    let path = path.split(['?', '#']).next().unwrap_or("");
    let mut segs = path.split('/').filter(|p| !p.is_empty());
    let user = segs.next().ok_or_else(|| bad("a board has /user/board"))?;
    let board = segs.next().ok_or_else(|| bad("a board has /user/board"))?;
    let board = board.strip_suffix(".rss").unwrap_or(board);
    if RESERVED.contains(&user.to_ascii_lowercase().as_str()) || board.is_empty() {
        return Err(bad("that looks like a pin or a page, not a board"));
    }
    let safe = |s: &str| {
        s.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '%' | '+'))
    };
    if !safe(user) || !safe(board) {
        return Err(bad("unexpected characters"));
    }
    Ok(BoardRef {
        user: user.to_owned(),
        board: board.to_owned(),
        rss_url: format!("{HOST}/{user}/{board}.rss"),
    })
}

/// `pinterest.<tld>`, optionally `<tld2>` (`co.uk`, `com.au`) and one subdomain (`www`, `ru`).
fn is_pinterest_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    let Some(i) = labels.iter().position(|l| *l == "pinterest") else {
        return false;
    };
    i <= 1
        && (1..=2).contains(&(labels.len() - i - 1))
        && labels[i + 1..].iter().all(|l| l.len() <= 3)
}

/// One pin of a feed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pin {
    pub title: String,
    pub link: String,
    pub author: Option<String>,
    /// Image URL found in the description (any size).
    pub image: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Feed {
    /// Board name (channel title).
    pub title: String,
    pub pins: Vec<Pin>,
}

fn unescape_ref(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)?
        }
    })
}

/// First `src="…pinimg.com…"` of an HTML fragment.
fn image_in_html(html: &str) -> Option<String> {
    let mut rest = html;
    while let Some(i) = rest.find("src=") {
        rest = &rest[i + 4..];
        let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let body = &rest[1..];
        let end = body.find(quote)?;
        let url = &body[..end];
        if url.contains("pinimg.com") {
            return Some(url.to_owned());
        }
        rest = &body[end..];
    }
    None
}

/// Parse a board's RSS feed. Pins without a usable image are skipped.
pub fn parse_feed(xml: &[u8]) -> Result<Feed> {
    let mut reader = Reader::from_reader(xml);
    let config = reader.config_mut();
    config.check_end_names = false;
    config.allow_unmatched_ends = true;

    let mut feed = Feed::default();
    let mut item: Option<Pin> = None;
    let mut desc = String::new();
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|e| Error::parse("pinterest feed", e))?;
        match ev {
            Event::Start(e) => {
                let name = e.name().as_ref().to_owned();
                if name == "item" {
                    item = Some(Pin::default());
                    desc.clear();
                }
                text.clear();
                path.push(name);
            }
            Event::Empty(e) => {
                // Media attributes as a fallback image source.
                let name = e.name().as_ref().to_owned();
                if let Some(pin) = item.as_mut() {
                    if matches!(
                        name.as_str(),
                        "enclosure" | "media:content" | "media:thumbnail"
                    ) && pin.image.is_empty()
                    {
                        for a in e.attributes().flatten() {
                            let v = a.value.to_string();
                            if a.key.as_ref() == "url" && v.contains("pinimg.com") {
                                pin.image = v;
                            }
                        }
                    }
                }
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::CData(t) => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) => {
                let name = r.xml10_content();
                if let Some(c) = unescape_ref(&name) {
                    text.push(c);
                }
            }
            Event::End(_) => {
                let Some(name) = path.pop() else { continue };
                let value = text.trim().to_owned();
                text.clear();
                match (item.as_mut(), name.as_str()) {
                    (None, "title")
                        if feed.title.is_empty() && path.last().is_some_and(|p| p == "channel") =>
                    {
                        feed.title = value;
                    }
                    (Some(pin), "title") => pin.title = value,
                    (Some(pin), "link") => pin.link = value,
                    (Some(pin), "dc:creator" | "author" | "creator") if !value.is_empty() => {
                        pin.author = Some(value);
                    }
                    (Some(_), "description") => desc = value,
                    _ => {}
                }
                if name == "item" {
                    if let Some(mut pin) = item.take() {
                        if pin.image.is_empty() {
                            pin.image = image_in_html(&desc).unwrap_or_default();
                        }
                        if !pin.image.is_empty() {
                            feed.pins.push(pin);
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(feed)
}

/// The part of an `i.pinimg.com` URL after the size: `ab/cd/ef/hash.jpg`.
fn image_path(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://i.pinimg.com/")
        .or_else(|| url.strip_prefix("http://i.pinimg.com/"))?;
    let (_size, path) = rest.split_once('/')?;
    let path = path.split(['?', '#']).next()?;
    let lower = path.to_ascii_lowercase();
    // Animated GIFs are not wallpapers.
    if path.is_empty() || lower.ends_with(".gif") || path.contains("..") {
        return None;
    }
    Some(path.to_owned())
}

fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

/// What probing a pin found: the URL that works and its size.
#[derive(Clone, Debug)]
struct Probed {
    url: String,
    width: u32,
    height: u32,
}

pub struct PinterestSource<'a> {
    http: &'a dyn HttpClient,
    /// Probe results by image path, so refill rounds of one prefetch do not probe twice.
    probed: Mutex<HashMap<String, Option<Probed>>>,
}

impl<'a> PinterestSource<'a> {
    pub fn new(http: &'a dyn HttpClient) -> Self {
        Self {
            http,
            probed: Mutex::new(HashMap::new()),
        }
    }

    /// First of originals / 1200x / 736x whose header can be read.
    fn probe(&self, path: &str) -> Option<Probed> {
        if let Some(hit) = self.probed.lock().unwrap().get(path) {
            return hit.clone();
        }
        let found = SIZES.iter().find_map(|size| {
            let url = format!("{IMAGE_HOST}/{size}/{path}");
            let head = self.http.get_prefix(&url, HEADER_BYTES).ok()?;
            let (width, height) = image_dimensions(&head)?;
            Some(Probed { url, width, height })
        });
        self.probed
            .lock()
            .unwrap()
            .insert(path.to_owned(), found.clone());
        found
    }

    fn probe_all(&self, paths: &[String]) -> Vec<Option<Probed>> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let next = AtomicUsize::new(0);
        let results: Vec<Mutex<Option<Probed>>> = paths.iter().map(|_| Mutex::new(None)).collect();
        std::thread::scope(|s| {
            for _ in 0..PROBE_THREADS.min(paths.len()) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(p) = paths.get(i) else { break };
                    *results[i].lock().unwrap() = self.probe(p);
                });
            }
        });
        results
            .into_iter()
            .map(|m| m.into_inner().unwrap())
            .collect()
    }
}

fn pin_id(link: &str, path: &str) -> String {
    link.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_owned)
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_owned())
}

impl Source for PinterestSource<'_> {
    fn kind(&self) -> SourceKind {
        SourceKind::Pinterest
    }

    fn candidates(&self, search: &Search<'_>, req: &CandidateRequest) -> Result<Vec<Candidate>> {
        let Search::Board { url, name } = search else {
            return Err(Error::Config(
                "Pinterest only serves Pinterest boards".into(),
            ));
        };
        let body = self.http.get(url, MAX_JSON_BYTES)?;
        let feed = parse_feed(&body)?;
        let label: &str = if name.is_empty() { &feed.title } else { name };
        let place_label = format!("Pinterest: {label}");

        let pins: Vec<(Pin, String)> = feed
            .pins
            .into_iter()
            .filter_map(|p| {
                let path = image_path(&p.image)?;
                Some((p, path))
            })
            .take(MAX_PINS)
            .collect();
        let paths: Vec<String> = pins.iter().map(|(_, p)| p.clone()).collect();
        let probed = self.probe_all(&paths);

        let mut out = Vec::new();
        for ((pin, path), probe) in pins.into_iter().zip(probed) {
            let Some(p) = probe else { continue };
            if !req.covers_screen(p.width, p.height) {
                continue;
            }
            let link = if pin.link.starts_with("https://") {
                pin.link.clone()
            } else {
                url.trim_end_matches(".rss").to_owned()
            };
            out.push(Candidate {
                source: SourceKind::Pinterest,
                id: format!("pinterest:{}", pin_id(&pin.link, &path)),
                title: String::new(),
                description: (!pin.title.is_empty()).then(|| truncate(&pin.title, 200)),
                page_url: link,
                image_url: p.url,
                preview_url: format!("{IMAGE_HOST}/{PREVIEW_SIZE}/{path}"),
                width: p.width,
                height: p.height,
                author: pin.author.clone().unwrap_or_else(|| "Pinterest".into()),
                author_url: None,
                license: License {
                    short_name: "No licence stated".into(),
                    url: None,
                    attribution_required: false,
                },
                curation: Curation::default(),
                coords: None,
                place_label: place_label.clone(),
            });
        }
        super::dedup(&mut out);
        out.truncate(req.limit as usize);
        Ok(out)
    }

    fn check(&self) -> Result<()> {
        self.http
            .get_quick(&format!("{HOST}/robots.txt"), &[], MAX_JSON_BYTES)
            .map(|_| ())
    }

    fn sized_url(&self, c: &Candidate, _min_width: u32, _min_height: u32) -> String {
        c.image_url.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_urls_are_normalised() {
        let want = "https://www.pinterest.com/alice/misty-forests.rss";
        for input in [
            "https://www.pinterest.com/alice/misty-forests/",
            "pinterest.com/alice/misty-forests",
            "  http://ru.pinterest.com/alice/misty-forests/?invite_code=x#y ",
            "https://www.pinterest.co.uk/alice/misty-forests/section-1/",
            "https://de.pinterest.com/alice/misty-forests.rss",
            "https://pinterest.ru/alice/misty-forests/",
        ] {
            assert_eq!(parse_board_url(input).unwrap().rss_url, want, "{input}");
        }
        for bad in [
            "https://pinterest.com/pin/123456/",
            "https://pinterest.com/alice/",
            "https://pinterest.com/",
            "https://example.com/alice/board/",
            "https://notpinterest.com/alice/board/",
            "https://evil.com/pinterest.com/alice/board/",
            "https://pin.it/abc123",
            "https://pinterest.com/alice/bo ard/",
            "",
        ] {
            assert!(parse_board_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn feed_is_parsed_with_entities_cdata_and_authors() {
        let xml = std::fs::read(format!(
            "{}/tests/fixtures/pinterest/board.rss",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let feed = parse_feed(&xml).unwrap();
        assert_eq!(feed.title, "Misty & quiet forests");
        // The text-only pin has no image and is dropped; the GIF is still listed here.
        assert_eq!(feed.pins.len(), 4);
        assert_eq!(feed.pins[0].title, "Pine fog");
        assert_eq!(feed.pins[0].author.as_deref(), Some("Bob Photographer"));
        assert_eq!(
            feed.pins[0].image,
            "https://i.pinimg.com/236x/aa/bb/cc/aaaa1111.jpg"
        );
        assert_eq!(feed.pins[1].author, None);
        assert_eq!(
            feed.pins[1].image,
            "https://i.pinimg.com/236x/dd/ee/ff/bbbb2222.jpg"
        );
        assert!(
            parse_feed(b"<rss><channel><title>x</title></channel></rss>")
                .unwrap()
                .pins
                .is_empty()
        );
    }

    #[test]
    fn image_paths() {
        assert_eq!(
            image_path("https://i.pinimg.com/236x/ab/cd/ef/hash.jpg").as_deref(),
            Some("ab/cd/ef/hash.jpg")
        );
        assert_eq!(
            image_path("https://i.pinimg.com/236x/ab/cd/ef/hash.GIF"),
            None
        );
        assert_eq!(image_path("https://evil.example/236x/ab/hash.jpg"), None);
    }
}
