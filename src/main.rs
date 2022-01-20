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
                    .service(list_this_week_resources)
                    .service(get_resource)
                    .service(get_resource_base64)
                    .service(get_resource_metadata)
                    .service(get_resource_metadata_display)
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
    let keys: Vec<String> = resource_processor::get_all(kv_reader.as_ref());

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

#[get("today")]
async fn list_this_week_resources(kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let keys: Vec<String> = resource_processor::get_this_week_in_past(kv_reader.as_ref());

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

#[get("{resource_id}")]
async fn get_resource(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>, web_dav_client: web::Data<WebDavClient>) -> HttpResponse {
    let resource_data = kv_reader.get_one(resources_id.as_str())
        .map(|value| value.to_string())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok())
        .and_then(|web_dav_resource| web_dav_client.request_resource_data(&web_dav_resource).bytes().ok());

    if let Some(resource_data) = resource_data {
        HttpResponse::Ok()
            .content_type("image/jpeg")
            .body(resource_data.to_vec())
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/base64")]
async fn get_resource_base64(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>, web_dav_client: web::Data<WebDavClient>) -> HttpResponse {
    let base64_image = kv_reader.get_one(resources_id.as_str())
        .map(|value| value.to_string())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok())
        .and_then(|web_dav_resource| web_dav_client.request_resource_data(&web_dav_resource).bytes().ok())
        .map(|resource_data| base64::encode(&resource_data))
        .map(|base64_string| format!("data:image/jpeg;base64,{}", base64_string));

    if let Some(base64_image) = base64_image {
        HttpResponse::Ok()
            .content_type("plain/text")
            .body(base64_image)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/metadata")]
async fn get_resource_metadata(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let metadata = kv_reader.get_one(resources_id.as_str())
        .map(|value| value.to_string());

    if let Some(metadata) = metadata {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(metadata)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/display")]
async fn get_resource_metadata_display(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let display_value = kv_reader.get_one(resources_id.as_str())
        .map(|value| value.to_string())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok())
        .map(resource_processor::build_display_value);

    if let Some(display_value) = display_value {
        HttpResponse::Ok()
            .content_type("plain/text")
            .body(display_value)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

