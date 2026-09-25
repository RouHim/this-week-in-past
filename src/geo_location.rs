use std::env;
use std::fmt::{Display, Formatter};
use std::sync::OnceLock;

use actix_web::web;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use crate::city_index::CityIndex;
use crate::country_names::country_name;

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

/// Maximum distance from query point to parent city for hierarchical display.
/// Covers Bayenthal→Köln ~4km, Volksdorf→Hamburg ~12km, Christianshavn→København ~2km.
const MAX_PARENT_DISTANCE_KM: f64 = 30.0;

/// Default path of the cities500 data file inside the container.
const CITIES500_PATH: &str = "/cities500.txt";

static DEPRECATION_ONCE: OnceLock<()> = OnceLock::new();
static CITY_INDEX: OnceLock<Option<CityIndex>> = OnceLock::new();
static CITY_INDEX_INIT_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

static HOME_COUNTRY: RwLock<Option<String>> = RwLock::new(None);

/// Parses a raw `HOME_COUNTRY` value: trims and uppercases; empty means unset.
/// Returns an error naming the variable and value when not a known ISO code.
pub fn parse_home_country(raw: &str) -> Result<Option<String>, String> {
    let code = raw.trim().to_ascii_uppercase();
    if code.is_empty() {
        return Ok(None);
    }
    if country_name(&code).is_some() {
        Ok(Some(code))
    } else {
        Err(format!(
            "HOME_COUNTRY=\"{raw}\" is not a known ISO-3166 alpha-2 country code"
        ))
    }
}

/// Reads `HOME_COUNTRY` once at startup; panics on invalid value.
pub fn init_home_country() {
    match parse_home_country(&env::var("HOME_COUNTRY").unwrap_or_default()) {
        Ok(code) => *HOME_COUNTRY.write() = code,
        Err(e) => panic!("{e}"),
    }
}

fn home_country() -> Option<String> {
    HOME_COUNTRY.read().clone()
}

#[cfg(test)]
pub(crate) fn set_home_country_for_tests(code: Option<String>) {
    *HOME_COUNTRY.write() = code;
}

#[cfg(test)]
pub(crate) fn home_country_for_tests() -> Option<String> {
    home_country()
}

/// Appends the English country name unless the photo is home, unknown, or unset.
fn apply_home_country(display: &str, photo_country: &str) -> String {
    if photo_country.is_empty() {
        return display.to_string();
    }
    match home_country().as_deref() {
        None => display.to_string(),
        Some(home) if home == photo_country => display.to_string(),
        Some(_) => match country_name(photo_country) {
            Some(name) => format!("{display}, {name}"),
            None => display.to_string(),
        },
    }
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

fn get_cities500_path() -> String {
    env::var("CITIES500_PATH").unwrap_or_else(|_| CITIES500_PATH.to_string())
}

fn load_city_index() -> Option<CityIndex> {
    let path = get_cities500_path();
    let index = CityIndex::load(&path)?;
    log::info!("loaded {} cities from {}", index.len(), path);
    Some(index)
}

fn get_city_index() -> Option<&'static CityIndex> {
    CITY_INDEX.get().and_then(|opt| opt.as_ref())
}

// Single-flight via tokio::sync::Mutex + double-checked locking. First caller
// holds the mutex while doing web::block(load_city_index); concurrent callers
// await the mutex, re-check CITY_INDEX, and reuse the winner's index.
async fn ensure_city_index() -> Option<&'static CityIndex> {
    if let Some(opt) = CITY_INDEX.get() {
        return opt.as_ref();
    }
    let _guard = CITY_INDEX_INIT_MUTEX.lock().await;
    if let Some(opt) = CITY_INDEX.get() {
        return opt.as_ref();
    }
    let loaded: Option<CityIndex> = match web::block(load_city_index).await {
        Ok(opt) => opt,
        Err(e) => {
            log::warn!("cities500 load blocked task failed: {}", e);
            return None;
        }
    };
    let _ = CITY_INDEX.set(loaded);
    get_city_index()
}

/// Returns the city name for the specified geo location
/// Resolved offline from the embedded GeoNames cities500 dataset.
/// Returns `None` for invalid coordinates or when no city is within `MAX_DISTANCE_KM`.
pub async fn resolve_city_name(geo_location: GeoLocation) -> Option<String> {
    maybe_warn_deprecated();

    // Validation: finite and in-range (is_finite covers NaN and ±inf)
    if !geo_location.latitude.is_finite()
        || !geo_location.longitude.is_finite()
        || geo_location.latitude < -90.0
        || geo_location.latitude > 90.0
        || geo_location.longitude < -180.0
        || geo_location.longitude > 180.0
    {
        return None;
    }

    let index = ensure_city_index().await?;
    let latitude = geo_location.latitude as f64;
    let longitude = geo_location.longitude as f64;

    let nearest = index.nearest_within(latitude, longitude, MAX_DISTANCE_KM)?;

    if !nearest.is_district() {
        return Some(apply_home_country(nearest.name(), nearest.country_code()));
    }

    // District → parent resolution: intentionally NOT pure closest-haversine, we pick the most
    // populous city within MAX_PARENT_DISTANCE_KM (population desc, distance asc tie-breaker).
    // This matches product expectations (Volksdorf 53.651,10.166 → Hamburg ~12–16 km, 1.8M over
    // nearer Ahrensburg ~6 km, 33k) and Bayenthal→Köln (~4 km), while a strict minimum-distance
    // rule would surprise users in dense metro areas.
    match index.most_populous_parent_within(latitude, longitude, MAX_PARENT_DISTANCE_KM) {
        Some(parent) => {
            log::debug!(
                "district '{}' -> parent '{}' (pop {}, {:.1}km) for {}",
                nearest.name(),
                parent.name(),
                parent.population(),
                parent.distance_km(),
                geo_location
            );
            Some(apply_home_country(
                &format!("{}, {}", nearest.name(), parent.name()),
                parent.country_code(),
            ))
        }
        None => {
            log::debug!(
                "district '{}' has no parent within {}km for {}",
                nearest.name(),
                MAX_PARENT_DISTANCE_KM,
                geo_location
            );
            Some(apply_home_country(nearest.name(), nearest.country_code()))
        }
    }
}
