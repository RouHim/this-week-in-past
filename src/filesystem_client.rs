use core::option::Option::None;
use std::fs;
use std::path::{Path, PathBuf};

use log::error;

use crate::resource_reader::RemoteResource;
use crate::{resource_reader, utils};

/// Reads all files of a folder and returns all found resources
/// The folder is recursively searched
pub fn read_files_recursive(path: &Path) -> Vec<RemoteResource> {
    let maybe_folder_path = fs::File::open(path);

    if maybe_folder_path.is_err() {
        error!("Could not open folder: {}", path.display());
        return vec![];
    }

    let metadata = maybe_folder_path
        .unwrap()
        .metadata()
        .expect("Failed to read metadata");

    if metadata.is_file() {
        return vec![];
    }

    let paths = fs::read_dir(path)
        .unwrap_or_else(|_| panic!("Failed to read directory: {}", &path.to_str().unwrap()));

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

/// Reads a single file and returns the found resource
/// Checks if the file is a supported resource currently all image types
fn read_resource(file_path: &PathBuf) -> Vec<RemoteResource> {
    let absolute_file_path = file_path.to_str().unwrap();
    let file_name = file_path.as_path().file_name().unwrap().to_str().unwrap();

    let file = fs::File::open(file_path)
        .unwrap_or_else(|_| panic!("Failed to read file {}", absolute_file_path));
    let metadata = file
        .metadata()
        .unwrap_or_else(|_| panic!("Failed to read metadata {}", absolute_file_path));

    let is_file = metadata.is_file();
    let mime_type: &str = mime_guess::from_path(file_name).first_raw().unwrap_or("");

    // Cancel if no image file
    if !is_file || !mime_type.starts_with("image/") {
        return vec![];
    }

    vec![RemoteResource {
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

/// Reads the exif data from the file and augments the remote resource with this information
pub fn fill_exif_data(resource: &RemoteResource) -> RemoteResource {
    let file_path = resource.path.as_str();
    let file = fs::File::open(file_path).unwrap();

    let mut bufreader = std::io::BufReader::new(&file);
    let exif_reader = exif::Reader::new();
    let maybe_exif_data = exif_reader.read_from_container(&mut bufreader).ok();

    resource_reader::fill_exif_data(resource, maybe_exif_data)
}
