use std::convert::Infallible;
use std::env;
use warp::http::{Response, StatusCode};
use warp::hyper::Body;

use crate::{config, weather_processor};

pub async fn get_is_weather_enabled() -> Result<Response<Body>, Infallible> {
    let is_weather_enabled = env::var("WEATHER_ENABLED").unwrap_or_else(|_| "false".to_string());

    Ok(text_response(is_weather_enabled))
}

pub async fn get_current_weather() -> Result<Response<Body>, Infallible> {
    let weather_data = weather_processor::get_current_weather().await;

    if let Some(weather_data) = weather_data {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(weather_data))
            .unwrap())
    } else {
        Ok(empty_response(StatusCode::INTERNAL_SERVER_ERROR))
    }
}

pub async fn get_is_home_assistant_enabled() -> Result<Response<Body>, Infallible> {
    let is_home_assistant_enabled = env::var("HOME_ASSISTANT_BASE_URL").is_ok()
        && env::var("HOME_ASSISTANT_API_TOKEN").is_ok()
        && env::var("HOME_ASSISTANT_ENTITY_ID").is_ok();

    Ok(text_response(is_home_assistant_enabled.to_string()))
}

pub async fn get_home_assistant_entity_data() -> Result<Response<Body>, Infallible> {
    let weather_data = weather_processor::get_home_assistant_data().await;

    if let Some(weather_data) = weather_data {
        Ok(text_response(weather_data))
    } else {
        Ok(empty_response(StatusCode::INTERNAL_SERVER_ERROR))
    }
}

pub async fn get_weather_unit() -> Result<Response<Body>, Infallible> {
    Ok(text_response(config::get_weather_unit()))
}

fn text_response(body: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "plain/text")
        .body(Body::from(body))
        .unwrap()
}

fn empty_response(status: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}
