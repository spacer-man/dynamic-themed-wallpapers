//! Engine of dynamic-themed-wallpapers: place search, photo sources, beauty scoring, cropping
//! and on-device storage. Pure Rust, no Tauri, no Android — the app shell and the background
//! worker both call into this crate.

pub mod analysis;
pub mod error;
pub mod geo;
pub mod http;
pub mod pipeline;
pub mod place;
pub mod score;
pub mod service;
pub mod source;
pub mod store;
pub mod subject;
pub mod text;
pub mod theme;

pub use error::{Error, Result};
pub use geo::GeoPoint;
pub use place::{Place, PlaceHit, PlaceKind, Scope, SubPlace, Wikidata};
pub use subject::{Subject, SubjectKind, SubjectSpec, Subjects};
