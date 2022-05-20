use std::env;

use actix_web::get;
use actix_web::HttpResponse;

#[get("interval")]
pub async fn get_slideshow_interval() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("plain/text")
        .body(env::var("SLIDESHOW_INTERVAL").unwrap_or_else(|_| "10000".to_string()))
}
