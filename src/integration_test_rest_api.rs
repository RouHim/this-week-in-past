use std::{env, fs};
use std::path::PathBuf;

use rand::Rng;

const TEST_JPEG_EXIF_URL: &str = "https://raw.githubusercontent.com/ianare/exif-samples/master/jpg/gps/DSCN0010.jpg";
const TEST_FOLDER_NAME: &str = "integration_test_rest_api";

#[cfg(test)]
mod integration_tests {
    use std::sync::{Arc, Mutex};

    use actix_web::{App, Error, test, web};
    use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
    use assertor::{assert_that, EqualityAssertion, VecAssertion};
    use evmap::{ReadHandle, WriteHandle};

    use crate::{resource_endpoint, resource_processor, resource_reader, scheduler};
    use crate::resource_reader::ResourceReader;

    use super::*;

    #[actix_web::test]
    async fn test_get_all_resources() {
        // GIVEN is a folder structure with two images and another file type
        let base_test_dir = create_temp_folder().await;
        let test_image_1 = create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;
        let test_image_2 = create_test_image(&base_test_dir, "", "test_image_2.jpg", TEST_JPEG_EXIF_URL).await;

        // AND a running this-week-in-past instance
        let (kv_reader, kv_writer) = evmap::new::<String, String>();
        let kv_writer_mutex = Arc::new(Mutex::new(kv_writer));
        let app_server = test::init_service(build_app(
            kv_reader,
            resource_reader::new(base_test_dir.to_str().unwrap()),
            kv_writer_mutex.clone(),
        )).await;

        // WHEN requesting all resources
        let response: Vec<String> = test::call_and_read_body_json(
            &app_server,
            test::TestRequest::get().uri("/api/resources").to_request(),
        ).await;

        // THEN the response should contain the two resources
        assert_that!(response).contains_exactly(vec![
            resource_processor::md5(test_image_1.as_str()),
            resource_processor::md5(test_image_2.as_str()),
        ]);

        // cleanup
        cleanup(&base_test_dir).await;
    }

    #[actix_web::test]
    async fn test_get_random_resources() {
        // GIVEN is a folder structure with two images and another file type
        let base_test_dir = create_temp_folder().await;
        let test_image_1 = create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_EXIF_URL).await;

        // AND a running this-week-in-past instance
        let (kv_reader, kv_writer) = evmap::new::<String, String>();
        let kv_writer_mutex = Arc::new(Mutex::new(kv_writer));
        let app_server = test::init_service(build_app(
            kv_reader,
            resource_reader::new(base_test_dir.to_str().unwrap()),
            kv_writer_mutex.clone(),
        )).await;


        // WHEN requesting a random resource
        let response: String = test::call_and_read_body_json(
            &app_server,
            test::TestRequest::get().uri("/api/resources/random").to_request(),
        ).await;

        // THEN the response should contain the random resources
        assert_that!(response).is_equal_to(resource_processor::md5(test_image_1.as_str()));

        // cleanup
        cleanup(&base_test_dir).await;
    }

    fn build_app(kv_reader: ReadHandle<String, String>, resource_reader: ResourceReader, kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>)
                 -> App<impl ServiceFactory<ServiceRequest, Config=(), Response=ServiceResponse, Error=Error, InitError=(), >, > {
        scheduler::init();
        scheduler::fetch_resources(
            resource_reader.clone(),
            kv_writer_mutex,
        );
        App::new()
            .app_data(web::Data::new(kv_reader))
            .app_data(resource_reader)
            .service(web::scope("/api/resources")
                .service(resource_endpoint::list_all_resources)
                .service(resource_endpoint::list_this_week_resources)
                .service(resource_endpoint::random_resource)
                .service(resource_endpoint::get_resource_by_id_and_resolution)
                .service(resource_endpoint::get_resource_base64_by_id_and_resolution)
                .service(resource_endpoint::get_resource_metadata_by_id)
                .service(resource_endpoint::get_resource_metadata_description_by_id)
            )
    }
}

/// Creates a test image withing a folder
async fn create_test_image(base_dir: &PathBuf, sub_dir: &str, file_name: &str, image_url: &str) -> String {
    let target_dir = base_dir.clone().join(sub_dir);

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).unwrap();
    }

    let test_image_path = target_dir.join(file_name);

    let data = reqwest::get(image_url).await.unwrap()
        .bytes().await.unwrap();

    fs::write(&test_image_path, data).unwrap_or_else(|_| panic!("error while writing test image {}", test_image_path.to_str().unwrap()));

    file_name.to_string()
}

/// Removes the test folder after test run
async fn cleanup(test_dir: &PathBuf) {
    let _ = fs::remove_dir_all(&test_dir);
}

/// Creates a temp folder with the given name and returns its full path
async fn create_temp_folder() -> PathBuf {
    let random_string = rand::thread_rng().gen::<u32>().to_string();
    let test_dir: PathBuf = env::temp_dir().join(TEST_FOLDER_NAME).join(random_string);

    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).expect("Failed to remove test dir");
    }

    fs::create_dir_all(&test_dir).unwrap();

    test_dir
}