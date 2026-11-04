use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

impl GeoPoint {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }

    /// Great-circle distance (haversine), kilometres.
    pub fn distance_km(&self, other: &GeoPoint) -> f64 {
        const R: f64 = 6371.0088;
        let (p1, p2) = (self.lat.to_radians(), other.lat.to_radians());
        let dp = (other.lat - self.lat).to_radians();
        let dl = (other.lon - self.lon).to_radians();
        let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
        2.0 * R * a.sqrt().asin()
    }

    pub fn is_valid(&self) -> bool {
        self.lat.is_finite()
            && self.lon.is_finite()
            && (-90.0..=90.0).contains(&self.lat)
            && (-180.0..=180.0).contains(&self.lon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kyoto_to_tokyo_is_about_365_km() {
        let kyoto = GeoPoint::new(35.0116, 135.7681);
        let tokyo = GeoPoint::new(35.6895, 139.6917);
        let d = kyoto.distance_km(&tokyo);
        assert!((360.0..372.0).contains(&d), "{d}");
    }

    #[test]
    fn validity() {
        assert!(GeoPoint::new(0.0, 0.0).is_valid());
        assert!(!GeoPoint::new(91.0, 0.0).is_valid());
        assert!(!GeoPoint::new(f64::NAN, 0.0).is_valid());
    }
}
