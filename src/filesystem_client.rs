use core::option::Option::None;
use std::fs;
use std::path::{Path, PathBuf};

use image::ImageFormat;
use lazy_static::lazy_static;
use log::{error, info, warn};

use crate::resource_reader::ImageResource;
use crate::{resource_reader, utils};

/// Reads all files of a folder and returns all found resources
/// The folder is recursively searched
pub fn read_files_recursive(path: &Path) -> Vec<ImageResource> {
    let folder_path = fs::File::open(path);

    if folder_path.is_err() {
        error!("Could not open folder: {:?}", path);
        return vec![];
    }
    let folder_path = folder_path.unwrap();
    let metadata = folder_path.metadata().unwrap();

    if metadata.is_file() {
        return vec![];
    }

    // Checks if the folder should be skipped, because it is ignored or contains a .ignore file
    if should_skip_folder(path) {
        return vec![];
    }

    let paths = fs::read_dir(path).unwrap_or_else(|error| {
        panic!(
            "Failed to read directory: {} Error:\n{}",
            path.to_str().unwrap(),
            error
        )
    });

    paths
        .flatten()
        .flat_map(|dir_entry| {
            let metadata = dir_entry.metadata().expect("Failed to read metadata");

            if metadata.is_file() {
                read_resource(&dir_entry.path())
            } else {
                read_files_recursive(&dir_entry.path())
            }
        })
        .collect()
}

/// Checks if the folder should be skipped, because it is ignored or contains a .ignore file
/// Returns true if the folder should be skipped
/// Returns false if the folder should be processed
fn should_skip_folder(path: &Path) -> bool {
    lazy_static! {
        static ref IGNORED_FOLDERS: Vec<String> = std::env::var("IGNORED_FOLDERS")
            .unwrap_or("".to_string())
            .as_str()
            .split(',')
            .map(|s| s.to_string())
            .collect();
    };

    let folder_name = path.file_name().unwrap().to_str().unwrap();
    if IGNORED_FOLDERS.contains(&folder_name.to_string()) {
        info!("Skipping folder: {:?} because it is ignored", path);
        return true;
    }

    let contains_ignore_file = fs::read_dir(path)
        .unwrap_or_else(|error| {
            panic!(
                "Failed to read directory: {} Error:\n{}",
                path.to_str().unwrap(),
                error
            )
        })
        .flatten()
        .any(|entry| {
            let metadata = entry.metadata().unwrap();
            metadata.is_file() && entry.file_name().to_str().unwrap() == ".ignore"
        });
    if contains_ignore_file {
        info!(
            "Skipping folder: {:?} because it contains a .ignore file",
            path
        );
        return true;
    }

    false
}

/// Reads a single file and returns the found resource
/// Checks if the file is a supported resource currently all image types
fn read_resource(file_path: &PathBuf) -> Vec<ImageResource> {
    let absolute_file_path = file_path.to_str().unwrap();
    let file_name = file_path.as_path().file_name().unwrap().to_str().unwrap();

    let file = fs::File::open(file_path)
        .unwrap_or_else(|error| panic!("Failed to read file {}: {}", absolute_file_path, error));

    let metadata = file.metadata().unwrap_or_else(|error| {
        panic!("Failed to read metadata {}: {}", absolute_file_path, error)
    });

    // Cancel if folder
    if !metadata.is_file() {
        return vec![];
    }

    let mime_type: &str = mime_guess::from_path(file_name).first_raw().unwrap_or("");
    let image_format = ImageFormat::from_mime_type(mime_type);

    // Cancel and print error if no supported image format
    if image_format.is_none() {
        // If the mime type is image, but the format is not supported, print a warning
        if mime_type.starts_with("image") {
            warn!(
                "{absolute_file_path} | has unsupported image format: {}",
                mime_type
            );
        }

        return vec![];
    }

    vec![ImageResource {
        id: utils::md5(file_name),
        path: absolute_file_path.to_string(),
        content_type: mime_type.to_string(),
        name: file_name.to_string(),
        content_length: metadata.len(),
        last_modified: utils::to_date_time(metadata.modified().unwrap()),
        taken: None,
        location: None,
        orientation: None,
    }]
}

/// Reads the exif data from the file and augments the image resource with this information
pub fn fill_exif_data(resource: &ImageResource) -> ImageResource {
    let file_path = resource.path.as_str();
    let file = fs::File::open(file_path).unwrap();

    let mut bufreader = std::io::BufReader::new(&file);
    let exif_reader = exif::Reader::new();
    let maybe_exif_data = exif_reader.read_from_container(&mut bufreader).ok();

    resource_reader::fill_exif_data(resource, maybe_exif_data)
}
