use actix_files::Files;
use actix_web::{App, HttpServer, middleware, web};
use evmap::{ReadHandle, WriteHandle};

mod scheduler;
mod web_dav_client;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let (kv_reader, mut kv_writer) = evmap::new::<String, WebDavResource>();

    let scheduler_handle = scheduler::run(&mut kv_writer);
    scheduler::fetch_images(&mut kv_writer);

    let http_server_result = HttpServer::new(move || {
        App::new()
            .app_data(kv_reader.clone())
            .wrap(middleware::Logger::default()) // enable logger
            // .route("/api/photos", web::post().to(api_get_all_photos_handler))
            // .route("/api/photos/{photo-name}", web::post().to(api_get_photo_handler))
            // .route("/api/photos/{photo-name}/metadata", web::post().to(api_get_photo_metadata_handler))
            .service(Files::new("/", "./static/").index_file("index.html"))
    })
        .bind("0.0.0.0:8080")?
        .run()
        .await;

    scheduler_handle.stop();

    return http_server_result;
}

