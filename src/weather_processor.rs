use crate::config;
use std::env;

/// Returns the current weather data provided by OpenWeatherMap
/// The data is selected by the configured location
/// Returns None if the data could not be retrieved or the weather json data
pub async fn get_current_weather() -> Option<String> {
    if env::var("OPEN_WEATHER_MAP_API_KEY").is_err() {
        return None;
    }

    let api_key: String = env::var("OPEN_WEATHER_MAP_API_KEY").unwrap();
    let city: String = env::var("WEATHER_LOCATION").unwrap_or_else(|_| "Berlin".to_string());
    let units: String = config::get_weather_unit();
    let language: String = env::var("WEATHER_LANGUAGE").unwrap_or_else(|_| "en".to_string());
    let response = ureq::get(format!(
        "https://api.openweathermap.org/data/2.5/weather?q={city}&appid={api_key}&units={units}&lang={language}"
    ).as_str()).call();

    if let Ok(response) = response {
        response.into_string().ok()
    } else {
        None
    }
}

/// Returns the current weather data provided by Home Assistant
/// if the Home Assistant integration is enabled
/// and entity_id is found
pub async fn get_home_assistant_data() -> Option<String> {
    let base_url = env::var("HOME_ASSISTANT_BASE_URL").ok();
    let api_token = env::var("HOME_ASSISTANT_API_TOKEN").ok();
    let entity_id = env::var("HOME_ASSISTANT_ENTITY_ID").ok();

    if base_url.is_none() || api_token.is_none() || entity_id.is_none() {
        return None;
    }

    let response =
        ureq::get(format!("{}/api/states/{}", base_url.unwrap(), entity_id.unwrap()).as_str())
            .set(
                "Authorization",
                format!("Bearer {}", api_token.unwrap()).as_str(),
            )
            .call();

    if let Ok(response) = response {
        response.into_string().ok()
    } else {
        None
    }
}
