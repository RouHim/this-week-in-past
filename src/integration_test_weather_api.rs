use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{env, fs};

use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::{test, web, App, Error};
use assertor::{assert_that, StringAssertion};
use evmap::{ReadHandle, WriteHandle};
use rand::Rng;

use crate::resource_reader::ResourceReader;
use crate::{resource_reader, scheduler, weather_endpoint};

const TEST_FOLDER_NAME: &str = "integration_test_weather_api";

#[actix_web::test]
async fn test_get_weather_current() {
    // GIVEN is a running this-week-in-past instance
    let base_test_dir = create_temp_folder().await;
    let (kv_reader, kv_writer) = evmap::new::<String, String>();
    let kv_writer_mutex = Arc::new(Mutex::new(kv_writer));
    let app_server = test::init_service(build_app(
        kv_reader,
        resource_reader::new(base_test_dir.to_str().unwrap()),
        kv_writer_mutex.clone(),
    ))
    .await;

    // WHEN requesting current weather
    let response: String = String::from_utf8(
        test::call_and_read_body(
            &app_server,
            test::TestRequest::get()
                .uri("/api/weather/current")
                .to_request(),
        )
        .await
        .to_vec(),
    )
    .unwrap();

    // THEN the response should contain weather data
    assert_that!(response).contains("weather");
}

#[actix_web::test]
async fn test_get_is_weather_enabled() {
    // GIVEN is a running this-week-in-past instance
    let base_test_dir = create_temp_folder().await;
    let (kv_reader, kv_writer) = evmap::new::<String, String>();
    let kv_writer_mutex = Arc::new(Mutex::new(kv_writer));
    let app_server = test::init_service(build_app(
        kv_reader,
        resource_reader::new(base_test_dir.to_str().unwrap()),
        kv_writer_mutex.clone(),
    ))
    .await;

    // WHEN requesting if weather is enabled
    let response: String = String::from_utf8(
        test::call_and_read_body(
            &app_server,
            test::TestRequest::get().uri("/api/weather").to_request(),
        )
        .await
        .to_vec(),
    )
    .unwrap();

    // THEN the response should return if weather is enabled
    assert_that!(response).contains("true");
}

fn build_app(
    kv_reader: ReadHandle<String, String>,
    resource_reader: ResourceReader,
    kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse,
        Error = Error,
        InitError = (),
    >,
> {
    scheduler::init();
    scheduler::fetch_resources(resource_reader.clone(), kv_writer_mutex);
    App::new()
        .app_data(web::Data::new(kv_reader))
        .app_data(web::Data::new(resource_reader))
        .service(
            web::scope("/api/weather")
                .service(weather_endpoint::get_is_weather_enabled)
                .service(weather_endpoint::get_current_weather),
        )
}

/// Creates a temp folder with the given name and returns its full path
async fn create_temp_folder() -> PathBuf {
    let random_string = rand::thread_rng().gen::<u32>().to_string();
    let test_dir: PathBuf = env::temp_dir().join(TEST_FOLDER_NAME).join(&random_string);

    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).expect("Failed to remove test dir");
    }

    fs::create_dir_all(&test_dir).unwrap();

    let cache_dir = format!("/tmp/cache/{}/{}", &random_string, TEST_FOLDER_NAME);
    env::set_var("CACHE_DIR", &cache_dir);
    fs::create_dir_all(&cache_dir).unwrap();

    test_dir
}
