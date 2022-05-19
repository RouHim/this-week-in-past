use std::collections::HashMap;

use evmap::ReadHandle;
use rand::prelude::SliceRandom;
use rand::Rng;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde_json::Value;

use crate::geo_location::GeoLocation;
use crate::resource_reader::RemoteResource;

pub fn md5(string: &str) -> String {
    format!("{:x}", md5::compute(string.as_bytes()))
}

/// Returns resources that was taken this week in the past
/// The resources are shuffled, to the result is not deterministic
pub fn get_this_week_in_past(kv_reader: &ReadHandle<String, String>) -> Vec<String> {
    let mut resource_ids: Vec<String> = kv_reader
        .read()
        .unwrap()
        .iter()
        .map(|(_, v)| serde_json::from_str::<RemoteResource>(v.get_one().unwrap()).unwrap())
        .collect::<Vec<RemoteResource>>()
        .par_iter()
        .filter(|resource| resource.is_this_week())
        .map(|resource| resource.clone().id)
        .collect();

    // shuffle resource keys
    let mut rng = rand::thread_rng();
    resource_ids.shuffle(&mut rng);

    resource_ids
}

/// Returns all resources in the same order
pub fn get_all(kv_reader: &ReadHandle<String, String>) -> Vec<String> {
    kv_reader
        .read()
        .unwrap()
        .iter()
        .map(|(_, v)| serde_json::from_str::<RemoteResource>(v.get_one().unwrap()).unwrap())
        .map(|resource| resource.id)
        .collect()
}

/// Builds the display value for the specified resource
/// The display value contains the date and location of a resource
pub fn build_display_value(resource: RemoteResource) -> String {
    let mut display_value: String = String::new();

    if let Some(taken_date) = resource.taken {
        display_value.push_str(taken_date.date().format("%d.%m.%Y").to_string().as_str());
    }

    let city_name = resource.location.and_then(resolve_city_name);

    if let Some(city_name) = city_name {
        if resource.taken.is_some() {
            display_value.push_str(", ");
        }

        display_value.push_str(city_name.as_str());
    }

    display_value.trim().to_string()
}

/// Returns the city name for the specified geo location
/// The city name is resolved from the geo location using the bigdatacloud api
pub fn resolve_city_name(geo_location: GeoLocation) -> Option<String> {
    let response_json = reqwest::blocking::get(format!(
        "https://api.bigdatacloud.net/data/reverse-geocode?latitude={}&longitude={}&localityLanguage=de&key={}",
        geo_location.latitude,
        geo_location.longitude,
        "6b8aad17eba7449d9d93c533359b0384",
    ))
        .and_then(|response| response.text()).ok()
        .and_then(|json_string| serde_json::from_str::<HashMap<String, serde_json::Value>>(&json_string).ok());

    let mut city_name = response_json
        .as_ref()
        .and_then(|json_data| get_string_value("city", json_data))
        .filter(|city_name| !city_name.trim().is_empty());

    if city_name.is_none() {
        city_name = response_json
            .as_ref()
            .and_then(|json_data| get_string_value("locality", json_data))
            .filter(|city_name| !city_name.trim().is_empty());
    }

    city_name
}

/// Returns the string value for the specified key of an hash map
fn get_string_value(field_name: &str, json_data: &HashMap<String, Value>) -> Option<String> {
    json_data
        .get(field_name)
        .and_then(|field_value| field_value.as_str())
        .map(|field_string_value| field_string_value.to_string())
}

/// Selects a random entry from the specified resource provider
/// The id of the resource is returned
pub fn random_entry(kv_reader: &ReadHandle<String, String>) -> Option<String> {
    let entry_count = kv_reader.read().unwrap().len();
    let random_index = rand::thread_rng().gen_range(0..entry_count);
    kv_reader
        .read()
        .unwrap()
        .iter()
        .nth(random_index)
        .map(|(key, _)| key.clone())
}
