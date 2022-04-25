extern crate core;

use std::env;
use std::sync::{Arc, Mutex};

use actix_files::Files;
use actix_web::{App, HttpResponse, HttpServer, middleware, web};

mod scheduler;
mod resource_processor;
mod exif_reader;
mod geo_location;
mod resource_endpoint;
mod image_processor;

#[cfg(test)]
mod resource_processor_test;
#[cfg(test)]
mod resource_reader_test;
mod resource_reader;

pub const CACHE_DIR: &str = "./cache";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Build webdav client
    let resource_reader = resource_reader::new(
        env::var("RESOURCE_PATHS").expect("RESOURCE_PATHS is missing").as_str()
    );

    // Initialize kv_store reader and writer
    let (kv_reader, kv_writer) = evmap::new::<String, String>();
    // Build arc mutex of kv_store writer, we need this exact instance (cause, we have multiple writer)
    let kv_writer_mutex = Arc::new(Mutex::new(kv_writer));

    // Start scheduler to run at midnight
    scheduler::init();
    let scheduler_handle = scheduler::run_webdav_indexer(
        resource_reader.clone(),
        kv_writer_mutex.clone(),
    );

    // Fetch resources for the first time
    scheduler::fetch_resources(
        resource_reader.clone(),
        kv_writer_mutex.clone(),
    );

    // Run the actual web server and hold the main thread here
    println!("Launching webserver 🚀");
    let http_server_result = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(kv_reader.clone()))
            .app_data(resource_reader.clone())
            .wrap(middleware::Logger::default()) // enable logger
            .service(web::scope("/api/resources")
                .service(resource_endpoint::list_all_resources)
                .service(resource_endpoint::list_this_week_resources)
                .service(resource_endpoint::random_resource)
                .service(resource_endpoint::get_resource_by_id_and_resolution)
                .service(resource_endpoint::get_resource_base64_by_id_and_resolution)
                .service(resource_endpoint::get_resource_metadata_by_id)
                .service(resource_endpoint::get_resource_metadata_description_by_id)
            )
            .service(web::resource("/api/health").route(web::get().to(HttpResponse::Ok)))
            .service(Files::new("/", "./static/").index_file("index.html"))
    })
        .bind("0.0.0.0:8080")?
        .run()
        .await;

    // If the http server is terminated, stop also the scheduler
    println!("Stopping Scheduler 🕐️");
    scheduler_handle.stop();

    println!("Stopping Application 😵️");
    // Done, let's get out here
    http_server_result
}