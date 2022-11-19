use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;

use chrono::{Local, NaiveDateTime, TimeZone};
use exif::Exif;
use now::DateTimeNow;
use pavao::SmbClient;

use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use serde::{Deserialize, Serialize};

use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;
use crate::samba_client::create_smb_client;
use crate::{exif_reader, filesystem_client, samba_client, ResourceReader};

/// Reads the specified resource from the filesystem
/// Returns the resource file data
pub fn read_resource_data(
    resource_reader: &ResourceReader,
    resource: &RemoteResource,
) -> Option<Vec<u8>> {
    match resource.resource_type {
        RemoteResourceType::Local => fs::read(resource.path.clone()).ok(),
        RemoteResourceType::Samba => {
            let smb_connection_path = resource_reader
                .samba_resource_paths
                .get(resource.samba_client_index)
                .unwrap();
            Some(samba_client::read_file(smb_connection_path, resource))
        }
    }
}

/// Returns all available resources
impl ResourceReader {
    pub fn read_all(&self) -> Vec<RemoteResource> {
        let local_resources: Vec<RemoteResource> = self
            .local_resource_paths
            .par_iter()
            .map(|path_str| Path::new(path_str.as_str()))
            .flat_map(filesystem_client::read_files_recursive)
            .map(|resource| filesystem_client::fill_exif_data(&resource))
            .collect();

        // TODO: Improve
        // Create smb clients
        let smb_clients: Vec<SmbClient> = self
            .samba_resource_paths
            .iter()
            .map(|connection_string| create_smb_client(connection_string))
            .collect();
        // Process resources
        let samba_resources: Vec<RemoteResource> = self
            .samba_resource_paths
            .iter()
            .enumerate()
            .flat_map(|(i, _)| samba_client::read_all_samba_files(i, smb_clients.get(i).unwrap()))
            .map(|resource| {
                samba_client::fill_exif_data(
                    &resource,
                    smb_clients.get(resource.samba_client_index).unwrap(),
                )
            })
            .collect();
        // Drop smb clients
        smb_clients.iter().for_each(drop);

        [local_resources, samba_resources].concat()
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
    pub samba_client_index: usize,
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

/// Represents the type of an resource, either on the local computer or a samba resource.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RemoteResourceType {
    Local,
    Samba,
}

/// Prints either Local or Samba depending on the RemoteResourceType
impl Display for RemoteResourceType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            RemoteResourceType::Local => write!(f, "Local"),
            RemoteResourceType::Samba => write!(f, "Samba"),
        }
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

/// Augments the provided resource with meta information
/// The meta information is extracted from the exif data
/// If the exif data is not available, the meta information is extracted from the gps data
/// If the gps data is not available, the meta information is extracted from the file name
pub fn fill_exif_data(resource: &RemoteResource, maybe_exif_data: Option<Exif>) -> RemoteResource {
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
