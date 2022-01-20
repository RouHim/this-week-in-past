use std::collections::HashMap;

use evmap::ReadHandle;

use crate::geo_location::GeoLocation;
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
        display_value.push_str(
            taken_date.date().format("%d.%m.%Y").to_string().as_str()
        );
    }

    let city_name = resource.location.and_then(resolve_city_name);

    if let Some(city_name) = city_name {
        if resource.taken.is_some() {
            display_value.push_str(", ");
        }

        display_value.push_str(city_name.as_str());
    }

    display_value
}

fn resolve_city_name(geo_location: GeoLocation) -> Option<String> {
    reqwest::blocking::get(format!(
        "https://api.bigdatacloud.net/data/reverse-geocode?latitude={}&longitude={}&localityLanguage=de&key={}",
        geo_location.latitude,
        geo_location.longitude,
        "6b8aad17eba7449d9d93c533359b0384",
    ))
        .and_then(|response| response.text()).ok()
        .and_then(|json_string| serde_json::from_str::<HashMap<String, serde_json::Value>>(&json_string).ok())
        .and_then(|json_data| json_data.get("city").map(|city| city.to_string()))
}

