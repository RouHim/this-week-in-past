use std::{env, fs};
use std::path::PathBuf;

use chrono::NaiveDateTime;
use rand::Rng;

use crate::{exif_reader, resource_processor, resource_reader};
use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;

const TEST_JPEG_URL: &str = "https://file-examples.com/storage/fe69f82402626533c98f608/2017/10/file_example_JPG_100kB.jpg";
const TEST_JPEG_EXIF_URL: &str = "https://raw.githubusercontent.com/ianare/exif-samples/master/jpg/gps/DSCN0010.jpg";
const TEST_PNG_URL: &str = "https://file-examples.com/storage/fe69f82402626533c98f608/2017/10/file_example_PNG_500kB.png";
const TEST_GIF_URL: &str = "https://file-examples.com/storage/fe69f82402626533c98f608/2017/10/file_example_GIF_500kB.gif";
const TEST_TEMP_BASE_DIR: &str = "resource_reader_test";

#[test]
fn read_dir_recursive() {
    // GIVEN is a folder structure with two images and another file type
    let base_test_dir = create_temp_folder();
    create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_URL);
    create_test_image(&base_test_dir, "sub1", "test_image_2.jpg", TEST_JPEG_URL);
    create_test_file(&base_test_dir, "sub2", "test_file.txt");

    // WHEN reading resources from a folder
    let resources_read = resource_reader::read_folder(&base_test_dir);

    // THEN two resources should be found
    assert_eq!(resources_read.len(), 2);

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn read_jpg_image_resource() {
    // GIVEN is a folder with one jpg image
    let base_test_dir = create_temp_folder();
    let test_image_name = "test_image_1.jpg";
    let test_image_1_path = create_test_image(&base_test_dir, "", test_image_name, TEST_JPEG_URL);

    // WHEN reading resources from a folder
    let resources_read = resource_reader::read_folder(&base_test_dir);

    // THEN the resource info should be correct
    assert_eq!(resources_read.len(), 1);
    assert_eq!(resources_read[0].id, resource_processor::md5(test_image_name));
    assert_eq!(resources_read[0].path, test_image_1_path);
    assert_eq!(resources_read[0].content_type, "image/jpeg");
    assert_eq!(resources_read[0].name, test_image_name);

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn read_jpg_with_exif_image_resource() {
    // GIVEN is a folder with one jpg image with exif and gps metadata
    let base_test_dir = create_temp_folder();
    let test_image_name = "test_image_1.jpg";
    create_test_image(&base_test_dir, "", test_image_name, TEST_JPEG_EXIF_URL);

    // WHEN reading resources from a folder
    let resources_read = exif_reader::fill_exif_data(&resource_reader::read_folder(&base_test_dir)[0]);

    // THEN the resource metadata should be correct
    assert_eq!(resources_read.taken, Some(NaiveDateTime::parse_from_str("2008-11-01T21:15:07", "%Y-%m-%dT%H:%M:%S").unwrap()));
    assert_eq!(resources_read.orientation, Some(ImageOrientation { rotation: 0, mirror_vertically: false }));
    assert_eq!(resources_read.location, Some(GeoLocation { latitude: 43.46745, longitude: 11.885126 }));

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn read_png_image_resource() {
    // GIVEN is a folder with one png image
    let base_test_dir = create_temp_folder();
    let test_image_name = "test_image_1.png";
    let test_image_1_path = create_test_image(&base_test_dir, "", test_image_name, TEST_PNG_URL);

    // WHEN reading resources from a folder
    let resources_read = resource_reader::read_folder(&base_test_dir);

    // THEN the resource info should be correct
    assert_eq!(resources_read.len(), 1);
    assert_eq!(resources_read[0].id, resource_processor::md5(test_image_name));
    assert_eq!(resources_read[0].path, test_image_1_path);
    assert_eq!(resources_read[0].content_type, "image/png");
    assert_eq!(resources_read[0].name, test_image_name);

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn read_gif_image_resource() {
    // GIVEN is a folder with one gif image
    let base_test_dir = create_temp_folder();
    let test_image_name = "test_image_1.gif";
    let test_image_1_path = create_test_image(&base_test_dir, "", test_image_name, TEST_GIF_URL);

    // WHEN reading resources from a folder
    let resources_read = resource_reader::read_folder(&base_test_dir);

    // THEN the resource info should be correct
    assert_eq!(resources_read.len(), 1);
    assert_eq!(resources_read[0].id, resource_processor::md5(test_image_name));
    assert_eq!(resources_read[0].path, test_image_1_path);
    assert_eq!(resources_read[0].content_type, "image/gif");
    assert_eq!(resources_read[0].name, test_image_name);

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn read_no_images_dir() {
    // GIVEN is a folder structure with no images
    let base_test_dir = create_temp_folder();
    create_test_file(&base_test_dir, "", "test_file.txt");

    // WHEN reading resources from a folder
    let resources_read = resource_reader::read_folder(&base_test_dir);

    // THEN two resources should be found
    assert_eq!(resources_read.len(), 0);

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn read_empty_dir() {
    // GIVEN is an empty folder
    let base_test_dir = create_temp_folder();

    // WHEN reading resources from a folder
    let resources_read = resource_reader::read_folder(&base_test_dir);

    // THEN two resources should be found
    assert_eq!(resources_read.len(), 0);

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn read_non_existent_folder() {
    // GIVEN is a folder path that does not exist
    let base_test_dir = PathBuf::from("/some/non/existent/path");

    // WHEN reading resources from a folder
    let resources_read = resource_reader::read_folder(&base_test_dir);

    // THEN two resources should be found
    assert_eq!(resources_read.len(), 0);

    // cleanup
    cleanup(&base_test_dir);
}

/// Creates a test image withing a folder
fn create_test_image(base_dir: &PathBuf, sub_dir: &str, file_name: &str, image_url: &str) -> String {
    let target_dir = base_dir.clone().join(sub_dir);

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).unwrap();
    }

    let test_image_path = target_dir.join(file_name);

    let mut image_data: Vec<u8> = vec![];
    reqwest::blocking::get(image_url)
        .unwrap()
        .copy_to(&mut image_data)
        .unwrap();
    fs::write(&test_image_path, &image_data).unwrap_or_else(|_| panic!("error while writing test image {}", test_image_path.to_str().unwrap()));

    test_image_path.to_str().unwrap().to_string()
}

/// Removes the test folder after test run
fn cleanup(test_dir: &PathBuf) {
    let _ = fs::remove_dir_all(&test_dir);
}

/// Creates a test file withing a folder
fn create_test_file(base_dir: &PathBuf, sub_dir: &str, file_name: &str) -> String {
    let target_dir = base_dir.clone().join(sub_dir);

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).unwrap();
    }

    let test_file_path = target_dir.join(file_name);

    fs::write(&test_file_path, b"test").unwrap_or_else(|_| panic!("error while writing test image {}", test_file_path.to_str().unwrap()));

    test_file_path.to_str().unwrap().to_string()
}

/// Creates a temp folder with the given name and returns its full path
fn create_temp_folder() -> PathBuf {
    let random_string = rand::thread_rng().gen::<u32>().to_string();
    let test_dir: PathBuf = env::temp_dir().join(TEST_TEMP_BASE_DIR).join(random_string);

    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).expect("Failed to remove test dir");
    }

    fs::create_dir_all(&test_dir).unwrap();

    test_dir
}