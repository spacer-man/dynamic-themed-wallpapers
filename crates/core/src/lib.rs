//! Engine of dynamic-themed-wallpapers: place search, photo sources, beauty scoring, cropping
//! and on-device storage. Pure Rust, no Tauri, no Android — the app shell and the background
//! worker both call into this crate.

pub mod error;
pub mod geo;
pub mod http;
pub mod place;
pub mod source;
pub mod text;

pub use error::{Error, Result};
pub use geo::GeoPoint;
pub use place::{Place, PlaceHit, PlaceKind, Scope, SubPlace, Wikidata};
