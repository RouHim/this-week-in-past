use std::collections::HashMap;
use std::env;
use std::fmt::{Display, Formatter};

use lazy_static::lazy_static;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        ).unwrap();
        static ref DMS_PARSE_PATTERN_2: Regex = Regex::new(
            // e.g.: 50/1, 25/1, 2519/100
            r"(?P<deg>\d+)/(?P<deg_fraction>\d+),\s*(?P<min>\d+)/(?P<min_fraction>\d+),\s*(?P<sec>\d+)/(?P<sec_fraction>\d+)"
        ).unwrap();
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

/// Returns the city name for the specified geo location
/// The city name is resolved from the geo location using the bigdatacloud api
pub async fn resolve_city_name(geo_location: GeoLocation) -> Option<String> {
    if env::var("BIGDATA_CLOUD_API_KEY").is_err() {
        return None;
    }

    let request_url = format!(
        "https://api.bigdatacloud.net/data/reverse-geocode?latitude={}&longitude={}&localityLanguage=de&key={}",
        geo_location.latitude,
        geo_location.longitude,
        env::var("BIGDATA_CLOUD_API_KEY").unwrap(),
    );

    let response = ureq::get(request_url.as_str()).call();

    if response.is_err() {
        return None;
    }

    let response_json = response
        .unwrap()
        .body_mut()
        .read_to_string()
        .ok()
        .and_then(|json_string| serde_json::from_str::<HashMap<String, Value>>(&json_string).ok());

    let mut city_name = response_json
        .as_ref()
        .and_then(|json_data| get_string_value("city", json_data))
        .filter(|city_name| !city_name.trim().is_empty());

    if city_name.is_none() {
        city_name = response_json
            .as_ref()
            .and_then(|json_data| get_string_value("locality", json_data))
            .filter(|city_name| !city_name.trim().is_empty());
    }

    city_name
}

/// Returns the string value for the specified key of an hash map
fn get_string_value(field_name: &str, json_data: &HashMap<String, Value>) -> Option<String> {
    json_data
        .get(field_name)
        .and_then(|field_value| field_value.as_str())
        .map(|field_string_value| field_string_value.to_string())
}
