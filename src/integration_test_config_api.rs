use std::path::PathBuf;
use std::{env, fs};

use assertor::{assert_that, EqualityAssertion};
use rand::Rng;
use warp::filters::BoxedFilter;
use warp::reply::Response;
use warp::test::request;

use crate::{resource_reader, resource_store, routes, scheduler};

const TEST_FOLDER_NAME: &str = "integration_test_config_api";

#[tokio::test]
async fn test_get_random_slideshow() {
    // GIVEN is a running this-week-in-past instance
    let base_test_dir = create_temp_folder().await;
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // AND random slideshow is set
    let random_slideshow: String = rand::rng().random::<bool>().to_string();
    env::set_var("RANDOM_SLIDESHOW", &random_slideshow);

    // WHEN requesting random slideshow
    let response = request()
        .method("GET")
        .path("/api/config/random-slideshow")
        .reply(&app_server)
        .await;
    let response = String::from_utf8(response.body().to_vec()).unwrap();

    // THEN the response should contain the correct interval
    assert_that!(response).is_equal_to(&random_slideshow);

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
