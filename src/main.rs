use std::sync::{Arc, Mutex};

use actix_files::Files;
use actix_web::{App, HttpResponse, HttpServer, middleware, web};
use evmap::ReadHandle;

use crate::web_dav_client::{WebDavClient, WebDavResource};

mod scheduler;
mod web_dav_client;
mod resource_processor;
mod exif_reader;
mod geo_location;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Build webdav client
    let web_dav_client = web_dav_client::new(
        "https://photos.himmelstein.info",
        "admin",
        "hPjCqWh5#P8c*r9XijqE",
    );

    // Initialize kv_store reader and writer
    let (kv_reader, kv_writer) = evmap::new::<String, String>();
    // Build arc mutex of kv_store writer (we have multiple writer)
    let kv_writer_mutex = Arc::new(Mutex::new(kv_writer));

    // Start scheduler to run at midnight
    let scheduler_handle = scheduler::initialize(
        web_dav_client.clone(),
        kv_writer_mutex.clone(),
    );

    // Fetch resources for the first time
    scheduler::fetch_resources(
        web_dav_client.clone(),
        kv_writer_mutex.clone(),
    );

    // Run the actual web server and hold the main thread here
    let http_server_result = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(kv_reader.clone()))
            .data(web::Data::new(web_dav_client.clone()))
            .wrap(middleware::Logger::default()) // enable logger
            .route("/api/resources", web::get().to(api_resources_handler))
            // .route("/api/resources/{resources-id}", web::get().to(api_get_resource_handler))
            // .route("/api/resources/{resources-id}/metadata", web::post().to(api_get_resource_metadata_handler))
            .service(Files::new("/", "./static/").index_file("index.html"))
    })
        .bind("0.0.0.0:8080")?
        .run()
        .await;

    // If the http server is terminated, stop also the scheduler
    scheduler_handle.stop();

    // Done, let's get out here
    http_server_result
}

async fn api_resources_handler(kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let keys: Vec<String> = resource_processor::get_this_week_in_past(kv_reader.as_ref());

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

