use std::env;

use evmap::ReadHandle;
use rand::prelude::SliceRandom;
use rand::Rng;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::geo_location;
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
pub async fn build_display_value(resource: RemoteResource) -> String {
    let mut display_value: String = String::new();

    // Append taken date
    if let Some(taken_date) = resource.taken {
        display_value.push_str(taken_date.date().format("%d.%m.%Y").to_string().as_str());
    }

    // Append city name
    if let Some(resource_location) = resource.location {
        let city_name = geo_location::resolve_city_name(resource_location).await;

        if let Some(city_name) = city_name {
            display_value.push_str(", ");
            display_value.push_str(city_name.as_str());
        }
    }

    display_value.trim().to_string()
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

/// Reads the directory to store the cache into, needs write rights
pub fn get_cache_dir() -> String {
    env::var("CACHE_DIR").expect("CACHE_DIR is missing")
}
