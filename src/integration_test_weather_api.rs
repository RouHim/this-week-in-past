use std::path::PathBuf;

use std::{env, fs};

use assertor::{assert_that, StringAssertion};
use rand::Rng;
use warp::filters::BoxedFilter;
use warp::reply::Response;
use warp::test::request;

use crate::{resource_reader, resource_store, routes, scheduler};

const TEST_FOLDER_NAME: &str = "integration_test_weather_api";

#[tokio::test]
async fn test_get_weather_current() {
    // GIVEN is a running this-week-in-past instance
    if env::var("OPEN_WEATHER_MAP_API_KEY").is_err() {
        eprintln!("Skipping weather API test: OPEN_WEATHER_MAP_API_KEY not set");
        return;
    }
    let base_test_dir = create_temp_folder().await;
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting current weather
    let response = request()
        .method("GET")
        .path("/api/weather/current")
        .reply(&app_server)
        .await;
    let response = String::from_utf8(response.body().to_vec()).unwrap();

    // THEN the response should contain weather data
    assert_that!(response).contains("weather");

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_get_is_weather_enabled() {
    // GIVEN is a running this-week-in-past instance
    let base_test_dir = create_temp_folder().await;
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // AND the weather is enabled via env var
    env::set_var("WEATHER_ENABLED", "true");

    // WHEN requesting if weather is enabled
    let response = request()
        .method("GET")
        .path("/api/weather")
        .reply(&app_server)
        .await;
    let response = String::from_utf8(response.body().to_vec()).unwrap();

    // THEN the response should return if weather is enabled
    assert_that!(response).contains("true");

    // cleanup
    cleanup(&base_test_dir).await;
}

fn build_app(base_test_dir: &str) -> BoxedFilter<(Response,)> {
    let resource_reader = resource_reader::new(base_test_dir);
    let resource_store = resource_store::initialize(base_test_dir);
    scheduler::index_resources(resource_reader.clone(), resource_store.clone());
    routes::build_routes(resource_store, resource_reader)
}

/// Creates a temp folder with the given name and returns its full path
async fn create_temp_folder() -> PathBuf {
    let random_string = rand::rng().random::<u32>().to_string();
    let test_dir: PathBuf = env::temp_dir().join(TEST_FOLDER_NAME).join(&random_string);

    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).expect("Failed to remove test dir");
    }

    fs::create_dir_all(&test_dir).unwrap();

    let data_dir = format!("/tmp/cache/{}/{}", &random_string, TEST_FOLDER_NAME);
    env::set_var("DATA_FOLDER", &data_dir);
    fs::create_dir_all(&data_dir).unwrap();

    test_dir
}

/// Removes the test folder after test run
async fn cleanup(test_dir: &PathBuf) {
    let _ = fs::remove_dir_all(test_dir);
}
