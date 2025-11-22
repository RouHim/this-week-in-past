use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use std::ops::{Add, Sub};
use std::{env, fs};

use assertor::{assert_that, EqualityAssertion, VecAssertion};
use chrono::{Duration, Local, NaiveDateTime};
use rand::Rng;
use serde::de::DeserializeOwned;
use warp::filters::BoxedFilter;
use warp::reply::Response;
use warp::test::request;

use crate::geo_location::GeoLocation;
use crate::resource_reader::ImageResource;
use crate::{resource_reader, resource_store, routes, scheduler, utils};

const TEST_JPEG_EXIF_URL: &str =
    "https://raw.githubusercontent.com/ianare/exif-samples/master/jpg/gps/DSCN0010.jpg";
const TEST_JPEG_URL: &str = "https://www.w3.org/People/mimasa/test/imgformat/img/w3c_home.jpg";
const TEST_FOLDER_NAME: &str = "integration_test_rest_api";

#[tokio::test]
async fn test_get_all_resources() {
    // GIVEN is a folder structure with two assets
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
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting all resources
    let response: Vec<String> = get_json(&app_server, "/api/resources").await;

    // THEN the response should contain the two resources
    assert_that!(response).contains_exactly(vec![
        utils::md5(test_image_1.as_str()),
        utils::md5(test_image_2.as_str()),
    ]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_this_week_in_past_resources_end_range() {
    // GIVEN is one in week range
    let base_test_dir = create_temp_folder().await;
    let upper_bound = Local::now().add(Duration::days(3));
    let today_date_string = upper_bound.date_naive().format("%Y%m%d").to_string();
    let test_image_1 = create_test_image(
        &base_test_dir,
        "",
        format!("IMG_{}.jpg", today_date_string).as_str(),
        TEST_JPEG_URL,
    )
    .await;
    let another_date_string = Local::now()
        .date_naive()
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
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting of this week in past resources
    let response: Vec<String> = get_json(&app_server, "/api/resources/week").await;

    // THEN the response should contain the resource
    assert_that!(response).contains_exactly(vec![utils::md5(test_image_1.as_str())]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_this_week_in_past_resources_begin_range() {
    // GIVEN is one image in week rnage
    let base_test_dir = create_temp_folder().await;
    let lower_bound = Local::now().sub(Duration::days(3));
    let today_date_string = lower_bound.date_naive().format("%Y%m%d").to_string();
    let test_image_1 = create_test_image(
        &base_test_dir,
        "",
        format!("IMG_{}.jpg", today_date_string).as_str(),
        TEST_JPEG_URL,
    )
    .await;
    let another_date_string = Local::now()
        .date_naive()
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
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting of this week in past resources
    let response: Vec<String> = get_json(&app_server, "/api/resources/week").await;

    // THEN the response should contain the resource
    assert_that!(response).contains_exactly(vec![utils::md5(test_image_1.as_str())]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_this_week_in_past_resources_out_of_end_range() {
    // GIVEN is one image that is out of range
    let base_test_dir = create_temp_folder().await;
    let upper_bound = Local::now().add(Duration::days(4));
    let today_date_string = upper_bound.date_naive().format("%Y%m%d").to_string();
    let _test_image_1 = create_test_image(
        &base_test_dir,
        "",
        format!("IMG_{}.jpg", today_date_string).as_str(),
        TEST_JPEG_URL,
    )
    .await;
    let another_date_string = Local::now()
        .date_naive()
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
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting of this week in past resources
    let response: Vec<String> = get_json(&app_server, "/api/resources/week").await;

    // THEN the response should not contain the resource
    assert_that!(response).is_empty();

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_this_week_in_past_resources_out_of_begin_range() {
    // GIVEN is a image that is out of range
    let base_test_dir = create_temp_folder().await;
    let lower_bound = Local::now().sub(Duration::days(4));
    let today_date_string = lower_bound.date_naive().format("%Y%m%d").to_string();
    let _test_image_1 = create_test_image(
        &base_test_dir,
        "",
        format!("IMG_{}.jpg", today_date_string).as_str(),
        TEST_JPEG_URL,
    )
    .await;
    let another_date_string = Local::now()
        .date_naive()
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
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting of this week in past resources
    let response: Vec<String> = get_json(&app_server, "/api/resources/week").await;

    // THEN the response should not contain the resource
    assert_that!(response).is_empty();

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_get_random_resources() {
    // GIVEN is one exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;

    // AND a running this-week-in-past instance
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting a random resource
    let response: Vec<String> = get_json(&app_server, "/api/resources/random").await;

    // THEN the response should contain the random resources
    assert_that!(response).contains_exactly(vec![utils::md5(test_image_1.as_str())]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_get_resources_week_count() {
    // GIVEN is a folder structure with two assets in the week range, and one out of range
    let base_test_dir = create_temp_folder().await;
    let upper_bound = Local::now().add(Duration::days(3));
    let today_date_string = upper_bound.date_naive().format("%Y%m%d").to_string();
    let _test_image_1 = create_test_image(
        &base_test_dir,
        "",
        format!("IMG_{}.jpg", today_date_string).as_str(),
        TEST_JPEG_URL,
    )
    .await;
    let lower_bound = Local::now().sub(Duration::days(3));
    let another_date_string = lower_bound.date_naive().format("%Y%m%d").to_string();
    let _test_image_2 = create_test_image(
        &base_test_dir,
        "",
        format!("IMG_{}.jpg", another_date_string).as_str(),
        TEST_JPEG_URL,
    )
    .await;
    let out_of_range_date_string = Local::now()
        .sub(Duration::days(4))
        .date_naive()
        .format("%Y%m%d")
        .to_string();
    let _ = create_test_image(
        &base_test_dir,
        "",
        format!("IMG_{}.jpg", out_of_range_date_string).as_str(),
        TEST_JPEG_URL,
    )
    .await;

    // AND a running this-week-in-past instance
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting the count of this week resources (text/plain)
    let response = get_text(&app_server, "/api/resources/week/count").await;
    let response = response.parse::<usize>().unwrap();

    // THEN the response should contain the count of the resources
    assert_that!(response).is_equal_to(2);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_get_resource_by_id_and_resolution() {
    // GIVEN is an exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;
    let test_image_1_id = utils::md5(test_image_1.as_str());

    // AND a running this-week-in-past instance
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting a random resource
    let response = request()
        .method("GET")
        .path(format!("/api/resources/{test_image_1_id}/10/10").as_str())
        .reply(&app_server)
        .await;

    // THEN the response should contain the resized image
    assert_that!(response.body().len()).is_equal_to(316);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_get_resource_metadata_by_id() {
    // GIVEN is an exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;
    let test_image_1_id = utils::md5(test_image_1.as_str());
    let test_image_1_path = format!("{}/{}", base_test_dir.to_str().unwrap(), test_image_1);

    // AND a running this-week-in-past instance
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting a random resource
    let response: ImageResource = get_json(
        &app_server,
        format!("/api/resources/{test_image_1_id}/metadata").as_str(),
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
        NaiveDateTime::parse_from_str("2008-10-22T16:28:39", "%Y-%m-%dT%H:%M:%S").unwrap(),
    ));
    assert_that!(response.location).is_equal_to(Some(GeoLocation {
        latitude: 43.46745,
        longitude: 11.885126,
    }));

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_get_resource_description_by_id() {
    // GIVEN is an exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;
    let test_image_1_id = utils::md5(test_image_1.as_str());

    // AND a running this-week-in-past instance
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting a description resource
    let response = get_text(
        &app_server,
        format!("/api/resources/{test_image_1_id}/description").as_str(),
    )
    .await;

    // THEN the response should contain the resized image
    if env::var("BIGDATA_CLOUD_API_KEY").is_ok() {
        assert_that!(response).is_equal_to("22.10.2008, Arezzo".to_string());
    } else {
        assert_that!(response).is_equal_to("22.10.2008".to_string());
    }

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn test_ignore_file_in_resources() {
    // GIVEN is a folder structure with two assets
    // AND a file with the name .ignore
    let base_test_dir = create_temp_folder().await;
    create_test_image(
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
    create_test_image(&base_test_dir, "sub1", ".ignore", TEST_JPEG_URL).await;

    // AND a running this-week-in-past instance
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // WHEN requesting all resources
    let response: Vec<String> = get_json(&app_server, "/api/resources").await;

    // THEN the response should contain only the second resource
    assert_that!(response).contains_exactly(vec![utils::md5(test_image_2.as_str())]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn get_hidden_resources() {
    // GIVEN is a folder structure with one assets
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
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // AND this image is hidden
    let _ = request()
        .method("POST")
        .path(format!("/api/resources/hide/{test_image_1_id}").as_str())
        .reply(&app_server)
        .await;

    // WHEN receiving all hidden resources
    let response: Vec<String> = get_json(&app_server, "/api/resources/hide").await;

    // THEN then one image should be hidden
    assert_that!(response).contains_exactly(vec![test_image_1_id]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[tokio::test]
async fn get_hidden_resources_when_set_visible_again() {
    // GIVEN is a folder structure with one assets and another file type
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
    let app_server = build_app(base_test_dir.to_str().unwrap());

    // AND this image is hidden
    let _ = request()
        .method("POST")
        .path(format!("/api/resources/hide/{test_image_1_id}").as_str())
        .reply(&app_server)
        .await;

    // AND this image is set to visible again
    let _ = request()
        .method("DELETE")
        .path(format!("/api/resources/hide/{test_image_1_id}").as_str())
        .reply(&app_server)
        .await;

    // WHEN receiving all hidden resources
    let response: Vec<String> = get_json(&app_server, "/api/resources/hide").await;

    // THEN then no image should be hidden
    assert_that!(response).contains_exactly(vec![]);

    // cleanup
    cleanup(&base_test_dir).await;
}

fn build_app(base_test_dir: &str) -> BoxedFilter<(Response,)> {
    let resource_reader = resource_reader::new(base_test_dir);
    let resource_store = resource_store::initialize(base_test_dir);
    scheduler::index_resources(resource_reader.clone(), resource_store.clone());
    routes::build_routes(resource_store, resource_reader)
}

async fn get_json<T: DeserializeOwned>(app: &BoxedFilter<(Response,)>, path: &str) -> T {
    let response = request().method("GET").path(path).reply(app).await;
    serde_json::from_slice(response.body()).unwrap()
}

async fn get_text(app: &BoxedFilter<(Response,)>, path: &str) -> String {
    let response = request().method("GET").path(path).reply(app).await;
    String::from_utf8(response.body().to_vec()).unwrap()
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

    let mut response = ureq::get(image_url).call().unwrap();

    let content_length = response.headers().get("Content-Length").unwrap();
    let len: usize = content_length.to_str().unwrap().parse().unwrap();

    let mut data: Vec<u8> = Vec::with_capacity(len);
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut data)
        .unwrap();

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
    let random_string = rand::rng().random::<u32>().to_string();
    let test_dir: PathBuf = env::temp_dir().join(TEST_FOLDER_NAME).join(random_string);

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
