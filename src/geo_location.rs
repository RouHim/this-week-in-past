use std::env;
use std::fmt::{Display, Formatter};
use std::sync::OnceLock;

use actix_web::web;
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

/// Maximum distance from query point to parent city for hierarchical display.
/// Covers Bayenthal→Köln ~4km, Volksdorf→Hamburg ~12km, Christianshavn→København ~2km.
const MAX_PARENT_DISTANCE_KM: f64 = 30.0;

/// Default path of the cities500 data file inside the container.
const CITIES500_PATH: &str = "/cities500.txt";

static DEPRECATION_ONCE: OnceLock<()> = OnceLock::new();
static CITY_INDEX: OnceLock<Option<CityIndex>> = OnceLock::new();
static CITY_INDEX_INIT_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone)]
struct CityEntry {
    name: String,
    lat: f64,
    lon: f64,
    feature_class: String,
    feature_code: String,
    // country_code/admin1_code parsed per FR-001/FR-005, reserved for future
    // 3-level display (District, City, Country); retained despite not yet
    // rendered. Heap budget documented at load site: steady-state <50 MB
    // (FR-008); transient peak 60-85 MB before bulk_load moves vectors.
    #[allow(dead_code)]
    country_code: String,
    #[allow(dead_code)]
    admin1_code: String,
    population: i64,
}

impl RTreeObject for CityEntry {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.lon, self.lat])
    }
}

impl PointDistance for CityEntry {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        // Plain Euclidean — must match AABB::distance_2 used for R-tree
        // internal nodes (rstar contract). Antimeridian wrap is handled
        // at query time by probing lon±360.
        let dx = self.lon - point[0];
        let dy = self.lat - point[1];
        dx * dx + dy * dy
    }
}

struct CityIndex {
    full_tree: RTree<CityEntry>,
    parent_tree: RTree<CityEntry>,
}

fn is_parent_city(entry: &CityEntry) -> bool {
    entry.feature_class == "P"
        && matches!(
            entry.feature_code.as_str(),
            "PPL" | "PPLA" | "PPLA2" | "PPLA3" | "PPLA4" | "PPLC" | "PPLG" | "PPLS"
        )
}

fn is_district(entry: &CityEntry) -> bool {
    entry.feature_class == "P" && entry.feature_code == "PPLX"
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
    let c = 2.0 * a.clamp(0.0, 1.0).sqrt().asin();
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
        // Extended columns — tolerant parsing (FR-001 edge: incomplete lines)
        let feature_class = cols.get(6).unwrap_or(&"").trim().to_string();
        let feature_code = cols.get(7).unwrap_or(&"").trim().to_string();
        let country_code = cols.get(8).unwrap_or(&"").trim().to_ascii_uppercase();
        let admin1_code = cols.get(10).unwrap_or(&"").trim().to_string();
        let population: i64 = cols
            .get(14)
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(0);
        entries.push(CityEntry {
            name,
            lat,
            lon,
            feature_class,
            feature_code,
            country_code,
            admin1_code,
            population,
        });
    }

    if entries.is_empty() {
        log::warn!(
            "cities500 data at {} contained no valid entries; city resolution disabled",
            path
        );
        return None;
    }

    let len = entries.len();
    // Peak transient heap ~60-85 MB (FR-008): entries Vec with 5 Strings per
    // entry (~30 MB), parent clone of filtered parents (~10-15 MB), plus two RTree
    // node allocations (~2×15 MB). Steady-state after bulk_load moves vectors
    // into RTrees is <50 MB. Single-flight in ensure_city_index prevents
    // N× peak burst under concurrent startup; clone is kept for simplicity
    // (Rc/Arc would add indirection without measurable win).
    let parent_entries: Vec<CityEntry> = entries
        .iter()
        .filter(|e| is_parent_city(e))
        .cloned()
        .collect();
    let district_count = entries.iter().filter(|e| is_district(e)).count();
    let parent_count = parent_entries.len();
    let full_tree = RTree::bulk_load(entries);
    let parent_tree = RTree::bulk_load(parent_entries);
    log::info!(
        "loaded {} cities ({} parents, {} districts) from {}",
        len,
        parent_count,
        district_count,
        path
    );
    Some(CityIndex {
        full_tree,
        parent_tree,
    })
}

fn get_city_index() -> Option<&'static CityIndex> {
    CITY_INDEX.get().and_then(|opt| opt.as_ref())
}

// Single-flight via tokio::sync::Mutex + double-checked locking. First caller
// holds the mutex while doing web::block(load_city_index) (~60-85 MB transient,
// steady-state <50 MB per FR-008 above); concurrent callers await the mutex,
// re-check CITY_INDEX, and reuse the winner's index, preventing N×50 MB burst.
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
    let lon = geo_location.longitude as f64;
    let lat = geo_location.latitude as f64;
    let point = [lon, lat];
    // Plain Euclidean cannot wrap at the antimeridian. Probe the
    // wrapped equivalent (lon±360) so a city just across the date
    // line is found via the second R-tree query; the true nearest
    // is selected below by haversine (which naturally wraps).
    let alt_lon = if lon >= 0.0 { lon - 360.0 } else { lon + 360.0 };
    let alt_point = [alt_lon, lat];

    // Euclidean ordering diverges from haversine (lon shrinks by cos(lat)),
    // so the Euclidean-nearest may be outside 50 km while a slightly farther
    // Euclidean candidate is inside. Scan k nearest and pick the closest
    // haversine within threshold from both query points.
    let mut best: Option<(&CityEntry, f64)> = None;
    for query_point in [point, alt_point] {
        for candidate in index.full_tree.nearest_neighbor_iter(&query_point).take(20) {
            let dist = haversine_km(lat, lon, candidate.lat, candidate.lon);
            if dist <= MAX_DISTANCE_KM {
                match &best {
                    Some((_, best_dist)) if dist >= *best_dist => {}
                    _ => best = Some((candidate, dist)),
                }
            }
        }
    }

    let best_entry = match best {
        Some((entry, _)) => entry,
        None => return None,
    };

    if !is_district(best_entry) {
        return Some(best_entry.name.clone());
    }
    // District → parent resolution (FR-003, see .spec/district-aware-city-display.md).
    // Intentionally NOT pure closest-haversine: we pick the most populous city within
    // MAX_PARENT_DISTANCE_KM (population desc, haversine asc tie-breaker). This matches
    // product expectations for Scenario 1 (Volksdorf 53.651,10.166 → Hamburg ~12–16km, 1.8M
    // over nearer Ahrensburg ~6km, 33k) and Bayenthal→Köln (~4km) ties, while a strict
    // minimum-distance rule would surprise users in dense metro areas. Candidates are
    // collected from parent_tree.nearest_neighbor_iter(..).take(100) for each antimeridian
    // probe, filtered by haversine ≤30km, then deduped by name+coords.
    let mut candidates: Vec<(&CityEntry, f64)> = Vec::new();
    for query_point in [point, alt_point] {
        for candidate in index
            .parent_tree
            .nearest_neighbor_iter(&query_point)
            .take(100)
        {
            let dist = haversine_km(lat, lon, candidate.lat, candidate.lon);
            if dist <= MAX_PARENT_DISTANCE_KM {
                candidates.push((candidate, dist));
            }
        }
    }
    // Deduplicate by name+coords to avoid double counting from antimeridian probes
    candidates.sort_by(|a, b| {
        a.0.name
            .cmp(&b.0.name)
            .then_with(|| a.0.lat.to_bits().cmp(&b.0.lat.to_bits()))
            .then_with(|| a.0.lon.to_bits().cmp(&b.0.lon.to_bits()))
    });
    candidates.dedup_by(|a, b| {
        a.0.name == b.0.name && (a.0.lat - b.0.lat).abs() < 1e-9 && (a.0.lon - b.0.lon).abs() < 1e-9
    });
    let best_parent = candidates.into_iter().max_by(|(a, da), (b, db)| {
        match a.population.cmp(&b.population) {
            std::cmp::Ordering::Equal => {
                // smaller distance wins → reverse ordering for max_by
                db.partial_cmp(da).unwrap_or(std::cmp::Ordering::Equal)
            }
            ord => ord,
        }
    });

    if let Some((parent, dist)) = best_parent {
        log::debug!(
            "district '{}' -> parent '{}' (pop {}, {:.1}km) for {}",
            best_entry.name,
            parent.name,
            parent.population,
            dist,
            geo_location
        );
        Some(format!("{}, {}", best_entry.name, parent.name))
    } else {
        log::debug!(
            "district '{}' has no parent within {}km for {}",
            best_entry.name,
            MAX_PARENT_DISTANCE_KM,
            geo_location
        );
        Some(best_entry.name.clone())
    }
}
