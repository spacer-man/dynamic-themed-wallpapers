//! Pinterest boards: no key, no login, no search by words. A board is read from the public JSON
//! endpoint behind Pinterest's own board embed widget (`widgets.pinterest.com/v3/pidgets/…`);
//! the RSS feed of the board (`…/<user>/<board>.rss`) is the fallback, because it answered 404 on
//! a real device. Both hold only the latest ~25 pins of the board. See [`fetch_board`].
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
use serde::Deserialize;

use super::{Candidate, CandidateRequest, Curation, License, Search, Source, SourceKind};
use crate::error::{Error, Result};
use crate::http::{HttpClient, MAX_JSON_BYTES};
use crate::text::truncate;

const HOST: &str = "https://www.pinterest.com";
const WIDGET_HOST: &str = "https://widgets.pinterest.com";
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
    /// `https://www.pinterest.com/<user>/<board>.rss` — the saved identity of the board.
    pub rss_url: String,
    /// `https://widgets.pinterest.com/v3/pidgets/boards/<user>/<board>/pins/` — read first.
    pub widget_url: String,
}

/// Accepts `pinterest.com/<user>/<board>/`, with or without scheme, `www.` or a country
/// domain/subdomain (`ru.pinterest.com`, `pinterest.co.uk`, `pinterest.de`…), a trailing section
/// or query, or the `.rss` URL itself. Names may be percent-encoded or not (`обои` and
/// `%D0%BE%D0%B1%D0%BE%D0%B8` are the same board). Short `pin.it` links need a request, see
/// [`resolve_board_url`].
pub fn parse_board_url(input: &str) -> Result<BoardRef> {
    let bad = |why: &str| Error::Config(format!("not a Pinterest board address: {why}"));
    let s = input.trim();
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_ascii_lowercase();
    if is_short_host(&host) {
        return Err(bad("short pin.it links have to be opened first"));
    }
    if !is_pinterest_host(&host) {
        return Err(bad("the host is not pinterest"));
    }
    let path = path.split(['?', '#']).next().unwrap_or("");
    let mut segs = path.split('/').filter(|p| !p.is_empty());
    let user = segs.next().ok_or_else(|| bad("a board has /user/board"))?;
    let board = segs.next().ok_or_else(|| bad("a board has /user/board"))?;
    let board = board.strip_suffix(".rss").unwrap_or(board);
    let decode = |s: &str| {
        percent_encoding::percent_decode_str(s)
            .decode_utf8()
            .map(|d| d.into_owned())
            .map_err(|_| bad("unexpected characters"))
    };
    let (user, board) = (decode(user)?, decode(board)?);
    if RESERVED.contains(&user.to_lowercase().as_str()) || board.is_empty() {
        return Err(bad("that looks like a pin or a page, not a board"));
    }
    let safe = |s: &str| {
        s.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
    };
    if !safe(&user) || !safe(&board) {
        return Err(bad("unexpected characters"));
    }
    let rss_url = format!(
        "{HOST}/{}/{}.rss",
        encode_segment(&user),
        encode_segment(&board)
    );
    let widget_url = format!(
        "{WIDGET_HOST}/v3/pidgets/boards/{}/{}/pins/",
        encode_segment(&user),
        encode_segment(&board)
    );
    Ok(BoardRef {
        user,
        board,
        rss_url,
        widget_url,
    })
}

/// Percent-encodes everything but ASCII letters, digits and `-_.` — one canonical form, so the
/// same board pasted encoded or not is saved once.
fn encode_segment(s: &str) -> String {
    const SET: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.');
    percent_encoding::utf8_percent_encode(s, SET).to_string()
}

fn is_short_host(host: &str) -> bool {
    host == "pin.it" || host.ends_with(".pin.it")
}

/// Hops through at most this many redirects of a short link.
const MAX_HOPS: usize = 5;

/// Like [`parse_board_url`], but also opens short `pin.it` links (the "Share" button of the
/// Pinterest app gives those), one redirect at a time, until a board address shows up.
pub fn resolve_board_url(http: &dyn HttpClient, input: &str) -> Result<BoardRef> {
    let s = input.trim();
    let host = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !is_short_host(&host) {
        return parse_board_url(s);
    }
    let mut url = if s.contains("://") {
        s.to_owned()
    } else {
        format!("https://{s}")
    };
    for _ in 0..MAX_HOPS {
        let hop = http.redirect_target(&url).map_err(|e| match e {
            Error::Network(m) => Error::Network(format!("opening {url}: {m}")),
            e => e,
        })?;
        match hop {
            Some(next) => {
                if let Ok(board) = parse_board_url(&next) {
                    return Ok(board);
                }
                url = next;
            }
            None => break,
        }
    }
    Err(Error::Config(
        "this short link does not lead to a Pinterest board (a single pin?)".into(),
    ))
}

/// `pinterest.<tld>`, optionally `<tld2>` (`co.uk`, `com.au`) and one subdomain: `www` or a
/// two-letter country (`ru`). Others (`api.` of the short-link redirect) are not board pages.
fn is_pinterest_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    let Some(i) = labels.iter().position(|l| *l == "pinterest") else {
        return false;
    };
    let sub_ok = i == 0 || (i == 1 && (labels[0] == "www" || labels[0].len() == 2));
    sub_ok
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

/// The JSON of the board embed widget. Everything is optional, unknown fields are ignored.
#[derive(Debug, Default, Deserialize)]
struct WidgetReply {
    status: Option<String>,
    data: Option<WidgetData>,
}

#[derive(Debug, Default, Deserialize)]
struct WidgetData {
    pins: Option<Vec<WidgetPin>>,
    board: Option<WidgetBoard>,
}

#[derive(Debug, Default, Deserialize)]
struct WidgetBoard {
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct WidgetPin {
    id: Option<serde_json::Value>,
    description: Option<String>,
    is_video: Option<bool>,
    images: Option<HashMap<String, WidgetImage>>,
    pinner: Option<WidgetPinner>,
}

#[derive(Debug, Default, Deserialize)]
struct WidgetImage {
    url: Option<String>,
    width: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct WidgetPinner {
    full_name: Option<String>,
    username: Option<String>,
}

/// Parse the answer of the board widget endpoint into the same [`Feed`] the RSS gives. Videos and
/// pins without an image are skipped; `status` other than `success` is a board that was not found.
pub fn parse_widget(json: &[u8], url: &str) -> Result<Feed> {
    let reply: WidgetReply =
        serde_json::from_slice(json).map_err(|e| Error::parse("pinterest widget", e))?;
    if reply.status.as_deref().is_some_and(|s| s != "success") {
        return Err(Error::Status {
            status: 404,
            url: url.to_owned(),
        });
    }
    let data = reply.data.unwrap_or_default();
    let title = data
        .board
        .and_then(|b| b.name)
        .map(|n| n.trim().to_owned())
        .unwrap_or_default();
    let mut pins = Vec::new();
    for p in data.pins.unwrap_or_default() {
        if p.is_video == Some(true) {
            continue;
        }
        let id = match &p.id {
            Some(serde_json::Value::String(s)) => s.trim().to_owned(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        // The largest rendition; the width falls back to the key (`564x`).
        let image = p
            .images
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, i)| {
                let url = i.url.filter(|u| !u.is_empty())?;
                let w = i
                    .width
                    .or_else(|| k.trim_end_matches('x').parse().ok())
                    .unwrap_or(0);
                Some((w, k, url))
            })
            .max()
            .map(|(_, _, u)| u);
        let Some(image) = image else { continue };
        let author = p.pinner.and_then(|n| {
            [n.full_name, n.username]
                .into_iter()
                .flatten()
                .map(|s| s.trim().to_owned())
                .find(|s| !s.is_empty())
        });
        pins.push(Pin {
            title: p.description.unwrap_or_default().trim().to_owned(),
            link: if id.is_empty() {
                String::new()
            } else {
                format!("{HOST}/pin/{id}/")
            },
            author,
            image,
        });
    }
    Ok(Feed { title, pins })
}

fn is_not_found(e: &Error) -> bool {
    matches!(e, Error::Status { status: 404, .. } | Error::NotFound(_))
}

/// Read a board: the widget JSON first; on any error or no usable pin, the RSS feed. If both fail
/// the widget's error is returned (it is the more meaningful one), except that two 404s become
/// "not found". A widget answer without pins is returned as is when the RSS has none either.
pub fn fetch_board(http: &dyn HttpClient, board: &BoardRef) -> Result<Feed> {
    let widget = http
        .get(&board.widget_url, MAX_JSON_BYTES)
        .and_then(|body| parse_widget(&body, &board.widget_url));
    if let Ok(feed) = &widget {
        if !feed.pins.is_empty() {
            return widget;
        }
    }
    let rss = http
        .get(&board.rss_url, MAX_JSON_BYTES)
        .and_then(|body| parse_feed(&body));
    match (widget, rss) {
        (_, Ok(feed)) if !feed.pins.is_empty() => Ok(feed),
        (Ok(feed), _) => Ok(feed),
        (Err(w), Err(r)) if is_not_found(&w) && is_not_found(&r) => Err(Error::NotFound(
            "Pinterest did not find this board; is it public?".into(),
        )),
        (Err(_), Ok(feed)) => Ok(feed),
        (Err(w), Err(_)) => Err(w),
    }
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
        let feed = fetch_board(self.http, &parse_board_url(url)?)?;
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
            "https://api.pinterest.com/url_shortener/abc/redirect/",
            "https://pin.it/abc123",
            "https://pinterest.com/alice/bo ard/",
            "https://pinterest.com/alice/bo%20ard/",
            "https://pinterest.com/alice/%FF/",
            "",
        ] {
            assert!(parse_board_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn cyrillic_boards_have_one_form() {
        let want = "https://www.pinterest.com/alice/%D0%BE%D0%B1%D0%BE%D0%B8.rss";
        for input in [
            "https://ru.pinterest.com/alice/%D0%BE%D0%B1%D0%BE%D0%B8/?invite_code=0000&sender=1",
            "https://ru.pinterest.com/alice/%d0%be%d0%b1%d0%be%d0%b8/",
            "ru.pinterest.com/alice/обои/",
        ] {
            let b = parse_board_url(input).unwrap();
            assert_eq!(b.rss_url, want, "{input}");
            assert_eq!(b.board, "обои");
        }
    }

    #[test]
    fn short_links_are_opened() {
        use crate::http::fixture::FixtureHttp;
        let board = "https://ru.pinterest.com/alice/%D0%BE%D0%B1%D0%BE%D0%B8/?invite_code=x";
        let http = FixtureHttp::new()
            .redirect(
                "https://pin.it/AbCd123",
                "https://api.pinterest.com/url_shortener/AbCd123/redirect/",
            )
            .redirect("https://api.pinterest.com/url_shortener/AbCd123/", board);
        let b = resolve_board_url(&http, " pin.it/AbCd123 ").unwrap();
        assert_eq!(b.board, "обои");
        assert_eq!(http.requests().len(), 2);

        let to_pin =
            FixtureHttp::new().redirect("https://pin.it/p", "https://www.pinterest.com/pin/123/");
        assert!(resolve_board_url(&to_pin, "https://pin.it/p").is_err());
        let nowhere = FixtureHttp::new();
        assert!(resolve_board_url(&nowhere, "https://pin.it/x").is_err());
        // Ordinary addresses make no request.
        let none = FixtureHttp::new();
        resolve_board_url(&none, "pinterest.com/alice/misty-forests").unwrap();
        assert!(none.requests().is_empty());
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

    fn widget_fixture() -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/pinterest/board.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn widget_urls_are_built_like_the_rss_one() {
        let b = parse_board_url("ru.pinterest.com/alice/обои/").unwrap();
        assert_eq!(
            b.widget_url,
            "https://widgets.pinterest.com/v3/pidgets/boards/alice/%D0%BE%D0%B1%D0%BE%D0%B8/pins/"
        );
    }

    #[test]
    fn widget_json_is_parsed() {
        let feed = parse_widget(&widget_fixture(), "u").unwrap();
        assert_eq!(feed.title, "Туманные леса");
        // The video and the pin without images are dropped.
        assert_eq!(feed.pins.len(), 3);
        assert_eq!(feed.pins[0].title, "Pine fog");
        assert_eq!(feed.pins[0].author.as_deref(), Some("Bob Photographer"));
        assert_eq!(
            feed.pins[0].link,
            "https://www.pinterest.com/pin/111111111111111/"
        );
        assert_eq!(
            feed.pins[0].image,
            "https://i.pinimg.com/564x/aa/bb/cc/aaaa1111.jpg"
        );
        assert_eq!(feed.pins[1].author.as_deref(), Some("carol_makes"));
        assert_eq!(
            feed.pins[1].link,
            "https://www.pinterest.com/pin/222222222222222/"
        );
        assert_eq!(
            feed.pins[2].link,
            "https://www.pinterest.com/pin/333333333333333/"
        );

        assert!(parse_widget(br#"{"status":"success"}"#, "u")
            .unwrap()
            .pins
            .is_empty());
        assert!(
            parse_widget(br#"{"status":"success","data":{"pins":null}}"#, "u")
                .unwrap()
                .pins
                .is_empty()
        );
        let e = parse_widget(br#"{"status":"failure","message":"x"}"#, "u").unwrap_err();
        assert!(matches!(e, Error::Status { status: 404, .. }));
        assert!(parse_widget(b"<html>", "u").is_err());
    }

    #[test]
    fn widget_first_then_rss_then_not_found() {
        use crate::http::fixture::FixtureHttp;
        let board = parse_board_url("pinterest.com/alice/misty-forests").unwrap();
        let rss = std::fs::read(format!(
            "{}/tests/fixtures/pinterest/board.rss",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();

        // Widget answers: RSS is not asked.
        let http = FixtureHttp::new()
            .route(&["pidgets/boards/alice/misty-forests"], widget_fixture())
            .route(&["misty-forests.rss"], rss.clone());
        assert_eq!(fetch_board(&http, &board).unwrap().title, "Туманные леса");
        assert_eq!(http.requests().len(), 1);

        // Widget missing: RSS is used.
        let http = FixtureHttp::new().route(&["misty-forests.rss"], rss.clone());
        assert_eq!(
            fetch_board(&http, &board).unwrap().title,
            "Misty & quiet forests"
        );
        assert_eq!(http.requests().len(), 2);

        // Widget without pins: RSS is used.
        let http = FixtureHttp::new()
            .route(&["pidgets"], r#"{"status":"success","data":{"pins":[]}}"#)
            .route(&["misty-forests.rss"], rss);
        assert_eq!(fetch_board(&http, &board).unwrap().pins.len(), 4);

        // Both 404.
        let http = FixtureHttp::new();
        let e = fetch_board(&http, &board).unwrap_err();
        assert!(
            matches!(&e, Error::NotFound(m) if m == "Pinterest did not find this board; is it public?"),
            "{e}"
        );

        // Widget has a real error, RSS 404: the widget's error wins.
        let http = FixtureHttp::new().route(&["pidgets"], "not json");
        assert!(matches!(
            fetch_board(&http, &board).unwrap_err(),
            Error::Parse { .. }
        ));
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
