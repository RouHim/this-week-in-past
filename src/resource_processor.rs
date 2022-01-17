use evmap::ReadHandle;

use crate::{WebDavClient, WebDavResource};

pub fn md5(string: &String) -> String {
    format!("{:x}", md5::compute(string.as_bytes()))
}

pub fn get_this_week_in_past(web_dav_client: &WebDavClient, kv_reader: &ReadHandle<String, String>) -> Vec<String> {
    kv_reader
        .read().unwrap()
        .iter()
        .map(|(_, v)| serde_json::from_str::<WebDavResource>(v.get_one().unwrap()).unwrap())
        .filter(|resource| resource.is_this_week())
        .map(|resource| resource.id)
        .collect()
}