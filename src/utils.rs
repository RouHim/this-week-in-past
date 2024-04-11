use chrono::{DateTime, NaiveDateTime};
use std::time::{SystemTime, UNIX_EPOCH};

/// Converts the type `SystemTime` to `NaiveDateTime`
pub fn to_date_time(system_time: SystemTime) -> NaiveDateTime {
    DateTime::from_timestamp(
        system_time
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::new(0, 0))
            .as_secs() as i64,
        0,
    )
    .unwrap()
    .naive_utc()
}

/// Returns a md5 string based on a given string
pub fn md5(string: &str) -> String {
    format!("{:x}", md5::compute(string.as_bytes()))
}
