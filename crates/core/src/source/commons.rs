//! Wikimedia Commons: free, no key, and — most importantly — curated by humans.
//!
//! We search only within Featured pictures, Quality images and Valued images, combined with
//! a geographic filter (`nearcoord:`) or the place's own curated categories.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::{Candidate, CandidateRequest, Curation, License, Search, Source, SourceKind};
use crate::error::Result;
use crate::geo::GeoPoint;
use crate::http::{build_url, get_json, HttpClient, MAX_JSON_BYTES};
use crate::place::{Scope, SubPlace};
use crate::text::{html_to_text, truncate};

const API: &str = "https://commons.wikimedia.org/w/api.php";

const CAT_FEATURED: &str = "Featured pictures on Wikimedia Commons";
const CAT_QUALITY: &str = "Quality images";
const CAT_VALUED: &str = "Valued images sorted by promotion date";

/// Thumbnail widths Wikimedia pre-renders. Other widths may be throttled.
const THUMB_STEPS: &[u32] = &[250, 330, 500, 960, 1280, 1920, 3840];

/// Radius around each sub-place in hierarchy mode.
const SUB_PLACE_RADIUS_KM: f64 = 3.0;
/// How many sub-places to sample per refill in hierarchy mode.
const SUB_PLACES_PER_REFILL: usize = 4;
/// Results per search request. Kept modest so `cllimit=max` (500) covers every page's categories.
const PAGE_SIZE: u32 = 20;

/// Curated categories also contain close-ups of insects, birds, food… — beautiful, but not a
/// picture *of a place*. A file is dropped if any of its category names contains one of these
/// (case-insensitive). See docs/sources.md.
const OFF_TOPIC_SUBJECTS: &[&str] = &[
    "insect",
    "lepidoptera",
    "butterfl",
    "moths",
    "odonata",
    "hymenoptera",
    "diptera",
    "coleoptera",
    "hemiptera",
    "orthoptera",
    "arachnid",
    "spider",
    "birds",
    "aves",
    "mammal",
    "animals",
    "reptil",
    "amphibia",
    "fish",
    "fungi",
    "food",
    "portrait",
    "macro photograph",
    "objects",
    "coins",
    "maps",
    "diagrams",
    "astronomy",
];

pub struct CommonsSource<'a> {
    http: &'a dyn HttpClient,
}

impl<'a> CommonsSource<'a> {
    pub fn new(http: &'a dyn HttpClient) -> Self {
        Self { http }
    }

    /// Run one search query and turn its pages into candidates.
    ///
    /// `page` selects which slice of the results to take (0 = best). Geographic and text
    /// queries (`paged`) get variety from paging; category queries use random order.
    fn search(
        &self,
        query: &str,
        place_label: &str,
        page: u32,
        paged: bool,
    ) -> Result<Vec<Candidate>> {
        let results = self.search_page(query, place_label, page, paged)?;
        if results.is_empty() && page > 0 {
            // Paged past the end of a small result set.
            return self.search_page(query, place_label, 0, paged);
        }
        Ok(results)
    }

    fn search_page(
        &self,
        query: &str,
        place_label: &str,
        page: u32,
        paged: bool,
    ) -> Result<Vec<Candidate>> {
        let limit = PAGE_SIZE.to_string();
        let offset = (page * PAGE_SIZE).to_string();
        let geo = paged;
        let mut params = vec![
            ("action", "query"),
            ("format", "json"),
            ("formatversion", "2"),
            ("generator", "search"),
            ("gsrnamespace", "6"),
            ("gsrlimit", &limit),
            ("gsrsearch", query),
            ("prop", "imageinfo|coordinates|categories"),
            ("iiprop", "url|size|mime|extmetadata"),
            ("iiurlwidth", "960"),
            (
                "iiextmetadatafilter",
                "ObjectName|Artist|LicenseShortName|LicenseUrl|ImageDescription|AttributionRequired",
            ),
            ("cllimit", "max"),
        ];
        if geo {
            params.push(("gsroffset", &offset));
        } else {
            params.push(("gsrsort", "random"));
        }
        let mut pages: BTreeMap<u64, Page> = BTreeMap::new();
        let mut cont: Option<Continue> = None;
        // `categories` may be split over several responses; three rounds is plenty at PAGE_SIZE.
        for _ in 0..3 {
            let mut p = params.clone();
            if let Some(c) = &cont {
                p.push(("continue", c.r#continue.as_str()));
                if let Some(cl) = &c.clcontinue {
                    p.push(("clcontinue", cl.as_str()));
                }
            }
            let resp: Response = get_json(self.http, &build_url(API, &p), "commons search")?;
            // A warning means the API ignored part of the query, so the results can be
            // arbitrary (e.g. worldwide instead of near the place). Fail loudly.
            if let Some(w) = &resp.warnings {
                return Err(crate::error::Error::Parse {
                    what: "commons search",
                    msg: format!("API warning: {w}"),
                });
            }
            for page in resp.query.map(|q| q.pages).unwrap_or_default() {
                match pages.get_mut(&page.pageid) {
                    Some(existing) => existing.categories.extend(page.categories),
                    None => {
                        pages.insert(page.pageid, page);
                    }
                }
            }
            cont = resp.r#continue.filter(|c| c.clcontinue.is_some());
            if cont.is_none() {
                break;
            }
        }
        let mut out: Vec<(u32, Candidate)> = pages
            .into_values()
            .filter_map(|p| Some((p.index.unwrap_or(u32::MAX), p.into_candidate(place_label)?)))
            .collect();
        out.sort_by_key(|(i, _)| *i);
        Ok(out.into_iter().map(|(_, c)| c).collect())
    }
}

fn curated_filter() -> String {
    format!(
        "incategory:{}|{}|{}",
        CAT_FEATURED.replace(' ', "_"),
        CAT_QUALITY.replace(' ', "_"),
        CAT_VALUED.replace(' ', "_")
    )
}

fn base_filters(req: &CandidateRequest) -> String {
    format!(
        "filemime:image/jpeg fileh:>{}",
        req.min_image_height().saturating_sub(1)
    )
}

pub fn radius_query(center: GeoPoint, km: f64, req: &CandidateRequest) -> String {
    format!(
        "nearcoord:{}m,{:.5},{:.5} {} {}",
        // Whole metres: the API rejects fractional units (and then silently drops the filter).
        (km * 1000.0).round().max(100.0) as u64,
        center.lat,
        center.lon,
        curated_filter(),
        base_filters(req)
    )
}

/// Free-text search restricted to the same curated categories. The text is reduced to plain
/// words so it cannot smuggle in CirrusSearch operators (`incategory:`, quotes, `|`).
pub fn text_query(text: &str, req: &CandidateRequest) -> String {
    let words: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{words} {} {}", curated_filter(), base_filters(req))
}

pub fn category_query(category: &str, req: &CandidateRequest) -> String {
    let c = category.replace(' ', "_").replace('"', "");
    format!(
        "incategory:\"Featured_pictures_of_{c}\"|\"Quality_images_of_{c}\" {}",
        base_filters(req)
    )
}

/// Deterministic for a given seed, so tests are stable and a refill doesn't repeat the
/// previous one's sub-places when the caller rotates the seed.
fn pick_sub_places(subs: &[SubPlace], n: usize, seed: u64) -> Vec<&SubPlace> {
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut idx: Vec<usize> = (0..subs.len()).collect();
    rng.shuffle(&mut idx);
    idx.into_iter().take(n).map(|i| &subs[i]).collect()
}

impl Source for CommonsSource<'_> {
    fn kind(&self) -> SourceKind {
        SourceKind::Commons
    }

    fn candidates(&self, search: &Search<'_>, req: &CandidateRequest) -> Result<Vec<Candidate>> {
        let (place, scope) = match search {
            Search::Place { place, scope } => (*place, *scope),
            Search::Query(text) => {
                let mut out = self.search(&text_query(text, req), text, req.page(), true)?;
                super::dedup(&mut out);
                out.truncate(req.limit as usize);
                return Ok(out);
            }
        };
        let mut out = Vec::new();
        match scope {
            Scope::Radius { center, km } => {
                out.extend(self.search(
                    &radius_query(*center, *km, req),
                    &place.label,
                    req.page(),
                    true,
                )?);
            }
            Scope::Hierarchy => {
                let mut last_err = None;
                let mut groups: Vec<Vec<Candidate>> = Vec::new();
                for sub in pick_sub_places(&req.sub_places, SUB_PLACES_PER_REFILL, req.seed) {
                    let q = radius_query(sub.coords, SUB_PLACE_RADIUS_KM, req);
                    let label = format!("{}, {}", sub.label, place.label);
                    // One flaky sub-place shouldn't sink the whole refill.
                    match self.search(&q, &label, req.page(), true) {
                        Ok(c) => groups.push(c),
                        Err(e) => last_err = Some(e),
                    }
                }
                if let Some(cat) = &place.commons_category {
                    match self.search(&category_query(cat, req), &place.label, 0, false) {
                        Ok(c) => groups.push(c),
                        Err(e) => last_err = Some(e),
                    }
                }
                // Round-robin so one dense area (Hiroshima…) can't crowd out the rest.
                out = super::interleave(groups);
                if out.is_empty() {
                    if let Some(e) = last_err {
                        return Err(e);
                    }
                }
            }
        }
        super::dedup(&mut out);
        out.truncate(req.limit as usize);
        Ok(out)
    }

    fn check(&self) -> Result<()> {
        let url = build_url(
            API,
            &[
                ("action", "query"),
                ("format", "json"),
                ("meta", "siteinfo"),
                ("siprop", "general"),
            ],
        );
        self.http.get_quick(&url, &[], MAX_JSON_BYTES).map(|_| ())
    }

    fn sized_url(&self, c: &Candidate, min_width: u32, min_height: u32) -> String {
        let need = min_width.max((min_height as f32 * c.aspect()).ceil() as u32);
        match THUMB_STEPS.iter().find(|&&s| s >= need) {
            Some(&step) if step < c.width => {
                thumb_with_width(&c.preview_url, step).unwrap_or_else(|| c.image_url.clone())
            }
            _ => c.image_url.clone(),
        }
    }
}

/// `…/thumb/a/ab/Name.jpg/960px-Name.jpg` → same with `{width}px-`.
fn thumb_with_width(thumb_url: &str, width: u32) -> Option<String> {
    let slash = thumb_url.rfind('/')?;
    let (head, file) = thumb_url.split_at(slash + 1);
    let px = file.find("px-")?;
    file[..px].parse::<u32>().ok()?;
    Some(format!("{head}{width}{}", &file[px..]))
}

fn strip_query(url: &str) -> String {
    url.split(['?', '#']).next().unwrap_or(url).to_owned()
}

// --- wire format ----------------------------------------------------------------------------

#[derive(Deserialize)]
struct Response {
    warnings: Option<Value>,
    query: Option<Query>,
    r#continue: Option<Continue>,
}

#[derive(Deserialize)]
struct Continue {
    r#continue: String,
    clcontinue: Option<String>,
}

#[derive(Deserialize)]
struct Query {
    #[serde(default)]
    pages: Vec<Page>,
}

#[derive(Deserialize)]
struct Page {
    pageid: u64,
    title: String,
    index: Option<u32>,
    #[serde(default)]
    imageinfo: Vec<ImageInfo>,
    #[serde(default)]
    coordinates: Vec<Coord>,
    #[serde(default)]
    categories: Vec<Category>,
}

#[derive(Deserialize)]
struct ImageInfo {
    width: u32,
    height: u32,
    url: String,
    thumburl: Option<String>,
    descriptionurl: String,
    mime: Option<String>,
    #[serde(default)]
    extmetadata: BTreeMap<String, MetaField>,
}

#[derive(Deserialize)]
struct MetaField {
    value: Value,
}

#[derive(Deserialize)]
struct Coord {
    lat: f64,
    lon: f64,
}

#[derive(Deserialize)]
struct Category {
    title: String,
}

impl ImageInfo {
    fn meta(&self, key: &str) -> Option<String> {
        let v = &self.extmetadata.get(key)?.value;
        let s = match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => return None,
        };
        let t = html_to_text(&s);
        (!t.is_empty()).then_some(t)
    }
}

impl Page {
    fn category_names(&self) -> impl Iterator<Item = &str> {
        self.categories
            .iter()
            .map(|c| c.title.strip_prefix("Category:").unwrap_or(&c.title))
    }

    fn is_off_topic(&self) -> bool {
        self.category_names().any(|name| {
            let lower = name.to_lowercase();
            (lower.starts_with("featured pictures of") || lower.starts_with("quality images of"))
                && OFF_TOPIC_SUBJECTS.iter().any(|s| lower.contains(s))
        })
    }

    fn into_candidate(self, place_label: &str) -> Option<Candidate> {
        if self.is_off_topic() {
            return None;
        }
        let info = self.imageinfo.first()?;
        if info.mime.as_deref().is_some_and(|m| m != "image/jpeg") {
            return None;
        }
        // No license metadata → we can't attribute it properly → don't use it.
        let license_name = info.meta("LicenseShortName")?;
        let author = info
            .meta("Artist")
            .map(|a| truncate(&a, 80))
            .unwrap_or_else(|| "Unknown author".to_owned());
        let title = info
            .meta("ObjectName")
            .unwrap_or_else(|| title_from_file_name(&self.title));
        let description = info
            .meta("ImageDescription")
            .map(|d| truncate(&d, 300))
            .filter(|d| d != &title);
        let names: Vec<&str> = self.category_names().collect();
        let curation = Curation {
            featured: names.contains(&CAT_FEATURED),
            quality: names.contains(&CAT_QUALITY),
            valued: names.contains(&CAT_VALUED),
            likes: None,
        };
        let image_url = strip_query(&info.url);
        Some(Candidate {
            source: SourceKind::Commons,
            id: format!("commons:{}", self.pageid),
            title: truncate(&title, 120),
            description,
            page_url: info.descriptionurl.clone(),
            preview_url: info
                .thumburl
                .as_deref()
                .map(strip_query)
                .unwrap_or_else(|| image_url.clone()),
            image_url,
            width: info.width,
            height: info.height,
            author,
            author_url: None,
            license: License {
                short_name: license_name,
                url: info.meta("LicenseUrl"),
                attribution_required: info.meta("AttributionRequired").as_deref() != Some("false"),
            },
            curation,
            coords: self
                .coordinates
                .first()
                .map(|c| GeoPoint::new(c.lat, c.lon))
                .filter(GeoPoint::is_valid),
            place_label: place_label.to_owned(),
        })
    }
}

fn title_from_file_name(title: &str) -> String {
    let name = title.strip_prefix("File:").unwrap_or(title);
    let name = name.rsplit_once('.').map(|(n, _)| n).unwrap_or(name);
    name.replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_resizing() {
        let u =
            "https://thumb.wikimedia.org/wikipedia/commons/thumb/4/4a/A%2C_B.jpg/960px-A%2C_B.jpg";
        assert_eq!(
            thumb_with_width(u, 1920).unwrap(),
            "https://thumb.wikimedia.org/wikipedia/commons/thumb/4/4a/A%2C_B.jpg/1920px-A%2C_B.jpg"
        );
        assert_eq!(thumb_with_width("https://x/y/Name.jpg", 100), None);
    }

    #[test]
    fn queries() {
        let req = CandidateRequest::new(1080, 2400);
        let q = radius_query(GeoPoint::new(35.01161, 135.76811), 16.2, &req);
        assert_eq!(
            q,
            "nearcoord:16200m,35.01161,135.76811 \
             incategory:Featured_pictures_on_Wikimedia_Commons|Quality_images|Valued_images_sorted_by_promotion_date \
             filemime:image/jpeg fileh:>1919"
        );
        assert_eq!(
            category_query("Mount Fuji", &req),
            "incategory:\"Featured_pictures_of_Mount_Fuji\"|\"Quality_images_of_Mount_Fuji\" \
             filemime:image/jpeg fileh:>1919"
        );
    }

    #[test]
    fn file_name_fallback() {
        assert_eq!(
            title_from_file_name("File:Kyoto_at_night.v2.jpg"),
            "Kyoto at night.v2"
        );
    }

    #[test]
    fn sub_place_sampling_is_deterministic() {
        let subs: Vec<SubPlace> = (0..10)
            .map(|i| SubPlace {
                qid: format!("Q{i}"),
                label: i.to_string(),
                coords: GeoPoint::new(0.0, 0.0),
            })
            .collect();
        let a: Vec<_> = pick_sub_places(&subs, 4, 7)
            .iter()
            .map(|s| s.qid.clone())
            .collect();
        let b: Vec<_> = pick_sub_places(&subs, 4, 7)
            .iter()
            .map(|s| s.qid.clone())
            .collect();
        assert_eq!(a, b);
        assert_eq!(a.len(), 4);
        assert_eq!(pick_sub_places(&subs[..2], 4, 7).len(), 2);
    }
}
