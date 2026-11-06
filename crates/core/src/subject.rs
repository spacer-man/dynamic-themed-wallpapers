//! What the user wants pictures of. Several subjects can be saved and one is active.
//!
//! A subject is a [`Place`] (resolved through Wikidata), a free-text query ("foggy forest"),
//! and — later — a Pinterest board. Which sources can serve which kind is decided by
//! [`SourceKind::supports`](crate::source::SourceKind::supports).

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::place::Place;

pub const MIN_QUERY_CHARS: usize = 2;
pub const MAX_QUERY_CHARS: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Place,
    Query,
}

/// Tagged by `kind` on the wire: `{"kind":"place","place":{…}}`, `{"kind":"query","text":"…"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubjectSpec {
    Place { place: Place },
    Query { text: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subject {
    pub id: String,
    #[serde(flatten)]
    pub spec: SubjectSpec,
}

impl Subject {
    pub fn kind(&self) -> SubjectKind {
        match self.spec {
            SubjectSpec::Place { .. } => SubjectKind::Place,
            SubjectSpec::Query { .. } => SubjectKind::Query,
        }
    }

    /// Short human name: the place label or the query text.
    pub fn label(&self) -> &str {
        match &self.spec {
            SubjectSpec::Place { place } => &place.label,
            SubjectSpec::Query { text } => text,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Subjects {
    pub items: Vec<Subject>,
    /// Id of the subject photos are searched for.
    pub active: Option<String>,
}

impl Subjects {
    pub fn get(&self, id: &str) -> Option<&Subject> {
        self.items.iter().find(|s| s.id == id)
    }

    pub fn active_subject(&self) -> Option<&Subject> {
        self.active.as_deref().and_then(|id| self.get(id))
    }

    fn fresh_id(&self) -> String {
        let max = self
            .items
            .iter()
            .filter_map(|s| s.id.strip_prefix('s')?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("s{}", max + 1)
    }

    /// Append `spec` and make it active. Returns the new id.
    pub fn add(&mut self, spec: SubjectSpec) -> String {
        let id = self.fresh_id();
        self.items.push(Subject {
            id: id.clone(),
            spec,
        });
        self.active = Some(id.clone());
        id
    }

    pub fn find_place(&self, qid: &str) -> Option<&Subject> {
        self.items
            .iter()
            .find(|s| matches!(&s.spec, SubjectSpec::Place { place } if place.qid == qid))
    }

    pub fn find_query(&self, text: &str) -> Option<&Subject> {
        self.items.iter().find(
            |s| matches!(&s.spec, SubjectSpec::Query { text: t } if t.to_lowercase() == text.to_lowercase()),
        )
    }

    /// Remove a subject. If it was the active one, the first remaining becomes active.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|s| s.id != id);
        if self.active.as_deref() == Some(id) {
            self.active = self.items.first().map(|s| s.id.clone());
        }
        self.items.len() != before
    }
}

/// Trim, collapse whitespace, drop control characters, enforce the length limits.
pub fn normalize_query(text: &str) -> Result<String> {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let q = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let n = q.chars().count();
    if !(MIN_QUERY_CHARS..=MAX_QUERY_CHARS).contains(&n) {
        return Err(Error::Config(format!(
            "a search text needs {MIN_QUERY_CHARS}–{MAX_QUERY_CHARS} characters"
        )));
    }
    Ok(q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(t: &str) -> SubjectSpec {
        SubjectSpec::Query { text: t.into() }
    }

    #[test]
    fn add_activates_and_ids_are_unique() {
        let mut s = Subjects::default();
        let a = s.add(q("foggy forest"));
        let b = s.add(q("desert"));
        assert_ne!(a, b);
        assert_eq!(s.active.as_deref(), Some(b.as_str()));
        s.remove(&a);
        let c = s.add(q("alps"));
        assert_ne!(c, b);
    }

    #[test]
    fn removing_the_active_one_activates_another() {
        let mut s = Subjects::default();
        let a = s.add(q("one"));
        let b = s.add(q("two"));
        assert!(s.remove(&b));
        assert_eq!(s.active.as_deref(), Some(a.as_str()));
        assert!(s.remove(&a));
        assert_eq!(s.active, None);
        assert!(!s.remove("nope"));
    }

    #[test]
    fn query_text_is_normalised() {
        assert_eq!(
            normalize_query("  foggy \n forest ").unwrap(),
            "foggy forest"
        );
        assert!(normalize_query("a").is_err());
        assert!(normalize_query(&"x".repeat(81)).is_err());
        assert!(normalize_query("   ").is_err());
    }

    #[test]
    fn wire_format_is_tagged() {
        let s = Subject {
            id: "s1".into(),
            spec: q("foggy forest"),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"id":"s1","kind":"query","text":"foggy forest"}"#);
        assert_eq!(serde_json::from_str::<Subject>(&json).unwrap(), s);
    }
}
