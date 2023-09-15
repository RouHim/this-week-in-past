use std::fmt::{Display, Formatter};
use std::path::Path;

use chrono::{Local, NaiveDateTime, TimeZone};
use exif::Exif;

use now::DateTimeNow;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use serde::{Deserialize, Serialize};

use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;
use crate::{exif_reader, filesystem_client, ResourceReader};

/// Returns all available resources
impl ResourceReader {
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

impl ImageResource {
    /// Checks if the resource was taken in the past but in this calendar week
    pub fn is_this_week(&self) -> bool {
        if self.taken.is_none() {
            return false;
        }

        let current_week_of_year = Local::now().week_of_year();
        let resource_week_of_year = Local
            .from_local_datetime(&self.taken.unwrap())
            .unwrap()
            .week_of_year();

        current_week_of_year == resource_week_of_year
    }
}

/// Renders the resource as a string
impl Display for ImageResource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {} {:?} {:?}",
            self.id,
            self.path,
            self.content_type,
            self.name,
            self.content_length,
            self.last_modified,
            self.taken,
            self.location,
        )
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
