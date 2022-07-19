use std::env;

use actix_web::get;
use actix_web::HttpResponse;

#[get("interval/slideshow")]
pub async fn get_slideshow_interval() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("plain/text")
        .body(env::var("SLIDESHOW_INTERVAL").unwrap_or_else(|_| "30".to_string()))
}

#[get("interval/refresh")]
pub async fn get_refresh_interval() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("plain/text")
        .body(env::var("REFRESH_INTERVAL").unwrap_or_else(|_| "180".to_string()))
}
