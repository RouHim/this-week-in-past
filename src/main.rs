use std::sync::{Arc, Mutex};

use actix_files::Files;
use actix_web::{App, HttpResponse, HttpServer, middleware, web};
use evmap::ReadHandle;
use reqwest::Error;

mod scheduler;
mod web_dav_client;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let (kv_reader, kv_writer) = evmap::new::<String, String>();

    let kv_writer_mutex = Arc::new(Mutex::new(kv_writer));
    let scheduler_handle = scheduler::run(kv_writer_mutex.clone());
    scheduler::fetch_images(kv_writer_mutex.clone());

    let http_server_result = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(kv_reader.clone()))
            .wrap(middleware::Logger::default()) // enable logger
            .route("/api/resources", web::get().to(api_get_all_resources_handler))
            // .route("/api/resources/{resources-id}", web::get().to(api_get_resource_handler))
            // .route("/api/resources/{resources-id}/metadata", web::post().to(api_get_resource_metadata_handler))
            .service(Files::new("/", "./static/").index_file("index.html"))
    })
        .bind("0.0.0.0:8080")?
        .run()
        .await;

    scheduler_handle.stop();

    return http_server_result;
}

async fn api_get_all_resources_handler(kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse  {
    let keys: Vec<String> = kv_reader.read().unwrap().iter().map(|(k,v)| k.clone()).collect();

    HttpResponse::Ok().content_type("application/json").body(serde_json::to_string(&keys).unwrap())

}

