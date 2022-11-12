use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use std::ops::Add;
use std::{env, fs};

use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::{test, web, App, Error};
use assertor::{assert_that, EqualityAssertion, VecAssertion};
use chrono::{Duration, Local, NaiveDateTime};
use rand::Rng;
use test::TestRequest;

use crate::geo_location::GeoLocation;
use crate::resource_reader::RemoteResource;
use crate::{resource_endpoint, resource_reader, resource_store, scheduler, utils};

const TEST_JPEG_EXIF_URL: &str =
    "https://raw.githubusercontent.com/ianare/exif-samples/master/jpg/gps/DSCN0010.jpg";
const TEST_JPEG_URL: &str = "https://www.w3.org/People/mimasa/test/imgformat/img/w3c_home.jpg";
const TEST_FOLDER_NAME: &str = "integration_test_rest_api";

#[actix_web::test]
async fn test_get_all_resources() {
    // GIVEN is a folder structure with two assets and another file type
    let base_test_dir = create_temp_folder().await;
    let test_image_1 = create_test_image(
        &base_test_dir,
        "sub1",
        "test_image_1.jpg",
        TEST_JPEG_EXIF_URL,
    )
    .await;
    let test_image_2 = create_test_image(
        &base_test_dir,
        "sub2",
        "test_image_2.jpg",
        TEST_JPEG_EXIF_URL,
    )
    .await;

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting all resources
    let response: Vec<String> = test::call_and_read_body_json(
        &app_server,
        TestRequest::get().uri("/api/resources").to_request(),
    )
    .await;

    // THEN the response should contain the two resources
    assert_that!(response).contains_exactly(vec![
        utils::md5(test_image_1.as_str()),
        utils::md5(test_image_2.as_str()),
    ]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_this_week_in_past_resources() {
    // GIVEN is one image assets
    let base_test_dir = create_temp_folder().await;
    let today_date_string = Local::now().date().format("%Y%m%d").to_string();
    let test_image_1 = create_test_image(
        &base_test_dir,
        "",
        format!("IMG_{}.jpg", today_date_string).as_str(),
        TEST_JPEG_URL,
    )
    .await;
    let another_date_string = Local::now()
        .date()
        .add(Duration::weeks(4))
        .format("%Y%m%d")
        .to_string();
    let _ = create_test_image(
        &base_test_dir,
        "",
        format!("IMG_{}.jpg", another_date_string).as_str(),
        TEST_JPEG_URL,
    )
    .await;

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting of this week in past resources
    let response: Vec<String> = test::call_and_read_body_json(
        &app_server,
        TestRequest::get().uri("/api/resources/week").to_request(),
    )
    .await;

    // THEN the response should contain the resource
    assert_that!(response).contains_exactly(vec![utils::md5(test_image_1.as_str())]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_random_resources() {
    // GIVEN is one exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting a random resource
    let response: String = test::call_and_read_body_json(
        &app_server,
        TestRequest::get().uri("/api/resources/random").to_request(),
    )
    .await;

    // THEN the response should contain the random resources
    assert_that!(response).is_equal_to(utils::md5(test_image_1.as_str()));

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_resource_by_id_and_resolution() {
    // GIVEN is an exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;
    let test_image_1_id = utils::md5(test_image_1.as_str());

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting a random resource
    let response = test::call_and_read_body(
        &app_server,
        TestRequest::get()
            .uri(format!("/api/resources/{test_image_1_id}/10/10").as_str())
            .to_request(),
    )
    .await;

    // THEN the response should contain the resized image
    assert_that!(response.len()).is_equal_to(283);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_resource_metadata_by_id() {
    // GIVEN is an exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;
    let test_image_1_id = utils::md5(test_image_1.as_str());
    let test_image_1_path = format!("{}/{}", base_test_dir.to_str().unwrap(), test_image_1);

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting a random resource
    let response: RemoteResource = test::call_and_read_body_json(
        &app_server,
        TestRequest::get()
            .uri(format!("/api/resources/{test_image_1_id}/metadata").as_str())
            .to_request(),
    )
    .await;

    // THEN the response should contain the resized image
    assert_that!(response.id).is_equal_to(test_image_1_id);
    assert_that!(response.path).is_equal_to(&test_image_1_path);
    assert_that!(response.content_type).is_equal_to("image/jpeg".to_string());
    assert_that!(response.name).is_equal_to("test_image_1.jpg".to_string());
    assert_that!(response.content_length).is_equal_to(
        File::open(&test_image_1_path)
            .unwrap()
            .metadata()
            .unwrap()
            .len(),
    );
    assert_that!(response.taken).is_equal_to(Some(
        NaiveDateTime::parse_from_str("2008-11-01T21:15:07", "%Y-%m-%dT%H:%M:%S").unwrap(),
    ));
    assert_that!(response.location).is_equal_to(Some(GeoLocation {
        latitude: 43.46745,
        longitude: 11.885126,
    }));

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_resource_description_by_id() {
    // GIVEN is an exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;
    let test_image_1_id = utils::md5(test_image_1.as_str());

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting a description resource
    let response = String::from_utf8(
        test::call_and_read_body(
            &app_server,
            TestRequest::get()
                .uri(format!("/api/resources/{test_image_1_id}/description").as_str())
                .to_request(),
        )
        .await
        .to_vec(),
    )
    .unwrap();

    // THEN the response should contain the resized image
    assert_that!(response).is_equal_to("01.11.2008, Arezzo".to_string());

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn get_hidden_resources() {
    // GIVEN is a folder structure with two assets and another file type
    let base_test_dir = create_temp_folder().await;
    let test_image_1_id = utils::md5(
        create_test_image(
            &base_test_dir,
            "sub1",
            "test_image_1.jpg",
            TEST_JPEG_EXIF_URL,
        )
        .await
        .as_str(),
    );

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // AND this image is hidden
    let _ = test::call_and_read_body(
        &app_server,
        TestRequest::post()
            .uri(format!("/api/resources/hide/{test_image_1_id}").as_str())
            .to_request(),
    )
    .await;

    // WHEN receiving all hidden resources
    let response: Vec<String> = test::call_and_read_body_json(
        &app_server,
        TestRequest::get().uri("/api/resources/hide").to_request(),
    )
    .await;

    // THEN then one image should be hidden
    assert_that!(response).contains_exactly(vec![test_image_1_id]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn get_hidden_resources_when_set_visible_again() {
    // GIVEN is a folder structure with two assets and another file type
    let base_test_dir = create_temp_folder().await;
    let test_image_1_id = utils::md5(
        create_test_image(
            &base_test_dir,
            "sub1",
            "test_image_1.jpg",
            TEST_JPEG_EXIF_URL,
        )
        .await
        .as_str(),
    );

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // AND this image is hidden
    let _ = test::call_and_read_body(
        &app_server,
        TestRequest::post()
            .uri(format!("/api/resources/hide/{test_image_1_id}").as_str())
            .to_request(),
    )
    .await;

    // AND this image is set to visible again
    let _ = test::call_and_read_body(
        &app_server,
        TestRequest::delete()
            .uri(format!("/api/resources/hide/{test_image_1_id}").as_str())
            .to_request(),
    )
    .await;

    // WHEN receiving all hidden resources
    let response: Vec<String> = test::call_and_read_body_json(
        &app_server,
        TestRequest::get().uri("/api/resources/hide").to_request(),
    )
    .await;

    // THEN then no image should be hidden
    assert_that!(response).contains_exactly(vec![]);

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
    scheduler::index_resources(resource_reader.clone(), resource_store.clone());
    App::new()
        .app_data(web::Data::new(resource_store))
        .app_data(web::Data::new(resource_reader))
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
}

/// Creates a test image withing a folder
async fn create_test_image(
    base_dir: &Path,
    sub_dir: &str,
    file_name: &str,
    image_url: &str,
) -> String {
    let target_dir = base_dir.join(sub_dir);

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).unwrap();
    }

    let test_image_path = target_dir.join(file_name);

    let response = ureq::get(image_url).call().unwrap();

    let len: usize = response.header("Content-Length").unwrap().parse().unwrap();

    let mut data: Vec<u8> = Vec::with_capacity(len);
    response
        .into_reader()
        .read_to_end(&mut data)
        .expect("write fail");

    fs::write(&test_image_path, data).unwrap_or_else(|_| {
        panic!(
            "error while writing test image {}",
            test_image_path.to_str().unwrap()
        )
    });

    file_name.to_string()
}

/// Removes the test folder after test run
async fn cleanup(test_dir: &PathBuf) {
    let _ = fs::remove_dir_all(test_dir);
}

/// Creates a temp folder with the given name and returns its full path
async fn create_temp_folder() -> PathBuf {
    let random_string = rand::thread_rng().gen::<u32>().to_string();
    let test_dir: PathBuf = env::temp_dir().join(TEST_FOLDER_NAME).join(&random_string);

    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).expect("Failed to remove test dir");
    }

    fs::create_dir_all(&test_dir).unwrap();

    // add data folder to test dir
    let data_dir = test_dir.join("data");
    env::set_var("DATA_FOLDER", data_dir.as_path().to_str().unwrap());
    fs::create_dir_all(&data_dir).unwrap();

    test_dir
}
