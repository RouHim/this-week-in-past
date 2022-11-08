use std::path::PathBuf;

use std::{env, fs};

use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::{test, web, App, Error};
use assertor::{assert_that, EqualityAssertion};
use rand::Rng;

use crate::{config_endpoint, resource_reader, resource_store, scheduler};

const TEST_FOLDER_NAME: &str = "integration_test_config_api";

#[actix_web::test]
async fn test_get_slideshow_interval() {
    // GIVEN is a running this-week-in-past instance
    let base_test_dir = create_temp_folder().await;
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // AND slideshow interval is set
    let slideshow_interval: String = rand::thread_rng().gen::<u16>().to_string();
    env::set_var("SLIDESHOW_INTERVAL", &slideshow_interval);

    // WHEN requesting slideshow interval
    let response: String = String::from_utf8(
        test::call_and_read_body(
            &app_server,
            test::TestRequest::get()
                .uri("/api/config/interval/slideshow")
                .to_request(),
        )
        .await
        .to_vec(),
    )
    .unwrap();

    // THEN the response should contain the correct interval
    assert_that!(response).is_equal_to(&slideshow_interval);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_refresh_interval() {
    // GIVEN is a running this-week-in-past instance
    let base_test_dir = create_temp_folder().await;
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // AND refresh interval is set
    let refresh_interval: String = rand::thread_rng().gen::<u16>().to_string();
    env::set_var("REFRESH_INTERVAL", &refresh_interval);

    // WHEN requesting refresh interval
    let response: String = String::from_utf8(
        test::call_and_read_body(
            &app_server,
            test::TestRequest::get()
                .uri("/api/config/interval/refresh")
                .to_request(),
        )
        .await
        .to_vec(),
    )
    .unwrap();

    // THEN the response should contain the correct interval
    assert_that!(response).is_equal_to(&refresh_interval);

    // cleanup
    cleanup(&base_test_dir).await;
}

fn build_app(
    base_test_dir: &str,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse,
        Error = Error,
        InitError = (),
    >,
> {
    let resource_reader = resource_reader::new(base_test_dir);
    let resource_store = resource_store::initialize(base_test_dir.to_string());
    scheduler::index_resources(resource_reader, resource_store.clone());
    App::new().app_data(web::Data::new(resource_store)).service(
        web::scope("/api/config")
            .service(config_endpoint::get_slideshow_interval)
            .service(config_endpoint::get_refresh_interval),
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

/// Removes the test folder after test run
async fn cleanup(test_dir: &PathBuf) {
    let _ = fs::remove_dir_all(test_dir);
}
