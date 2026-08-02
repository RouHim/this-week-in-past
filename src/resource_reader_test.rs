use std::io::Read;
use std::path::{Path, PathBuf};
use std::{env, fs};

use chrono::NaiveDateTime;
use rand::RngExt;

use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;
use crate::{filesystem_client, utils};

const TEST_JPEG_EXIF_URL: &str =
    "https://raw.githubusercontent.com/ianare/exif-samples/master/jpg/gps/DSCN0010.jpg";
const TEST_JPEG_URL: &str = "https://www.w3.org/People/mimasa/test/imgformat/img/w3c_home.jpg";
const TEST_PNG_URL: &str = "https://www.w3.org/People/mimasa/test/imgformat/img/w3c_home.png";
const TEST_GIF_URL: &str = "https://www.w3.org/People/mimasa/test/imgformat/img/w3c_home.gif";
const TEST_FOLDER_NAME: &str = "resource_reader_test";

#[test]
fn read_dir_recursive() {
    // GIVEN is a folder structure with two assets and another file type
    let base_test_dir = create_temp_folder();
    create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_URL);
    create_test_image(&base_test_dir, "sub1", "test_image_2.jpg", TEST_JPEG_URL);
    create_test_file(&base_test_dir, "sub2", "test_file.txt");

    // WHEN reading resources from a folder
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);

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
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);

    // THEN the resource info should be correct
    assert_eq!(resources_read.len(), 1);
    assert_eq!(resources_read[0].id, utils::md5(test_image_name));
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
    let resources_read = filesystem_client::fill_exif_data(
        &filesystem_client::read_files_recursive(&base_test_dir)[0],
    );

    // THEN the resource metadata should be correct
    assert_eq!(
        resources_read.taken,
        Some(NaiveDateTime::parse_from_str("2008-10-22T16:28:39", "%Y-%m-%dT%H:%M:%S").unwrap())
    );
    assert_eq!(
        resources_read.orientation,
        Some(ImageOrientation {
            rotation: 0,
            mirror_vertically: false,
        })
    );
    assert_eq!(
        resources_read.location,
        Some(GeoLocation {
            latitude: 43.46745,
            longitude: 11.885126,
        })
    );

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
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);

    // THEN the resource info should be correct
    assert_eq!(resources_read.len(), 1);
    assert_eq!(resources_read[0].id, utils::md5(test_image_name));
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
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);

    // THEN the resource info should be correct
    assert_eq!(resources_read.len(), 1);
    assert_eq!(resources_read[0].id, utils::md5(test_image_name));
    assert_eq!(resources_read[0].path, test_image_1_path);
    assert_eq!(resources_read[0].content_type, "image/gif");
    assert_eq!(resources_read[0].name, test_image_name);

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn read_no_images_dir() {
    // GIVEN is a folder structure with no assets
    let base_test_dir = create_temp_folder();
    create_test_file(&base_test_dir, "", "test_file.txt");

    // WHEN reading resources from a folder
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);

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
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);

    // THEN two resources should be found
    assert_eq!(resources_read.len(), 0);

    // cleanup
    cleanup(&base_test_dir);
}

#[cfg(unix)]
#[test]
fn read_dir_with_unreadable_file_does_not_panic() {
    use std::os::unix::fs::PermissionsExt;

    // GIVEN is a folder with one readable image and one unreadable file
    let base_test_dir = create_temp_folder();
    create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_URL);
    let unreadable_file = base_test_dir.join("unreadable.jpg");
    fs::write(&unreadable_file, b"not an image").unwrap();
    let mut permissions = fs::metadata(&unreadable_file).unwrap().permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&unreadable_file, permissions).unwrap();

    // Skip the test when running as root (permissions are not enforced)
    if fs::File::open(&unreadable_file).is_ok() {
        cleanup(&base_test_dir);
        return;
    }

    // WHEN reading resources from a folder
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);

    // THEN the readable image is found and the unreadable file is skipped without panicking
    assert_eq!(resources_read.len(), 1);
    assert_eq!(resources_read[0].name, "test_image_1.jpg");

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn fill_exif_data_missing_file_does_not_panic() {
    // GIVEN is a resource whose file is removed before exif data is read
    let base_test_dir = create_temp_folder();
    create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_URL);
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);
    assert_eq!(resources_read.len(), 1);
    fs::remove_file(&resources_read[0].path).unwrap();

    // WHEN filling exif data for the now missing file
    let augmented = filesystem_client::fill_exif_data(&resources_read[0]);

    // THEN the resource is returned unchanged without panicking
    assert_eq!(augmented.taken, None);
    assert_eq!(augmented.location, None);

    // cleanup
    cleanup(&base_test_dir);
}

#[cfg(unix)]
#[test]
fn read_dir_with_non_utf8_file_name_does_not_panic() {
    use std::os::unix::ffi::OsStringExt;

    // GIVEN is a folder with one readable image and one file with a non-UTF8 name
    let base_test_dir = create_temp_folder();
    create_test_image(&base_test_dir, "", "test_image_1.jpg", TEST_JPEG_URL);
    let non_utf8_name =
        std::ffi::OsString::from_vec(vec![0x74, 0x65, 0x73, 0x74, 0xFF, 0x2E, 0x6A, 0x70, 0x67]); // "test\xFF.jpg"
    fs::write(base_test_dir.join(&non_utf8_name), b"not an image").unwrap();

    // WHEN reading resources from a folder
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);

    // THEN the readable image is found and the non-UTF8 file is skipped without panicking
    assert_eq!(resources_read.len(), 1);
    assert_eq!(resources_read[0].name, "test_image_1.jpg");

    // cleanup
    cleanup(&base_test_dir);
}

#[test]
fn read_non_existent_folder() {
    // GIVEN is a folder path that does not exist
    let base_test_dir = PathBuf::from("/some/non/existent/path");

    // WHEN reading resources from a folder
    let resources_read = filesystem_client::read_files_recursive(&base_test_dir);

    // THEN two resources should be found
    assert_eq!(resources_read.len(), 0);

    // cleanup
    cleanup(&base_test_dir);
}

/// Creates a test image withing a folder
fn create_test_image(base_dir: &Path, sub_dir: &str, file_name: &str, image_url: &str) -> String {
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

    fs::write(&test_image_path, &data).unwrap_or_else(|_| {
        panic!(
            "error while writing test image {}",
            test_image_path.to_str().unwrap()
        )
    });

    test_image_path.to_str().unwrap().to_string()
}

/// Removes the test folder after test run
fn cleanup(test_dir: &PathBuf) {
    let _ = fs::remove_dir_all(test_dir);
}

/// Creates a test file withing a folder
fn create_test_file(base_dir: &Path, sub_dir: &str, file_name: &str) -> String {
    let target_dir = base_dir.join(sub_dir);

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).unwrap();
    }

    let test_file_path = target_dir.join(file_name);

    fs::write(&test_file_path, b"test").unwrap_or_else(|_| {
        panic!(
            "error while writing test image {}",
            test_file_path.to_str().unwrap()
        )
    });

    test_file_path.to_str().unwrap().to_string()
}

/// Creates a temp folder with the given name and returns its full path
fn create_temp_folder() -> PathBuf {
    let random_string = rand::rng().random::<u32>().to_string();
    let test_dir: PathBuf = env::temp_dir().join(TEST_FOLDER_NAME).join(random_string);

    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).expect("Failed to remove test dir");
    }

    fs::create_dir_all(&test_dir).unwrap();

    test_dir
}
