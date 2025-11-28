use std::convert::Infallible;

use bytes::Bytes;
use warp::http::{Response, StatusCode};
use warp::hyper::Body;

pub async fn index() -> Result<Response<Body>, Infallible> {
    let html = include_str!("../web-app/index.html");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html")
        .body(Body::from(html))
        .unwrap())
}

pub async fn style_css() -> Result<Response<Body>, Infallible> {
    let css = include_str!("../web-app/style.css");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/css")
        .body(Body::from(css))
        .unwrap())
}

pub async fn script_js() -> Result<Response<Body>, Infallible> {
    let js = include_str!("../web-app/script.js");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/javascript")
        .body(Body::from(js))
        .unwrap())
}

pub async fn hide_png() -> Result<Response<Body>, Infallible> {
    let hide_icon: &[u8] = include_bytes!("../web-app/images/hide.png");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "image/png")
        .body(Body::from(Bytes::from_static(hide_icon)))
        .unwrap())
}

pub async fn icon_png() -> Result<Response<Body>, Infallible> {
    let icon: &[u8] = include_bytes!("../icon.png");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "image/png")
        .body(Body::from(Bytes::from_static(icon)))
        .unwrap())
}

pub async fn font() -> Result<Response<Body>, Infallible> {
    let font: &[u8] = include_bytes!("../web-app/fonts/Inter-Regular.ttf");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "font/ttf")
        .body(Body::from(Bytes::from_static(font)))
        .unwrap())
}
