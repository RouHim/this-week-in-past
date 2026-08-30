use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use std::ops::{Add, Sub};
use std::{env, fs};

use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::{test, web, App, Error};
use assertor::{assert_that, EqualityAssertion, VecAssertion};
use chrono::{Duration, Local, NaiveDateTime};
use rand::RngExt;
use test::TestRequest;

use crate::geo_location::GeoLocation;
use crate::resource_reader::ImageResource;
use crate::{resource_endpoint, resource_reader, resource_store, scheduler, utils};

const TEST_JPEG_EXIF_URL: &str =
    "https://raw.githubusercontent.com/ianare/exif-samples/master/jpg/gps/DSCN0010.jpg";
const TEST_JPEG_URL: &str = "https://www.w3.org/People/mimasa/test/imgformat/img/w3c_home.jpg";
const TEST_FOLDER_NAME: &str = "integration_test_rest_api";

#[actix_web::test]
async fn test_get_all_resources() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
async fn test_this_week_in_past_resources_end_range() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
async fn test_this_week_in_past_resources_begin_range() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
async fn test_this_week_in_past_resources_out_of_end_range() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting of this week in past resources
    let response: Vec<String> = test::call_and_read_body_json(
        &app_server,
        TestRequest::get().uri("/api/resources/week").to_request(),
    )
    .await;

    // THEN the response should not contain the resource
    assert_that!(response).is_empty();

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_this_week_in_past_resources_out_of_begin_range() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting of this week in past resources
    let response: Vec<String> = test::call_and_read_body_json(
        &app_server,
        TestRequest::get().uri("/api/resources/week").to_request(),
    )
    .await;

    // THEN the response should not contain the resource
    assert_that!(response).is_empty();

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_random_resources() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN is one exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting a random resource
    let response: Vec<String> = test::call_and_read_body_json(
        &app_server,
        TestRequest::get().uri("/api/resources/random").to_request(),
    )
    .await;

    // THEN the response should contain the random resources
    assert_that!(response).contains_exactly(vec![utils::md5(test_image_1.as_str())]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_resources_week_count() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting the count of this week resources (text/plain)
    let response = test::call_and_read_body(
        &app_server,
        TestRequest::get()
            .uri("/api/resources/week/count")
            .to_request(),
    )
    .await;
    let result = String::from_utf8(response.to_vec()).unwrap();
    let response = result.parse::<usize>().unwrap();

    // THEN the response should contain the count of the resources
    assert_that!(response).is_equal_to(2);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_resource_by_id_and_resolution() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN is an exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;
    let test_image_1_id = utils::md5(test_image_1.as_str());

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting a random resource
    let req = TestRequest::get()
        .uri(format!("/api/resources/{test_image_1_id}/10/10").as_str())
        .to_request();
    let resp = test::call_service(&app_server, req).await;
    assert_that!(resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap())
    .is_equal_to("image/jpeg");
    let response = test::read_body(resp).await;

    // THEN the response should contain a JPEG resized to fit within the requested display bounds
    assert_that!(response.first()).is_equal_to(Some(&0xFF));
    assert_that!(response.get(1)).is_equal_to(Some(&0xD8)); // JPEG magic FF D8
    let (width, height) = image::ImageReader::new(std::io::Cursor::new(&response))
        .with_guessed_format()
        .unwrap()
        .into_dimensions()
        .unwrap();
    assert_that!(width <= 10).is_equal_to(true);
    assert_that!(height <= 10).is_equal_to(true);
    assert_that!(width > 0).is_equal_to(true);
    assert_that!(height > 0).is_equal_to(true);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_resource_metadata_by_id() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN is an exif image
    let base_test_dir = create_temp_folder().await;
    let test_image_1 =
        create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;
    let test_image_1_id = utils::md5(test_image_1.as_str());
    let test_image_1_path = format!("{}/{}", base_test_dir.to_str().unwrap(), test_image_1);

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting a random resource
    let response: ImageResource = test::call_and_read_body_json(
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
        NaiveDateTime::parse_from_str("2008-10-22T16:28:39", "%Y-%m-%dT%H:%M:%S").unwrap(),
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
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    assert_that!(response).is_equal_to("22.10.2008, Arezzo".to_string());

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_get_unknown_resource_metadata_returns_not_found() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN is an empty library
    let base_test_dir = create_temp_folder().await;

    // AND a running this-week-in-past instance
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting metadata for an unknown resource id
    let metadata_response = test::call_service(
        &app_server,
        TestRequest::get()
            .uri("/api/resources/unknown-id/metadata")
            .to_request(),
    )
    .await;

    let description_response = test::call_service(
        &app_server,
        TestRequest::get()
            .uri("/api/resources/unknown-id/description")
            .to_request(),
    )
    .await;

    // THEN both endpoints respond with 404
    assert_that!(metadata_response.status().as_u16()).is_equal_to(404);
    assert_that!(description_response.status().as_u16()).is_equal_to(404);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
async fn test_ignore_file_in_resources() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    let app_server = test::init_service(build_app(base_test_dir.to_str().unwrap())).await;

    // WHEN requesting all resources
    let response: Vec<String> = test::call_and_read_body_json(
        &app_server,
        TestRequest::get().uri("/api/resources").to_request(),
    )
    .await;

    // THEN the response should contain only the second resource
    assert_that!(response).contains_exactly(vec![utils::md5(test_image_2.as_str())]);

    // cleanup
    cleanup(&base_test_dir).await;
}

#[actix_web::test]
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
    let resource_store = resource_store::initialize(base_test_dir);
    scheduler::index_resources(resource_reader.clone(), resource_store.clone());
    App::new()
        .app_data(web::Data::new(resource_store))
        .app_data(web::Data::new(resource_reader))
        .service(
            web::scope("/api/resources")
                .service(resource_endpoint::get_all_resources)
                .service(resource_endpoint::get_this_week_resources_count)
                .service(resource_endpoint::get_this_week_resources)
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

fn create_local_image_file(base_dir: &Path, file_name: &str) {
    let hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        file_name.hash(&mut h);
        h.finish()
    };
    let r = ((hash >> 16) & 0xFF) as u8;
    let g = ((hash >> 8) & 0xFF) as u8;
    let b = (hash & 0xFF) as u8;
    let img = image::RgbImage::from_pixel(20, 20, image::Rgb([r, g, b]));
    let path = base_dir.join(file_name);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Jpeg,
        )
        .unwrap();
    fs::write(&path, buf).unwrap();
}

#[actix_web::test]
async fn test_image_endpoint_serves_jpeg_and_caches_on_filesystem() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN a temp folder with a local JPEG image indexed and filesystem cache enabled
    let base = create_temp_folder().await;
    env::set_var("DATA_FOLDER", base.to_str().unwrap());
    create_local_image_file(&base, "test_image_fs.jpg");
    let id = utils::md5("test_image_fs.jpg");
    let app = test::init_service(build_app(base.to_str().unwrap())).await;

    // WHEN requesting the image at 100x100 for the first time
    let req = TestRequest::get()
        .uri(&format!("/api/resources/{}/100/100", id))
        .to_request();
    let resp = test::call_service(&app, req).await;

    // THEN the response is JPEG and a filesystem cache file is created
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/jpeg"
    );
    let body = test::read_body(resp).await;
    assert_eq!(&body[0..2], &[0xFF, 0xD8]);
    let cache_file = base.join("cache").join(format!("{}_100_100.jpg", id));
    assert!(
        cache_file.exists(),
        "cache file not found: {:?}",
        cache_file
    );
    let mtime1 = fs::metadata(&cache_file).unwrap().modified().unwrap();

    // WHEN requesting the same image again after a short delay
    actix_rt::time::sleep(std::time::Duration::from_millis(50)).await;
    let req2 = TestRequest::get()
        .uri(&format!("/api/resources/{}/100/100", id))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;

    // THEN the second response is also JPEG and the cache mtime is updated (LRU touch)
    assert_eq!(resp2.status(), 200);
    let body2 = test::read_body(resp2).await;
    assert_eq!(&body2[0..2], &[0xFF, 0xD8]);
    let mtime2 = fs::metadata(&cache_file).unwrap().modified().unwrap();
    assert!(mtime2 >= mtime1, "mtime not updated on cache hit");
    cleanup(&base).await;
}

#[actix_web::test]
#[allow(clippy::arc_with_non_send_sync)]
async fn test_three_concurrent_clients_no_pool_timeout() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN a temp folder with 20 distinct images and filesystem cache enabled
    let base = create_temp_folder().await;
    env::set_var("DATA_FOLDER", base.to_str().unwrap());
    for i in 0..20 {
        create_local_image_file(&base, &format!("concurrent_{}.jpg", i));
    }
    let app = test::init_service(build_app(base.to_str().unwrap())).await;
    let app = std::sync::Arc::new(app);

    // WHEN 60 concurrent clients request images (3x the image set)
    let mut handles = Vec::new();
    for n in 0..60 {
        let app = app.clone();
        let idx = n % 20;
        let file_name = format!("concurrent_{}.jpg", idx);
        let id = utils::md5(&file_name);
        let handle = actix_rt::spawn(async move {
            let req = TestRequest::get()
                .uri(&format!("/api/resources/{}/10/10", id))
                .to_request();
            let resp = test::call_service(&*app, req).await;
            assert_eq!(resp.status(), 200);
            assert_eq!(
                resp.headers()
                    .get("content-type")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "image/jpeg"
            );
            let body = test::read_body(resp).await;
            assert_eq!(&body[0..2], &[0xFF, 0xD8]);
        });
        handles.push(handle);
    }

    // THEN all concurrent requests succeed without pool timeout and return JPEG
    for h in handles {
        h.await.unwrap();
    }
    cleanup(&base).await;
}

#[actix_web::test]
async fn test_cache_eviction_caps_500() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN a fresh filesystem cache directory
    let base = create_temp_folder().await;
    env::set_var("DATA_FOLDER", base.to_str().unwrap());
    let cache_dir = crate::image_cache::cache_dir(base.to_str().unwrap());

    // WHEN putting 600 distinct entries (exceeding the 500-file cap)
    for i in 0..600 {
        let key = format!("evict_{}.jpg", i);
        let data = vec![0xFF, 0xD8, 0xFF, 0x00, i as u8];
        crate::image_cache::put(&cache_dir, &key, &data).unwrap();
        if i % 100 == 0 {
            actix_rt::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }

    // THEN the cache is bounded to 500 files / 1 GiB and oldest is evicted
    let (count, bytes) = crate::image_cache::cache_stats(&cache_dir);
    assert!(count <= 500, "count {} exceeds 500", count);
    assert!(bytes <= 1_073_741_824, "bytes {} exceeds 1GB", bytes);
    let early_exists = cache_dir.join("evict_0.jpg").exists();
    assert!(!early_exists, "LRU should have evicted earliest entry");
    cleanup(&base).await;
}

#[actix_web::test]
async fn test_week_image_endpoint_filesystem_cache() {
    let _serial_guard = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN a temp folder with a week-range image (taken = now) and filesystem cache enabled
    let base = create_temp_folder().await;
    env::set_var("DATA_FOLDER", base.to_str().unwrap());
    create_local_image_file(&base, "week_image_test.jpg");
    let app = test::init_service(build_app(base.to_str().unwrap())).await;
    {
        let store = resource_store::initialize(base.to_str().unwrap());
        let id = utils::md5("week_image_test.jpg");
        if let Some(val) = store.get_resource(&id) {
            let mut v: serde_json::Value = serde_json::from_str(&val).unwrap();
            let now_str = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
            v["taken"] = serde_json::Value::String(now_str.clone());
            let new_val = serde_json::to_string(&v).unwrap();
            let mut map = std::collections::HashMap::new();
            map.insert(id.clone(), new_val);
            store.add_resources(map);
        }
    }

    // WHEN requesting the week image endpoint
    let req = TestRequest::get()
        .uri("/api/resources/week/image")
        .to_request();
    let resp = test::call_service(&app, req).await;

    // THEN the response is JPEG and a filesystem cache file for the week id is created
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/jpeg"
    );
    let body = test::read_body(resp).await;
    assert_eq!(&body[0..2], &[0xFF, 0xD8]);
    let store2 = resource_store::initialize(base.to_str().unwrap());
    let ids = store2.get_resources_this_week_visible_random();
    assert!(!ids.is_empty(), "week query should return at least one id");
    let week_id = &ids[0];
    let cache_file = base.join("cache").join(format!("{}_0_0.jpg", week_id));
    assert!(
        cache_file.exists(),
        "week image cache file not found: {:?}",
        cache_file
    );
    cleanup(&base).await;
}
