use std::env;

use actix_web::get;
use actix_web::HttpResponse;

use crate::weather_processor;

#[get("")]
pub async fn get_is_weather_enabled() -> HttpResponse {
    let is_weather_enabled = env::var("WEATHER_ENABLED").unwrap_or_else(|_| "true".to_string());

    HttpResponse::Ok()
        .content_type("plain/text")
        .body(is_weather_enabled)
}

#[get("current")]
pub async fn get_current_weather() -> HttpResponse {
    let weather_data = weather_processor::get_current_weather().await;

    if let Some(weather_data) = weather_data {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(weather_data)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("homeassistant")]
pub async fn get_is_home_assistant_enabled() -> HttpResponse {
    let is_home_assistant_enabled = env::var("HOME_ASSISTANT_BASE_URL").is_ok()
        && env::var("HOME_ASSISTANT_API_TOKEN").is_ok()
        && env::var("HOME_ASSISTANT_ENTITY_ID").is_ok();

    HttpResponse::Ok()
        .content_type("plain/text")
        .body(is_home_assistant_enabled.to_string())
}

#[get("homeassistant/temperature")]
pub async fn get_home_assistant_entity_data() -> HttpResponse {
    let weather_data = weather_processor::get_home_assistant_data().await;

    if let Some(weather_data) = weather_data {
        HttpResponse::Ok()
            .content_type("plain/text")
            .body(weather_data)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}
