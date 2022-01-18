use std::fmt::{Display, Formatter};

use lazy_static::lazy_static;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct GeoLocation {
    pub latitude: f32,
    pub longitude: f32,
}

impl Display for GeoLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[lat={} lon={}]",
            self.latitude,
            self.longitude,
        )
    }
}

/// Converts Degrees Minutes Seconds To Decimal Degrees
fn dms_to_dd(dms_string: &str) -> Option<f32> {
    lazy_static! {
        static ref DMS_PARSE_PATTERN_1: Regex = Regex::new(
            // e.g.: 7 deg 33 min 55.5155 sec
            r"(?P<deg>\d+) deg (?P<min>\d+) min (?P<sec>\d+.\d+) sec"
        ).unwrap();
        static ref DMS_PARSE_PATTERN_2: Regex = Regex::new(
            // e.g.: 50/1, 25/1, 2519/100
            r"(?P<deg>\d+)/(?P<deg_fraction>\d+),\s*(?P<min>\d+)/(?P<min_fraction>\d+),\s*(?P<sec>\d+)/(?P<sec_fraction>\d+)"
        ).unwrap();
    }

    let dms_pattern_1_match: Option<Captures> = DMS_PARSE_PATTERN_1.captures(dms_string);
    let dms_pattern_2_match: Option<Captures> = DMS_PARSE_PATTERN_2.captures(dms_string);

    if let Some(pattern_match) = dms_pattern_1_match {
        parse_pattern_1(pattern_match)
    } else if let Some(pattern_match) = dms_pattern_2_match {
        parse_pattern_2(pattern_match)
    } else {
        println!("Could not parse location: {}", dms_string);
        None
    }
}

/// Parses Degrees minutes seconds for the following example pattern: "7 deg 33 min 55.5155 sec"
fn parse_pattern_1(caps: Captures) -> Option<f32> {
    let maybe_deg: Option<f32> = caps.name("deg").map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_min: Option<f32> = caps.name("min").map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_sec: Option<f32> = caps.name("sec").map(|cap| cap.as_str().parse::<f32>().unwrap());

    if let (Some(deg), Some(min), Some(sec)) = (maybe_deg, maybe_min, maybe_sec) {
        Some(deg + (min / 60.0) + (sec / 3600.0))
    } else {
        None
    }
}

/// Parses Degrees minutes seconds for the following example pattern: "50/1, 25/1, 2519/100"
fn parse_pattern_2(caps: Captures) -> Option<f32> {
    let maybe_deg: Option<f32> = caps.name("deg").map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_deg_fraction: Option<f32> = caps.name("deg_fraction").map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_min: Option<f32> = caps.name("min").map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_min_fraction: Option<f32> = caps.name("min_fraction").map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_sec: Option<f32> = caps.name("sec").map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_sec_fraction: Option<f32> = caps.name("sec_fraction").map(|cap| cap.as_str().parse::<f32>().unwrap());

    if let (
        Some(deg),
        Some(deg_frac),
        Some(min),
        Some(min_frac),
        Some(sec),
        Some(sec_frac)
    ) = (maybe_deg, maybe_deg_fraction, maybe_min, maybe_min_fraction, maybe_sec, maybe_sec_fraction) {
        Some((deg / deg_frac) + ((min / min_frac) / 60.0) + ((sec / sec_frac) / 3600.0))
    } else {
        None
    }
}

pub fn from_degrees_minutes_seconds(latitude: String, longitude: String) -> Option<GeoLocation> {
    let maybe_dd_lat = dms_to_dd(&latitude);
    let maybe_dd_lon = dms_to_dd(&longitude);

    if let (Some(latitude), Some(longitude)) = (maybe_dd_lat, maybe_dd_lon) {
        Some(GeoLocation { latitude, longitude })
    } else {
        None
    }
}