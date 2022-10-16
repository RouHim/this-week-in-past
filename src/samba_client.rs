use std::io::Read;
use std::path::PathBuf;

use lazy_static::lazy_static;
use log::error;
use pavao::{
    SmbClient, SmbCredentials, SmbDirent, SmbDirentType, SmbOpenOptions, SmbOptions, SmbStat,
};
use regex::Regex;

use crate::resource_reader::{RemoteResource, RemoteResourceType};
use crate::{resource_processor, resource_reader, utils};

/// Reads all files of a samba folder and returns all found resources
/// The folder is recursively searched
pub fn read_all_samba_files(
    samba_client_index: usize,
    samba_client: &SmbClient,
) -> Vec<RemoteResource> {
    read_all_samba_files_recursive(samba_client_index, samba_client, "/")
}

fn read_all_samba_files_recursive(
    samba_client_index: usize,
    client: &SmbClient,
    path: &str,
) -> Vec<RemoteResource> {
    let smb_dir = client.list_dir(path);

    if smb_dir.is_err() {
        error!("Could not open folder: {}", path);
        return vec![];
    }
    let smb_dir = smb_dir.unwrap();

    smb_dir
        .iter()
        .flat_map(|dir_entry| {
            let entry_path = PathBuf::from(path.to_string()).join(PathBuf::from(dir_entry.name()));
            let entry_path_str = entry_path.as_path().to_str().unwrap();

            if dir_entry.get_type() == SmbDirentType::File {
                let file_metadata = client.stat(entry_path_str).unwrap();
                read_samba_file(samba_client_index, entry_path_str, dir_entry, file_metadata)
            } else {
                read_all_samba_files_recursive(samba_client_index, client, entry_path_str)
            }
        })
        .collect()
}

fn read_samba_file(
    samba_client_index: usize,
    path: &str,
    samba_file: &SmbDirent,
    samba_stat: SmbStat,
) -> Vec<RemoteResource> {
    let file_name = samba_file.name();
    let mime_type: &str = mime_guess::from_path(file_name).first_raw().unwrap_or("");

    // Cancel if no image file
    if !mime_type.starts_with("image/") {
        return vec![];
    }

    vec![RemoteResource {
        id: resource_processor::md5(file_name),
        path: path.to_string(),
        content_type: mime_type.to_string(),
        name: file_name.to_string(),
        content_length: samba_stat.size,
        last_modified: utils::to_date_time(samba_stat.modified),
        taken: None,
        location: None,
        orientation: None,
        resource_type: RemoteResourceType::Samba,
        samba_client_index,
    }]
}

pub fn create_smb_client(smb_connection_string: &str) -> SmbClient {
    lazy_static! {
        static ref SAMBA_CONNECTION_PATTERN: Regex = Regex::new(
            // Example: "smb://user:passwd@192.168.0.1//share/photos"
            r"smb://(?P<user>.*):(?P<password>.*)@(?P<server>.*)/(?P<path>/.*)"
        ).unwrap();
    }

    if let Some(samba_pattern_match) = SAMBA_CONNECTION_PATTERN.captures(smb_connection_string) {
        let server = format!(
            "smb://{}",
            samba_pattern_match
                .name("server")
                .expect("No samba server specified")
                .as_str()
        );
        let share = samba_pattern_match
            .name("path")
            .expect("No samba path specified")
            .as_str();
        let username = samba_pattern_match
            .name("user")
            .expect("No username specified")
            .as_str();
        let password = samba_pattern_match
            .name("password")
            .expect("No password specified")
            .as_str();

        SmbClient::new(
            SmbCredentials::default()
                .server(server)
                .share(share)
                .username(username)
                .password(password),
            SmbOptions::default(),
        )
        .unwrap()
    } else {
        panic!("Could not connect to {smb_connection_string}");
    }
}

pub fn fill_exif_data(resource: &RemoteResource, smb_client: &SmbClient) -> RemoteResource {
    let smb_file = smb_client
        .open_with(&resource.path, SmbOpenOptions::default().read(true))
        .unwrap();

    let mut bufreader = std::io::BufReader::new(smb_file);
    let exif_reader = exif::Reader::new();
    let maybe_exif_data = exif_reader.read_from_container(&mut bufreader).ok();

    resource_reader::fill_exif_data(resource, maybe_exif_data)
}

pub fn read(smb_connection_url: &str, resource: &RemoteResource) -> Vec<u8> {
    let smb_client = create_smb_client(smb_connection_url);

    let mut smb_file = smb_client
        .open_with(&resource.path, SmbOpenOptions::default().read(true))
        .unwrap();
    let mut file_data = Vec::new();
    smb_file
        .read_to_end(&mut file_data)
        .unwrap_or_else(|_| panic!("Can't read samba file: {}", resource.path));
    drop(smb_file);
    drop(smb_client);
    file_data
}
