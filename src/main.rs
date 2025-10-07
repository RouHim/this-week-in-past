extern crate core;

use std::env;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use env_logger::Builder;
use log::{info, warn, LevelFilter};

mod config;
mod config_endpoint;
mod exif_reader;
mod filesystem_client;
mod geo_location;
mod image_processor;
mod resource_endpoint;
mod resource_processor;
mod resource_reader;
mod resource_store;
mod scheduler;
mod utils;
mod weather_endpoint;
mod weather_processor;
mod web_app_endpoint;

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
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Configure logger
    let mut builder = Builder::from_default_env();
    builder
        .filter(Some("actix_web::middleware::logger"), LevelFilter::Error)
        .init();

    // Print cargo version to console
    info!(
        "👋 Welcome to this-week-in-past version {}",
        env!("CARGO_PKG_VERSION")
    );

    // Print system date and time
    info!(
        "📅 System time: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    // Create a new resource reader based on the provided resources path
    let resource_reader = resource_reader::new(
        env::var("RESOURCE_PATHS")
            .expect("RESOURCE_PATHS is missing")
            .as_str(),
    );

    // Initialize databases
    if env::var("CACHE_DIR").is_ok() {
        warn!("CACHE_DIR environment variable is deprecated, use DATA_FOLDER instead!")
    }
    let data_folder = env::var("DATA_FOLDER")
        .or_else(|_| env::var("CACHE_DIR"))
        .unwrap_or_else(|_| "./data".to_string());
    let resource_store = resource_store::initialize(&data_folder);

    info!("📅 Database time: {}", resource_store.get_database_time());

    // Start scheduler to run at midnight
    let scheduler_handle =
        scheduler::schedule_indexer(resource_reader.clone(), resource_store.clone());

    let bind_address = format!(
        "0.0.0.0:{}",
        env::var("PORT").unwrap_or_else(|_| "8080".to_string())
    );
    // Run the actual web server and hold the main thread here
    info!("🚀 Launching webserver on http://{} 🚀", bind_address);
    let http_server_result = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(resource_store.clone()))
            .app_data(web::Data::new(resource_reader.clone()))
            .wrap(middleware::Logger::default()) // enable logger
            .service(web_app_endpoint::index)
            .service(web_app_endpoint::style_css)
            .service(web_app_endpoint::script_js)
            .service(web_app_endpoint::hide_png)
            .service(web_app_endpoint::icon_png)
            .service(web_app_endpoint::font)
            .service(
                web::scope("/api/resources")
                    .service(resource_endpoint::get_all_resources)
                    .service(resource_endpoint::get_this_week_resources)
                    .service(resource_endpoint::get_this_week_resources_count)
                    .service(resource_endpoint::get_this_week_resources_metadata)
                    .service(resource_endpoint::get_this_week_resource_image)
                    .service(resource_endpoint::random_resources)
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
                    .service(weather_endpoint::get_home_assistant_entity_data)
                    .service(weather_endpoint::get_weather_unit),
            )
            .service(
                web::scope("/api/config")
                    .service(config_endpoint::get_slideshow_interval)
                    .service(config_endpoint::get_refresh_interval)
                    .service(config_endpoint::get_hide_button_enabled)
                    .service(config_endpoint::get_random_slideshow_enabled)
                    .service(config_endpoint::get_preload_images_enabled),
            )
            .service(web::resource("/api/version").route(web::get().to(
                |_: HttpRequest, _: web::Payload| async move {
                    Ok::<_, actix_web::Error>(
                        HttpResponse::Ok()
                            .content_type("plain/text")
                            .body(env!("CARGO_PKG_VERSION")),
                    )
                },
            )))
            .service(web::resource("/api/health").route(web::get().to(HttpResponse::Ok)))
    })
    .bind(bind_address)?
    .run()
    .await;

    // If the http server is terminated...

    // Cleanup database
    info!("Cleanup database 🧹");
    resource_store::initialize(&data_folder).vacuum();

    // Stop the scheduler
    info!("Stopping scheduler 🕐️");
    scheduler_handle.stop();

    // Done, let's get out here
    info!("Stopping Application 😵️");
    http_server_result
}
