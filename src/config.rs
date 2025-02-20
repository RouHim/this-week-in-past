use std::env;

pub fn get_slideshow_interval_value() -> usize {
    env::var("SLIDESHOW_INTERVAL")
        .unwrap_or_else(|_| "30".to_string())
        .parse()
        .unwrap_or(30)
}

pub fn get_refresh_interval_value() -> usize {
    env::var("REFRESH_INTERVAL")
        .unwrap_or_else(|_| "360".to_string())
        .parse()
        .unwrap_or(360)
}

pub fn get_weather_unit() -> String {
    env::var("WEATHER_UNIT").unwrap_or_else(|_| "metric".to_string())
}
