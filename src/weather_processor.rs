use std::env;

/// Returns the current weather data provided by OpenWeatherMap
/// The data is selected by the configured location
/// Returns None if the data could not be retrieved or the weather json data
pub async fn get_current_weather() -> Option<String> {
    if env::var("OPEN_WEATHER_MAP_API_KEY").is_err() {
        return None;
    }

    let api_key: String = env::var("OPEN_WEATHER_MAP_API_KEY").unwrap();
    let city: String = env::var("LOCATION_NAME").unwrap_or_else(|_| "Berlin".to_string());
    let units: String = env::var("UNITS").unwrap_or_else(|_| "metric".to_string());
    let language: String = env::var("LANGUAGE").unwrap_or_else(|_| "en".to_string());

    let response = reqwest::get(format!(
        "https://api.openweathermap.org/data/2.5/weather?q={city}&appid={api_key}&units={units}&lang={language}"
    )).await;

    if response.is_ok() {
        response.unwrap().text().await.ok()
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

    let response = reqwest::Client::new()
        .get(format!(
            "{}/api/states/{}",
            base_url.unwrap(),
            entity_id.unwrap()
        ))
        .bearer_auth(api_token.unwrap())
        .send()
        .await;

    if response.is_ok() {
        response.unwrap().text().await.ok()
    } else {
        None
    }
}
