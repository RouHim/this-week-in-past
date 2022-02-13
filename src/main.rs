use std::env;
use std::sync::{Arc, Mutex};

use actix_files::Files;
use actix_web::{App, get, HttpServer, middleware, web};

use crate::web_dav_client::{WebDavClient, WebDavResource};

mod scheduler;
mod web_dav_client;
mod resource_processor;
mod exif_reader;
mod geo_location;
mod resource_endpoint;
mod image_processor;

#[cfg(test)]
mod resource_processor_test;
#[cfg(test)]
mod image_processor_test;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Build webdav client
    let web_dav_client = web_dav_client::new(
        env::var("TWIP_WEBDAV_BASE_URL").expect("TWIP_WEBDAV_BASE_URL is missing").as_str(),
        env::var("TWIP_USERNAME").expect("TWIP_USERNAME is missing").as_str(),
        env::var("TWIP_PASSWORD").expect("TWIP_PASSWORD is missing").as_str(),
    );

    // Initialize kv_store reader and writer
    let (kv_reader, kv_writer) = evmap::new::<String, String>();
    // Build arc mutex of kv_store writer, we need this exact instance (cause, we have multiple writer)
    let kv_writer_mutex = Arc::new(Mutex::new(kv_writer));

    // Start scheduler to run at midnight
    let scheduler_handle = scheduler::run_webdav_indexer(
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
                    .service(resource_endpoint::list_resources)
                    .service(resource_endpoint::list_this_week_resources)
                    .service(resource_endpoint::get_resource)
                    .service(resource_endpoint::get_resource_base64)
                    .service(resource_endpoint::get_resource_metadata)
                    .service(resource_endpoint::get_resource_metadata_description)
            )
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

