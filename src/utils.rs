use chrono::NaiveDateTime;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn to_date_time(system_time: SystemTime) -> NaiveDateTime {
    NaiveDateTime::from_timestamp(
        system_time
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::new(0, 0))
            .as_secs() as i64,
        0,
    )
}
