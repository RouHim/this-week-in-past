extern crate core;

use actix_files::Files;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use std::env;


mod config_endpoint;
mod exif_reader;
mod filesystem_client;
mod geo_location;
mod image_processor;
mod resource_endpoint;
mod resource_processor;
mod resource_reader;
mod resource_store;
mod samba_client;
mod scheduler;
mod utils;
mod weather_endpoint;
mod weather_processor;

#[cfg(test)]
mod integration_test_config_api;
#[cfg(test)]
mod integration_test_resources_api;
#[cfg(test)]
mod integration_test_weather_api;
#[cfg(test)]
mod resource_processor_test;
#[cfg(test)]
mod resource_reader_test;

#[derive(Clone)]
pub struct ResourceReader {
    /// Holds all specified local paths
    pub local_resource_paths: Vec<String>,

    /// Holds all samba paths
    pub samba_resource_paths: Vec<String>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Build application state based on the provided parameter
    let app_config = resource_reader::new(
        env::var("RESOURCE_PATHS")
            .expect("RESOURCE_PATHS is missing")
            .as_str(),
    );

    // Initialize databases
    let resource_store = resource_store::initialize();

    // Start scheduler to run at midnight
    let scheduler_handle = scheduler::schedule_indexer(app_config.clone(), resource_store.clone());

    // Fetch resources for the first time
    scheduler::fetch_resources(app_config.clone(), resource_store.clone());

    // Run the actual web server and hold the main thread here
    println!("Launching webserver 🚀");
    let http_server_result = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(resource_store.clone()))
            .app_data(web::Data::new(app_config.clone()))
            .wrap(middleware::Logger::default()) // enable logger
            .service(
                web::scope("/api/resources")
                    .service(resource_endpoint::get_all_resources)
                    .service(resource_endpoint::get_this_week_resources)
                    .service(resource_endpoint::random_resource)
                    .service(resource_endpoint::get_resource_by_id_and_resolution)
                    .service(resource_endpoint::get_resource_metadata_by_id)
                    .service(resource_endpoint::get_resource_metadata_description_by_id)
                    .service(resource_endpoint::get_all_hidden_resources)
                    .service(resource_endpoint::set_resource_hidden)
                    .service(resource_endpoint::delete_resource_hidden),
            )
            .service(
                web::scope("/api/weather")
                    .service(weather_endpoint::get_is_weather_enabled)
                    .service(weather_endpoint::get_current_weather)
                    .service(weather_endpoint::get_is_home_assistant_enabled)
                    .service(weather_endpoint::get_home_assistant_entity_data),
            )
            .service(
                web::scope("/api/config")
                    .service(config_endpoint::get_slideshow_interval)
                    .service(config_endpoint::get_refresh_interval)
                    .service(config_endpoint::get_hide_button_enabled),
            )
            .service(web::resource("/api/health").route(web::get().to(HttpResponse::Ok)))
            .service(Files::new("/", "./web-app/").index_file("index.html"))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await;

    // If the http server is terminated

    // Stop the scheduler
    println!("Stopping scheduler 🕐️");
    scheduler_handle.stop();

    // Done, let's get out here
    println!("Stopping Application 😵️");
    http_server_result
}
