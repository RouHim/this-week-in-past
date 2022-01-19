use std::fmt::Display;
use evmap::ReadHandle;

use crate::WebDavResource;

pub fn md5(string: &str) -> String {
    format!("{:x}", md5::compute(string.as_bytes()))
}

pub fn get_this_week_in_past(kv_reader: &ReadHandle<String, String>) -> Vec<String> {
    kv_reader.read().unwrap()
        .iter()
        .map(|(_, v)| serde_json::from_str::<WebDavResource>(v.get_one().unwrap()).unwrap())
        .filter(|resource| resource.is_this_week())
        .map(|resource| resource.id)
        .collect()
}

pub fn get_all(kv_reader: &ReadHandle<String, String>) -> Vec<String> {
    kv_reader.read().unwrap()
        .iter()
        .map(|(_, v)| serde_json::from_str::<WebDavResource>(v.get_one().unwrap()).unwrap())
        .map(|resource| resource.id)
        .collect()
}

pub fn build_display_value(resource: WebDavResource) -> String {
    let mut display_value: String = String::new();

    if let Some(taken_date) = resource.taken {
        display_value.push_str(taken_date.date().format("%Y-%m-%d").to_string().as_str());
    }

    if let Some(location) = resource.location {

    }

    return display_value;
}