use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;

use chrono::{Local, NaiveDateTime, TimeZone};

use now::DateTimeNow;

use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use serde::{Deserialize, Serialize};

use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;
use crate::{exif_reader, filesystem_client, samba_client};

/// A resource reader that reads available resources from the filesystem
#[derive(Clone)]
pub struct ResourceReader {
    /// Holds all specified local paths
    pub local_resource_paths: Vec<String>,
    /// Holds all specified local paths
    pub samba_resource_paths: Vec<String>,
}

impl ResourceReader {
    /// Reads the specified resource from the filesystem
    /// Returns the resource file data
    pub fn read_resource_data(&self, resource: &RemoteResource) -> Vec<u8> {
        match resource.resource_type {
            RemoteResourceType::Local => fs::read(resource.path.clone()).unwrap(),
            RemoteResourceType::Samba => {
                // TODO: implement me
                vec![]
            }
        }
    }

    /// Returns all available resources from the filesystem
    pub fn list_all_resources(&self) -> Vec<RemoteResource> {
        let local_resources: Vec<RemoteResource> = self
            .local_resource_paths
            .par_iter()
            .flat_map(|path| {
                filesystem_client::read_all_local_files_recursive(&PathBuf::from(path))
            })
            .map(|resource| exif_reader::fill_exif_data(&resource))
            .collect();

        let samba_resources: Vec<RemoteResource> = self
            .samba_resource_paths
            .par_iter()
            .flat_map(|smb_path| samba_client::read_all_samba_files(smb_path))
            .map(|resource| exif_reader::fill_exif_data(&resource))
            .collect();

        [local_resources, samba_resources].concat()
    }
}

/// Instantiates a new resource reader for the given paths
pub fn new(resource_folder_paths: &str) -> ResourceReader {
    let mut local_resource_paths = vec![];
    let mut samba_resource_paths = vec![];

    for resource_folder in resource_folder_paths.split(',').map(|s| s.to_string()) {
        if resource_folder.starts_with("smb://") {
            samba_resource_paths.push(resource_folder);
        } else {
            local_resource_paths.push(resource_folder);
        }
    }

    ResourceReader {
        local_resource_paths,
        samba_resource_paths,
    }
}

/// A remote resource that is available on the filesystem
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteResource {
    pub id: String,
    pub path: String,
    pub content_type: String,
    pub name: String,
    pub content_length: u64,
    pub last_modified: NaiveDateTime,
    pub taken: Option<NaiveDateTime>,
    pub location: Option<GeoLocation>,
    pub orientation: Option<ImageOrientation>,
    pub resource_type: RemoteResourceType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RemoteResourceType {
    Local,
    Samba,
}

impl Display for RemoteResourceType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            RemoteResourceType::Local => write!(f, "Local"),
            RemoteResourceType::Samba => write!(f, "Samba"),
        }
    }
}

impl RemoteResource {
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
impl Display for RemoteResource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {} {:?} {:?} {}",
            self.id,
            self.path,
            self.content_type,
            self.name,
            self.content_length,
            self.last_modified,
            self.taken,
            self.location,
            self.resource_type,
        )
    }
}
