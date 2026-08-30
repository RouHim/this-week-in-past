use std::env;
use std::fmt::{Display, Formatter};
use std::sync::{LazyLock, OnceLock};

use lazy_static::lazy_static;
use regex::{Captures, Regex};
use rstar::{PointDistance, RTree, RTreeObject, AABB};
use serde::{Deserialize, Serialize};

/// Struct representing a geo location
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub struct GeoLocation {
    pub latitude: f32,
    pub longitude: f32,
}

/// Display trait implementation for GeoLocation
impl Display for GeoLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[lat={} lon={}]", self.latitude, self.longitude,)
    }
}

/// Converts Degrees Minutes Seconds To Decimal Degrees
/// See <https://stackoverflow.com/questions/14906764/converting-gps-coordinates-to-decimal-degrees>
fn dms_to_dd(dms_string: &str, dms_ref: &str) -> Option<f32> {
    lazy_static! {
        static ref DMS_PARSE_PATTERN_1: Regex = Regex::new(
            // e.g.: 7 deg 33 min 55.5155 sec or 7 deg 33 min 55 sec
            r"(?P<deg>\d+) deg (?P<min>\d+) min (?P<sec>\d+.?\d*) sec"
        )
        .unwrap();
        static ref DMS_PARSE_PATTERN_2: Regex = Regex::new(
            // e.g.: 50/1, 25/1, 2519/100
            r"(?P<deg>\d+)/(?P<deg_fraction>\d+),\s*(?P<min>\d+)/(?P<min_fraction>\d+),\s*(?P<sec>\d+)/(?P<sec_fraction>\d+)"
        )
        .unwrap();
    }

    let dms_pattern_1_match: Option<Captures> = DMS_PARSE_PATTERN_1.captures(dms_string);
    let dms_pattern_2_match: Option<Captures> = DMS_PARSE_PATTERN_2.captures(dms_string);

    // Depending on the dms ref the value has to be multiplied by -1
    let dms_ref_multiplier = match dms_ref {
        "S" | "W" => -1.0,
        _ => 1.0,
    };

    if let Some(pattern_match) = dms_pattern_1_match {
        parse_pattern_1(pattern_match).map(|value| value * dms_ref_multiplier)
    } else if let Some(pattern_match) = dms_pattern_2_match {
        parse_pattern_2(pattern_match).map(|value| value * dms_ref_multiplier)
    } else {
        None
    }
}

/// Parses Degrees minutes seconds for the following example pattern: "7 deg 33 min 55.5155 sec"
fn parse_pattern_1(caps: Captures) -> Option<f32> {
    let maybe_deg: Option<f32> = caps
        .name("deg")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_min: Option<f32> = caps
        .name("min")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_sec: Option<f32> = caps
        .name("sec")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());

    if let (Some(deg), Some(min), Some(sec)) = (maybe_deg, maybe_min, maybe_sec) {
        Some(deg + (min / 60.0) + (sec / 3600.0))
    } else {
        None
    }
}

/// Parses Degrees minutes seconds for the following example pattern: "50/1, 25/1, 2519/100"
fn parse_pattern_2(caps: Captures) -> Option<f32> {
    let maybe_deg: Option<f32> = caps
        .name("deg")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_deg_fraction: Option<f32> = caps
        .name("deg_fraction")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_min: Option<f32> = caps
        .name("min")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_min_fraction: Option<f32> = caps
        .name("min_fraction")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_sec: Option<f32> = caps
        .name("sec")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_sec_fraction: Option<f32> = caps
        .name("sec_fraction")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());

    if let (Some(deg), Some(deg_frac), Some(min), Some(min_frac), Some(sec), Some(sec_frac)) = (
        maybe_deg,
        maybe_deg_fraction,
        maybe_min,
        maybe_min_fraction,
        maybe_sec,
        maybe_sec_fraction,
    ) {
        Some((deg / deg_frac) + ((min / min_frac) / 60.0) + ((sec / sec_frac) / 3600.0))
    } else {
        None
    }
}

/// Converts latitude and longitude to a GeoLocation
/// If the latitude or longitude is not valid, None is returned
/// This is done by converting the latitude and longitude to degrees minutes seconds
pub fn from_degrees_minutes_seconds(
    latitude: &str,
    longitude: &str,
    latitude_ref: &str,
    longitude_ref: &str,
) -> Option<GeoLocation> {
    let maybe_dd_lat = dms_to_dd(latitude, latitude_ref);
    let maybe_dd_lon = dms_to_dd(longitude, longitude_ref);

    if let (Some(latitude), Some(longitude)) = (maybe_dd_lat, maybe_dd_lon) {
        Some(GeoLocation {
            latitude,
            longitude,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Offline city resolution via cities500
// ---------------------------------------------------------------------------

/// Maximum distance from query point to nearest city for a valid match.
/// Beyond this threshold the result is `None` (ocean / desert case).
const MAX_DISTANCE_KM: f64 = 50.0;

/// Default path of the cities500 data file inside the container.
const CITIES500_PATH: &str = "/cities500.txt";

static DEPRECATION_ONCE: OnceLock<()> = OnceLock::new();
static CITY_INDEX: LazyLock<Option<CityIndex>> = LazyLock::new(load_city_index);

struct CityEntry {
    name: String,
    lat: f64,
    lon: f64,
}

impl RTreeObject for CityEntry {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.lon, self.lat])
    }
}

impl PointDistance for CityEntry {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        let dx = self.lon - point[0];
        let dy = self.lat - point[1];
        dx * dx + dy * dy
    }
}

struct CityIndex {
    tree: RTree<CityEntry>,
}

fn maybe_warn_deprecated() {
    if env::var("BIGDATA_CLOUD_API_KEY").is_ok() {
        DEPRECATION_ONCE.get_or_init(|| {
            log::warn!(
                "BIGDATA_CLOUD_API_KEY is deprecated and ignored; offline city resolution via cities500 is used. Remove it from compose/env."
            );
        });
    }
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0088;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    R * c
}

fn get_cities500_path() -> String {
    env::var("CITIES500_PATH").unwrap_or_else(|_| CITIES500_PATH.to_string())
}

fn load_city_index() -> Option<CityIndex> {
    let path = get_cities500_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!(
                "cities500 data not found at {}; city resolution disabled: {}",
                path,
                e
            );
            return None;
        }
    };

    let mut entries: Vec<CityEntry> = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 6 {
            log::warn!(
                "skipping malformed cities500 line {}: expected >=6 cols, got {}",
                line_no + 1,
                cols.len()
            );
            continue;
        }
        let name = cols[1].to_string();
        if name.trim().is_empty() {
            log::warn!("skipping cities500 line {}: empty name", line_no + 1);
            continue;
        }
        let lat: f64 = match cols[4].parse() {
            Ok(v) => v,
            Err(_) => {
                log::warn!(
                    "skipping cities500 line {}: invalid latitude '{}'",
                    line_no + 1,
                    cols[4]
                );
                continue;
            }
        };
        let lon: f64 = match cols[5].parse() {
            Ok(v) => v,
            Err(_) => {
                log::warn!(
                    "skipping cities500 line {}: invalid longitude '{}'",
                    line_no + 1,
                    cols[5]
                );
                continue;
            }
        };
        entries.push(CityEntry { name, lat, lon });
    }

    if entries.is_empty() {
        log::warn!(
            "cities500 data at {} contained no valid entries; city resolution disabled",
            path
        );
        return None;
    }

    let len = entries.len();
    let tree = RTree::bulk_load(entries);
    log::info!("loaded {} cities from {}", len, path);
    Some(CityIndex { tree })
}

fn get_city_index() -> Option<&'static CityIndex> {
    CITY_INDEX.as_ref()
}

/// Returns the city name for the specified geo location
/// Resolved offline from the embedded GeoNames cities500 dataset.
/// Returns `None` for invalid coordinates or when no city is within `MAX_DISTANCE_KM`.
pub async fn resolve_city_name(geo_location: GeoLocation) -> Option<String> {
    maybe_warn_deprecated();

    // Validation (FR-004)
    if geo_location.latitude.is_nan()
        || geo_location.longitude.is_nan()
        || geo_location.latitude < -90.0
        || geo_location.latitude > 90.0
        || geo_location.longitude < -180.0
        || geo_location.longitude > 180.0
    {
        return None;
    }

    let index = get_city_index()?;
    let point = [geo_location.longitude as f64, geo_location.latitude as f64];
    let nearest = index.tree.nearest_neighbor(&point)?;

    let dist = haversine_km(
        geo_location.latitude as f64,
        geo_location.longitude as f64,
        nearest.lat,
        nearest.lon,
    );

    if dist <= MAX_DISTANCE_KM {
        Some(nearest.name.clone())
    } else {
        None
    }
}
