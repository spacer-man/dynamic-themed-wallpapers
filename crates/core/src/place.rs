//! Places: what the user typed → a Wikidata entity with coordinates, kind and size.
//!
//! Uses only the regular MediaWiki/Wikibase API on www.wikidata.org (no SPARQL: the query
//! service is rate-limited too aggressively to be a dependency of a daily background job).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::geo::GeoPoint;
use crate::http::{build_url, get_json, HttpClient};

const API: &str = "https://www.wikidata.org/w/api.php";

/// One row of the "search a place" result list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaceHit {
    pub qid: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceKind {
    Country,
    Region,
    City,
    District,
    Landmark,
    Natural,
    Other,
}

/// A resolved place, ready to fetch photos for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub qid: String,
    pub label: String,
    pub description: Option<String>,
    pub coords: Option<GeoPoint>,
    pub kind: PlaceKind,
    pub area_km2: Option<f64>,
    /// Commons category name without the `Category:` prefix (Wikidata P373).
    pub commons_category: Option<String>,
}

/// How photos are selected for a place.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Scope {
    /// Photos geotagged within `km` of the place's coordinates.
    Radius { center: GeoPoint, km: f64 },
    /// Big places (countries, large regions): curated categories of the place itself, plus
    /// radius searches around notable sub-places. A plain radius around a country's centroid
    /// would both miss most of it and bleed into its neighbours.
    Hierarchy,
}

/// Radius searches above this switch to [`Scope::Hierarchy`].
pub const MAX_RADIUS_KM: f64 = 40.0;
pub const MIN_RADIUS_KM: f64 = 0.5;

impl Place {
    /// Radius that roughly covers the place: from its area when known, else a per-kind default.
    pub fn natural_radius_km(&self) -> f64 {
        let from_area = self
            .area_km2
            .filter(|a| a.is_finite() && *a > 0.0)
            .map(|a| (a / std::f64::consts::PI).sqrt());
        let r = from_area.unwrap_or(match self.kind {
            PlaceKind::Country => 500.0,
            PlaceKind::Region => 80.0,
            PlaceKind::City => 10.0,
            PlaceKind::District => 2.0,
            PlaceKind::Landmark => 1.0,
            PlaceKind::Natural => 6.0,
            PlaceKind::Other => 5.0,
        });
        r.max(MIN_RADIUS_KM)
    }

    pub fn default_scope(&self) -> Scope {
        let km = self.natural_radius_km();
        match self.coords {
            Some(center) if self.kind != PlaceKind::Country && km <= MAX_RADIUS_KM => {
                Scope::Radius { center, km }
            }
            _ => Scope::Hierarchy,
        }
    }
}

/// A notable place inside a bigger one (used by [`Scope::Hierarchy`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubPlace {
    pub qid: String,
    pub label: String,
    pub coords: GeoPoint,
}

// --- classification -------------------------------------------------------------------------

// "instance of" (P31) values. Not exhaustive on purpose: anything unknown is `Other`, which
// still works through its area/coordinates.
const COUNTRY: &[&str] = &["Q6256", "Q3624078", "Q7275", "Q112099"];
const REGION: &[&str] = &[
    "Q10864048",
    "Q50337",
    "Q35657",
    "Q34876",
    "Q107390",
    "Q82794",
    "Q1620908",
    "Q15284",
    "Q13220204",
    "Q3336843",
];
const CITY: &[&str] = &[
    "Q515",
    "Q1549591",
    "Q1637706",
    "Q5119",
    "Q3957",
    "Q532",
    "Q200250",
    "Q1749269",
    "Q494721",
    "Q15661340",
    "Q1093829",
];
const DISTRICT: &[&str] = &["Q123705", "Q5327704", "Q2198484", "Q4286337", "Q1289426"];
const NATURAL: &[&str] = &[
    "Q8502", "Q46831", "Q23397", "Q34038", "Q23442", "Q46169", "Q4022", "Q8072", "Q169358",
    "Q40080", "Q150784", "Q33837", "Q35666", "Q35509", "Q4421", "Q39594", "Q54050", "Q185113",
];
const LANDMARK: &[&str] = &[
    "Q570116", "Q5393308", "Q845945", "Q23413", "Q22698", "Q1107656", "Q16970", "Q12518", "Q12280",
    "Q33506", "Q44539", "Q32815", "Q2977", "Q4989906", "Q174782", "Q16560", "Q811979", "Q41176",
    "Q9259", "Q1030034", "Q839954", "Q55488",
];

/// Types worth sampling as sub-places of a big place. Order matters only for readability.
const PHOTOGENIC: &[&str] = &[
    "Q570116", "Q8502", "Q5393308", "Q845945", "Q23413", "Q46169", "Q23397", "Q34038", "Q23442",
    "Q1107656", "Q22698", "Q8072", "Q16560", "Q40080", "Q9259", "Q839954", "Q515", "Q1549591",
];

pub fn classify(instance_of: &[String]) -> PlaceKind {
    let has = |set: &[&str]| instance_of.iter().any(|q| set.contains(&q.as_str()));
    // Most specific first: a "city" that is also a "tourist attraction" is a city.
    if has(COUNTRY) {
        PlaceKind::Country
    } else if has(CITY) {
        PlaceKind::City
    } else if has(DISTRICT) {
        PlaceKind::District
    } else if has(REGION) {
        PlaceKind::Region
    } else if has(NATURAL) {
        PlaceKind::Natural
    } else if has(LANDMARK) {
        PlaceKind::Landmark
    } else {
        PlaceKind::Other
    }
}

// --- client ---------------------------------------------------------------------------------

pub struct Wikidata<'a> {
    http: &'a dyn HttpClient,
    /// UI language, e.g. "ru" or "en". Labels fall back to English.
    lang: String,
}

impl<'a> Wikidata<'a> {
    pub fn new(http: &'a dyn HttpClient, lang: &str) -> Self {
        let lang = if lang.trim().is_empty() {
            "en"
        } else {
            lang.trim()
        };
        Self {
            http,
            lang: lang.to_owned(),
        }
    }

    /// Free-text search. Tries the UI language first, then English (people type "Kyoto" in a
    /// Russian UI all the time).
    pub fn search(&self, text: &str, limit: u32) -> Result<Vec<PlaceHit>> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let hits = self.search_in(text, &self.lang, limit)?;
        if hits.is_empty() && self.lang != "en" {
            return self.search_in(text, "en", limit);
        }
        Ok(hits)
    }

    fn search_in(&self, text: &str, lang: &str, limit: u32) -> Result<Vec<PlaceHit>> {
        let limit = limit.clamp(1, 50).to_string();
        let url = build_url(
            API,
            &[
                ("action", "wbsearchentities"),
                ("format", "json"),
                ("type", "item"),
                ("search", text),
                ("language", lang),
                ("uselang", &self.lang),
                ("limit", &limit),
            ],
        );
        let resp: SearchResponse = get_json(self.http, &url, "wikidata search")?;
        Ok(resp
            .search
            .into_iter()
            .map(|h| PlaceHit {
                qid: h.id,
                label: h.label.unwrap_or_default(),
                description: h.description.filter(|d| !d.is_empty()),
            })
            .filter(|h| is_qid(&h.qid))
            .collect())
    }

    /// Fetch everything needed to pick photos for `qid`.
    pub fn resolve(&self, qid: &str) -> Result<Place> {
        if !is_qid(qid) {
            return Err(Error::NotFound(format!("not a Wikidata item id: {qid:?}")));
        }
        let (label, description, coords) = self.terms_and_coords(qid)?;
        let instance_of = self.item_claims(qid, "P31")?;
        let area_km2 = self.area_km2(qid)?;
        let commons_category = self.string_claims(qid, "P373")?.into_iter().next();
        Ok(Place {
            qid: qid.to_owned(),
            label: label.unwrap_or_else(|| qid.to_owned()),
            description,
            coords,
            kind: classify(&instance_of),
            area_km2,
            commons_category,
        })
    }

    /// Notable, photogenic places inside `place`, most notable first.
    pub fn sub_places(&self, place: &Place, limit: u32) -> Result<Vec<SubPlace>> {
        let containment = match place.kind {
            PlaceKind::Country => format!("haswbstatement:P17={}", place.qid),
            _ => format!("haswbstatement:P131={}", place.qid),
        };
        let types = PHOTOGENIC
            .iter()
            .map(|q| format!("P31={q}"))
            .collect::<Vec<_>>()
            .join("|");
        let query = format!("{containment} haswbstatement:{types}");
        let limit = limit.clamp(1, 50).to_string();
        let url = build_url(
            API,
            &[
                ("action", "query"),
                ("format", "json"),
                ("formatversion", "2"),
                ("generator", "search"),
                ("gsrnamespace", "0"),
                ("gsrlimit", &limit),
                ("gsrsearch", &query),
                ("prop", "coordinates|entityterms"),
                ("wbetterms", "label"),
                ("wbetlanguage", &self.lang),
            ],
        );
        let resp: QueryResponse = get_json(self.http, &url, "wikidata sub-places")?;
        let mut pages = resp.query.map(|q| q.pages).unwrap_or_default();
        pages.sort_by_key(|p| p.index.unwrap_or(u32::MAX));
        Ok(pages
            .into_iter()
            .filter_map(|p| {
                let coords = p.primary_coords()?;
                let label = p.label().unwrap_or_else(|| p.title.clone());
                Some(SubPlace {
                    qid: p.title,
                    label,
                    coords,
                })
            })
            .collect())
    }

    fn terms_and_coords(
        &self,
        qid: &str,
    ) -> Result<(Option<String>, Option<String>, Option<GeoPoint>)> {
        let url = build_url(
            API,
            &[
                ("action", "query"),
                ("format", "json"),
                ("formatversion", "2"),
                ("titles", qid),
                ("prop", "coordinates|entityterms"),
                ("wbetterms", "label|description"),
                ("wbetlanguage", &self.lang),
            ],
        );
        let resp: QueryResponse = get_json(self.http, &url, "wikidata entity")?;
        let page = resp
            .query
            .and_then(|q| q.pages.into_iter().next())
            .filter(|p| !p.missing)
            .ok_or_else(|| Error::NotFound(qid.to_owned()))?;
        let description = page
            .entityterms
            .as_ref()
            .and_then(|t| t.description.first().cloned());
        Ok((page.label(), description, page.primary_coords()))
    }

    fn claims(&self, qid: &str, property: &str) -> Result<Vec<Value>> {
        let url = build_url(
            API,
            &[
                ("action", "wbgetclaims"),
                ("format", "json"),
                ("entity", qid),
                ("property", property),
            ],
        );
        let resp: ClaimsResponse = get_json(self.http, &url, "wikidata claims")?;
        let mut claims = resp
            .claims
            .into_iter()
            .flat_map(|(_, v)| v)
            .collect::<Vec<Claim>>();
        // Preferred first, deprecated never.
        claims.retain(|c| c.rank != "deprecated");
        claims.sort_by_key(|c| if c.rank == "preferred" { 0 } else { 1 });
        Ok(claims
            .into_iter()
            .filter_map(|c| c.mainsnak.datavalue.map(|d| d.value))
            .collect())
    }

    fn item_claims(&self, qid: &str, property: &str) -> Result<Vec<String>> {
        Ok(self
            .claims(qid, property)?
            .iter()
            .filter_map(|v| v.get("id").and_then(Value::as_str).map(str::to_owned))
            .collect())
    }

    fn string_claims(&self, qid: &str, property: &str) -> Result<Vec<String>> {
        Ok(self
            .claims(qid, property)?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect())
    }

    fn area_km2(&self, qid: &str) -> Result<Option<f64>> {
        Ok(self.claims(qid, "P2046")?.iter().find_map(quantity_to_km2))
    }
}

pub fn is_qid(s: &str) -> bool {
    s.len() > 1 && s.starts_with('Q') && s[1..].bytes().all(|b| b.is_ascii_digit())
}

fn quantity_to_km2(v: &Value) -> Option<f64> {
    let amount: f64 = v
        .get("amount")?
        .as_str()?
        .trim_start_matches('+')
        .parse()
        .ok()?;
    let unit = v.get("unit")?.as_str()?.rsplit('/').next()?;
    let factor = match unit {
        "Q712226" => 1.0,         // square kilometre
        "Q25343" => 1e-6,         // square metre
        "Q35852" => 0.01,         // hectare
        "Q232291" => 2.589_988,   // square mile
        "Q81292" => 0.004_046_86, // acre
        _ => return None,
    };
    let km2 = amount * factor;
    (km2.is_finite() && km2 > 0.0).then_some(km2)
}

// --- wire formats ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    search: Vec<SearchHit>,
}

#[derive(Deserialize)]
struct SearchHit {
    id: String,
    label: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct QueryResponse {
    query: Option<QueryPages>,
}

#[derive(Deserialize)]
struct QueryPages {
    #[serde(default)]
    pages: Vec<Page>,
}

#[derive(Deserialize)]
struct Page {
    title: String,
    index: Option<u32>,
    #[serde(default)]
    missing: bool,
    #[serde(default)]
    coordinates: Vec<Coord>,
    entityterms: Option<EntityTerms>,
}

impl Page {
    fn primary_coords(&self) -> Option<GeoPoint> {
        self.coordinates
            .iter()
            .find(|c| c.primary.is_some() && c.globe.as_deref().unwrap_or("earth") == "earth")
            .or_else(|| self.coordinates.first())
            .map(|c| GeoPoint::new(c.lat, c.lon))
            .filter(GeoPoint::is_valid)
    }

    fn label(&self) -> Option<String> {
        self.entityterms
            .as_ref()
            .and_then(|t| t.label.first().cloned())
    }
}

#[derive(Deserialize)]
struct Coord {
    lat: f64,
    lon: f64,
    primary: Option<Value>,
    globe: Option<String>,
}

#[derive(Deserialize, Default)]
struct EntityTerms {
    #[serde(default)]
    label: Vec<String>,
    #[serde(default)]
    description: Vec<String>,
}

#[derive(Deserialize)]
struct ClaimsResponse {
    #[serde(default)]
    claims: std::collections::BTreeMap<String, Vec<Claim>>,
}

#[derive(Deserialize)]
struct Claim {
    mainsnak: Snak,
    #[serde(default)]
    rank: String,
}

#[derive(Deserialize)]
struct Snak {
    datavalue: Option<DataValue>,
}

#[derive(Deserialize)]
struct DataValue {
    value: Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(kind: PlaceKind, area: Option<f64>) -> Place {
        Place {
            qid: "Q1".into(),
            label: "x".into(),
            description: None,
            coords: Some(GeoPoint::new(35.0, 135.0)),
            kind,
            area_km2: area,
            commons_category: None,
        }
    }

    #[test]
    fn classify_prefers_specific_kinds() {
        let s = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(classify(&s(&["Q1749269", "Q570116"])), PlaceKind::City);
        assert_eq!(classify(&s(&["Q3624078"])), PlaceKind::Country);
        assert_eq!(classify(&s(&["Q8502"])), PlaceKind::Natural);
        assert_eq!(classify(&s(&["Q845945"])), PlaceKind::Landmark);
        assert_eq!(classify(&s(&["Q50337"])), PlaceKind::Region);
        assert_eq!(classify(&s(&["Q42"])), PlaceKind::Other);
    }

    #[test]
    fn scope_from_area_and_kind() {
        // Kyoto city: 827 km² → ~16 km radius.
        match place(PlaceKind::City, Some(827.83)).default_scope() {
            Scope::Radius { km, .. } => assert!((16.0..16.5).contains(&km), "{km}"),
            s => panic!("{s:?}"),
        }
        assert_eq!(
            place(PlaceKind::Country, Some(377_975.0)).default_scope(),
            Scope::Hierarchy
        );
        assert_eq!(
            place(PlaceKind::Region, Some(10_000.0)).default_scope(),
            Scope::Hierarchy
        );
        match place(PlaceKind::Landmark, None).default_scope() {
            Scope::Radius { km, .. } => assert_eq!(km, 1.0),
            s => panic!("{s:?}"),
        }
        // No coordinates → can't do radius.
        let mut p = place(PlaceKind::City, None);
        p.coords = None;
        assert_eq!(p.default_scope(), Scope::Hierarchy);
    }

    #[test]
    fn area_units() {
        let v = |amount: &str, unit: &str| serde_json::json!({"amount": amount, "unit": format!("http://www.wikidata.org/entity/{unit}")});
        assert_eq!(quantity_to_km2(&v("+827.83", "Q712226")), Some(827.83));
        assert_eq!(quantity_to_km2(&v("+250", "Q35852")), Some(2.5));
        assert_eq!(quantity_to_km2(&v("+1", "Q11573")), None);
        assert_eq!(quantity_to_km2(&v("-5", "Q712226")), None);
    }

    #[test]
    fn qid_validation() {
        assert!(is_qid("Q17"));
        assert!(!is_qid("Q"));
        assert!(!is_qid("P31"));
        assert!(!is_qid("Q1 OR 1"));
    }
}
