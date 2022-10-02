use std::path::PathBuf;

use lazy_static::lazy_static;
use log::error;
use pavao::{SmbClient, SmbCredentials, SmbDirent, SmbDirentType, SmbOptions, SmbStat};
use regex::{Captures, Regex};

use crate::resource_reader::{RemoteResource, RemoteResourceType};
use crate::{resource_processor, utils};

/// Reads all files of a samba folder and returns all found resources
/// The folder is recursively searched
pub fn read_all_samba_files(smb_path: &str) -> Vec<RemoteResource> {
    lazy_static! {
        static ref SAMBA_CONNECTION_PATTERN: Regex = Regex::new(
            // smb://user:passwd@192.168.0.1//share/photos
            r"smb://(?P<user>.*):(?P<password>.*)@(?P<server>.*)/(?P<path>/.*)"
        ).unwrap();
    }

    if let Some(samba_pattern_match) = SAMBA_CONNECTION_PATTERN.captures(smb_path) {
        let smb_client: SmbClient = build_samba_client(samba_pattern_match);
        let files = read_all_samba_files_recursive(&smb_client, "/");
        drop(smb_client);

        files
    } else {
        vec![]
    }
}

fn read_all_samba_files_recursive(client: &SmbClient, path: &str) -> Vec<RemoteResource> {
    let smb_dir = client.list_dir(path);

    if smb_dir.is_err() {
        error!("Could not open folder: {}", path);
        return vec![];
    }
    let smb_dir = smb_dir.unwrap();

    smb_dir
        .iter()
        .flat_map(|dir_entry| {
            let entry_path = PathBuf::from(path).join(PathBuf::from(dir_entry.name()));
            let entry_path_str = entry_path.as_path().to_str().unwrap();

            if dir_entry.get_type() == SmbDirentType::File {
                let file_metadata = client.stat(entry_path_str).unwrap();
                read_samba_file(entry_path_str, dir_entry, file_metadata)
            } else {
                read_all_samba_files_recursive(client, entry_path_str)
            }
        })
        .collect()
}

fn read_samba_file(path: &str, samba_file: &SmbDirent, samba_stat: SmbStat) -> Vec<RemoteResource> {
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
    }]
}

fn build_samba_client(captures: Captures) -> SmbClient {
    let server = format!(
        "smb://{}",
        captures
            .name("server")
            .expect("No samba server specified")
            .as_str()
    );
    let share = captures
        .name("path")
        .expect("No samba path specified")
        .as_str();
    let username = captures
        .name("user")
        .expect("No username specified")
        .as_str();
    let password = captures
        .name("password")
        .expect("No password specified")
        .as_str();

    SmbClient::new(
        SmbCredentials::default()
            .server(server)
            .share(share)
            .username(username)
            .password(password),
        SmbOptions::default().one_share_per_server(true),
    )
    .unwrap()
}
