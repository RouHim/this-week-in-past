use crate::config;
use std::convert::Infallible;
use std::env;
use warp::http::{Response, StatusCode};
use warp::hyper::Body;

pub async fn get_slideshow_interval() -> Result<Response<Body>, Infallible> {
    Ok(plain_response(
        config::get_slideshow_interval_value().to_string(),
    ))
}

pub async fn get_refresh_interval() -> Result<Response<Body>, Infallible> {
    Ok(plain_response(
        config::get_refresh_interval_value().to_string(),
    ))
}

pub async fn get_hide_button_enabled() -> Result<Response<Body>, Infallible> {
    Ok(plain_response(
        env::var("SHOW_HIDE_BUTTON").unwrap_or_else(|_| "false".to_string()),
    ))
}

pub async fn get_random_slideshow_enabled() -> Result<Response<Body>, Infallible> {
    Ok(plain_response(
        env::var("RANDOM_SLIDESHOW").unwrap_or_else(|_| "false".to_string()),
    ))
}

pub async fn get_preload_images_enabled() -> Result<Response<Body>, Infallible> {
    Ok(plain_response(
        env::var("PRELOAD_IMAGES").unwrap_or_else(|_| "false".to_string()),
    ))
}

fn plain_response(body: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "plain/text")
        .body(Body::from(body))
        .unwrap()
}
