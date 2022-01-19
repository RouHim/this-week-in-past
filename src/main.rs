use std::sync::{Arc, Mutex};

use actix_files::Files;
use actix_web::{App, get, HttpResponse, HttpServer, middleware, web};
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
    println!("Launching webserver 🚀");
    let http_server_result = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(kv_reader.clone()))
            .data(web_dav_client.clone())
            .wrap(middleware::Logger::default()) // enable logger
            .service(
                web::scope("/api/resources")
                    .service(list_resources)
                    .service(get_resource)
                    .service(get_resource_metadata)
            )
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

#[get("")]
async fn list_resources(kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let keys: Vec<String> = resource_processor::get_this_week_in_past(kv_reader.as_ref());

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

#[get("{resource_id}")]
async fn get_resource(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>, web_dav_client: web::Data<WebDavClient>) -> HttpResponse {
    println!("requesting resource with id: {}", resources_id);

    let guard = kv_reader.get(resources_id.as_str()).unwrap();
    let web_dav_resource: WebDavResource = serde_json::from_str(guard.get_one().unwrap()).unwrap();
    let response_data = web_dav_client.request_resource_data(&web_dav_resource).bytes().unwrap();

    HttpResponse::Ok()
        .content_type(web_dav_resource.content_type)
        .body(response_data.to_vec())
}

#[get("{resource_id}/metadata")]
async fn get_resource_metadata(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    println!("requesting resource with id: {}", resources_id);

    let guard = kv_reader.get(resources_id.as_str()).unwrap();
    let data = guard.get_one().unwrap();

    HttpResponse::Ok()
        .content_type("application/json")
        .body(data)
}

