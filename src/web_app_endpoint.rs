use actix_web::get;
use actix_web::HttpResponse;

#[get("/")]
pub async fn index() -> HttpResponse {
    let html = include_str!("../web-app/index.html");
    HttpResponse::Ok().content_type("text/html").body(html)
}

#[get("/style.css")]
pub async fn style_css() -> HttpResponse {
    let css = include_str!("../web-app/style.css");
    HttpResponse::Ok().content_type("text/css").body(css)
}

#[get("/script.js")]
pub async fn script_js() -> HttpResponse {
    let js = include_str!("../web-app/script.js");
    HttpResponse::Ok().content_type("text/javascript").body(js)
}

#[get("/images/hide.png")]
pub async fn hide_png() -> HttpResponse {
    let hide_icon: &[u8] = include_bytes!("../web-app/images/hide.png");
    HttpResponse::Ok().content_type("image/png").body(hide_icon)
}

#[get("/icon.png")]
pub async fn icon_png() -> HttpResponse {
    let icon: &[u8] = include_bytes!("../icon.png");
    HttpResponse::Ok().content_type("image/png").body(icon)
}

#[get("/font.ttf")]
pub async fn font() -> HttpResponse {
    let font: &[u8] = include_bytes!("../web-app/fonts/Inter-Regular.ttf");
    HttpResponse::Ok().content_type("font/ttf").body(font)
}
