use std::env;

use actix_web::web::Data;
use evmap::ReadHandle;
use rand::prelude::SliceRandom;
use rand::Rng;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::geo_location;
use crate::kv_store::KvStore;
use crate::resource_reader::RemoteResource;
use crate::resource_store::ResourceStore;

/// Returns resources that was taken this week in the past
/// The resources are shuffled, to the result is not deterministic
/// Excluded are hidden resources
pub fn get_this_week_in_past(kv_reader: &ReadHandle<String, String>, resource_store: &ResourceStore) -> Vec<String> {
    let hidden_resources = resource_store.get_all_hidden();

    let mut resource_ids: Vec<String> = kv_reader
        .read()
        .unwrap()
        .iter()
        .map(|(_, v)| serde_json::from_str::<RemoteResource>(v.get_one().unwrap()).unwrap())
        .collect::<Vec<RemoteResource>>()
        .par_iter()
        .filter(|resource| resource.is_this_week())
        .map(|resource| resource.clone().id)
        .filter(|resource_id| hidden_resources.contains(resource_id))
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
pub async fn build_display_value(resource: RemoteResource, geo_location_cache: &KvStore) -> String {
    let mut display_value: String = String::new();

    // Append taken date
    if let Some(taken_date) = resource.taken {
        let date_format: String =
            env::var("DATE_FORMAT").unwrap_or_else(|_| "%d.%m.%Y".to_string());
        display_value.push_str(
            taken_date
                .date()
                .format(date_format.as_str())
                .to_string()
                .as_str(),
        );
    };

    // Append city name
    let city_name = get_city_name(resource, geo_location_cache).await;
    if let Some(city_name) = city_name {
        display_value.push_str(", ");
        display_value.push_str(city_name.as_str());
    }

    display_value.trim().to_string()
}

/// Returns the city name for the specified resource
/// The city name is taken from the cache, if available
/// If not, the city name is taken from the geo location service
async fn get_city_name(resource: RemoteResource, geo_location_cache: &KvStore) -> Option<String> {
    let resource_location = resource.location?;
    let resource_location_string = resource_location.to_string();

    // Check if cache contains resource location
    if geo_location_cache.contains_key(resource_location_string.as_str()) {
        geo_location_cache.get(resource_location_string.as_str())
    } else {
        // Get city name
        let city_name = geo_location::resolve_city_name(resource_location).await;

        if let Some(city_name) = &city_name {
            // Write to cache
            geo_location_cache.insert(resource_location_string, city_name.clone());
        }

        city_name
    }
}

/// Selects a random, not hidden, resource
/// The id of the resource is returned
pub fn random_entry(kv_reader: &ReadHandle<String, String>, resource_store: Data<ResourceStore>) -> Option<String> {
    let entry_count = kv_reader.read().unwrap().len();
    if entry_count == 0 {
        return None;
    }

    let mut tries = 0;
    let mut resource_id = get_random_resource(kv_reader, entry_count);

    // Try at most 100 times to get a new random image, otherwise show nothing
    while tries < 100 && resource_store.is_hidden(resource_id.as_str()) {
        resource_id = get_random_resource(kv_reader, entry_count);
        tries += 1;
        println!("{tries}");
    }

    if tries == 100 { None } else { Some(resource_id) }
}

/// Reads a random resource from data store
fn get_random_resource(kv_reader: &ReadHandle<String, String>, entry_count: usize) -> String {
    let random_index = rand::thread_rng().gen_range(0..entry_count);
    kv_reader
        .read()
        .unwrap()
        .iter()
        .nth(random_index)
        .map(|(key, _)| key.clone())
        .unwrap()
}

/// Reads the directory to store the cache into, needs write rights
pub fn get_cache_dir() -> String {
    env::var("CACHE_DIR").expect("CACHE_DIR is missing")
}
