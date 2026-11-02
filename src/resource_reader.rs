use std::path::Path;

use chrono::{Local, NaiveDateTime};
use exif::Exif;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use serde::{Deserialize, Serialize};

use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;
use crate::{exif_reader, filesystem_client, ResourceReader};

/// Operations over the configured resource roots.
impl ResourceReader {
    /// Returns `resource_path` relative to the longest matching configured resource root.
    /// Falls back to the stored path when no configured root contains it, e.g. after
    /// `RESOURCE_PATHS` changed while the index still holds resources from the old root.
    pub fn relative_path(&self, resource_path: &str) -> String {
        let path = Path::new(resource_path);
        let mut longest_root_components = 0;
        let mut relative_path = path;

        for root in &self.local_resource_paths {
            let Ok(candidate) = path.strip_prefix(root) else {
                continue;
            };
            let root_components = Path::new(root).components().count();
            if root_components >= longest_root_components {
                longest_root_components = root_components;
                relative_path = candidate;
            }
        }

        relative_path.to_string_lossy().into_owned()
    }

    pub fn read_all(&self) -> Vec<ImageResource> {
        self.local_resource_paths
            .par_iter()
            .map(|path_str| Path::new(path_str.as_str()))
            .flat_map(filesystem_client::read_files_recursive)
            .map(|resource| filesystem_client::fill_exif_data(&resource))
            .collect()
    }
}

/// A image resource that is available on the filesystem
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageResource {
    pub id: String,
    pub path: String,
    pub content_type: String,
    pub name: String,
    pub content_length: u64,
    pub last_modified: NaiveDateTime,
    pub taken: Option<NaiveDateTime>,
    pub location: Option<GeoLocation>,
    pub orientation: Option<ImageOrientation>,
}

/// Impl Default for ImageResource
impl Default for ImageResource {
    fn default() -> Self {
        ImageResource {
            id: "".to_string(),
            path: "".to_string(),
            content_type: "".to_string(),
            name: "".to_string(),
            content_length: 0,
            last_modified: Local::now().naive_local(),
            taken: None,
            location: None,
            orientation: None,
        }
    }
}

/// Augments the provided resource with meta information
/// The meta information is extracted from the exif data
/// If the exif data is not available, the meta information is extracted from the gps data
/// If the gps data is not available, the meta information is extracted from the file name
pub fn fill_exif_data(resource: &ImageResource, maybe_exif_data: Option<Exif>) -> ImageResource {
    let mut taken_date = None;
    let mut location = None;
    let mut orientation = None;

    if let Some(exif_data) = maybe_exif_data {
        taken_date = exif_reader::get_exif_date(&exif_data);
        location = exif_reader::detect_location(&exif_data);
        orientation = exif_reader::detect_orientation(&exif_data);
    }

    if taken_date.is_none() {
        taken_date = exif_reader::detect_date_by_name(&resource.path);
    }

    let mut augmented_resource = resource.clone();
    augmented_resource.taken = taken_date;
    augmented_resource.location = location;
    augmented_resource.orientation = orientation;

    augmented_resource
}

/// Instantiates a new resource reader for the given paths
pub fn new(resource_folder_paths: &str) -> ResourceReader {
    let local_resource_paths: Vec<String> = resource_folder_paths
        .split(',')
        .map(|entry| entry.to_string())
        .map(|entry| entry.trim().to_string())
        .collect();

    local_resource_paths.iter().for_each(|entry| verify(entry));

    ResourceReader {
        local_resource_paths,
    }
}

/// Ensure that all folder exists
fn verify(path: &str) {
    let folder_path = Path::new(path);
    let exists = folder_path.exists();
    if !exists {
        panic!("{} does not exists", path);
    }

    let is_dir = folder_path.is_dir();
    if !is_dir {
        panic!("{} is not a folder", path);
    }
}
