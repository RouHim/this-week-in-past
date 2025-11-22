extern crate core;

use std::env;
use std::net::SocketAddr;

use env_logger::Builder;
use log::{info, warn, LevelFilter};
use warp::Filter;

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
mod routes;
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

// Avoid musl's default allocator due to lackluster performance
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Clone)]
pub struct ResourceReader {
    /// Holds all specified local paths
    pub local_resource_paths: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure logger
    let mut builder = Builder::from_default_env();
    builder
        .filter(Some("this_week_in_past"), LevelFilter::Error)
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
    let addr: SocketAddr = bind_address.parse().expect("invalid bind address");
    let routes = routes::build_routes(resource_store.clone(), resource_reader.clone())
        .with(warp::log("this_week_in_past"));

    let (_bound_addr, server) = warp::serve(routes).bind_with_graceful_shutdown(addr, async {
        let _ = tokio::signal::ctrl_c().await;
    });
    server.await;

    // If the http server is terminated...

    // Cleanup database
    info!("Cleanup database 🧹");
    resource_store::initialize(&data_folder).vacuum();

    // Stop the scheduler
    info!("Stopping scheduler 🕐️");
    scheduler_handle.stop();

    // Done, let's get out here
    info!("Stopping Application 😵️");
    Ok(())
}
