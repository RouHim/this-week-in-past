use std::env;

use actix_web::web::Data;
use rand::prelude::SliceRandom;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::geo_location;
use crate::resource_reader::RemoteResource;
use crate::resource_store::ResourceStore;

/// Returns resources that was taken this week in the past
/// The resources are shuffled, to the result is not deterministic
/// Excluded are hidden resources
pub fn get_this_week_in_past(
    resource_store: &ResourceStore,
) -> Vec<String> {
    // TODO: solve this with a select statement, when kv store is migrated to sqlite
    let hidden_resources = resource_store.get_all_hidden();

    let mut resource_ids: Vec<String> = resource_store.get_all_resource_values()
        .par_iter()
        .map(|resource_json_string| serde_json::from_str::<RemoteResource>(resource_json_string.as_str()).unwrap_or_else(|_| panic!("Parsing of '{resource_json_string}' failed!")))
        .filter(|resource| resource.is_this_week())
        .map(|resource| resource.id)
        .filter(|resource_id| !hidden_resources.contains(resource_id))
        .collect();

    // shuffle resource keys
    let mut rng = rand::thread_rng();
    resource_ids.shuffle(&mut rng);

    resource_ids
}

/// Builds the display value for the specified resource
/// The display value contains the date and location of a resource
pub async fn build_display_value(resource: RemoteResource, resource_store: &ResourceStore) -> String {
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
    let city_name = get_city_name(resource, resource_store).await;
    if let Some(city_name) = city_name {
        display_value.push_str(", ");
        display_value.push_str(city_name.as_str());
    }

    display_value.trim().to_string()
}

/// Returns the city name for the specified resource
/// The city name is taken from the cache, if available
/// If not, the city name is taken from the geo location service
async fn get_city_name(resource: RemoteResource, resource_store: &ResourceStore) -> Option<String> {
    let resource_location = resource.location?;
    let resource_location_string = resource_location.to_string();

    // Check if cache contains resource location
    if resource_store.location_exists(resource_location_string.as_str()) {
        resource_store.get_location(resource_location_string.as_str())
    } else {
        // Get city name
        let city_name = geo_location::resolve_city_name(resource_location).await;

        if let Some(city_name) = &city_name {
            // Write to cache
            resource_store.add_location(resource_location_string, city_name.clone());
        }

        city_name
    }
}

/// Selects a random, not hidden, resource
/// The id of the resource is returned
pub fn random_entry(
    resource_store: Data<ResourceStore>,
) -> Option<String> {
    // TODO: improve this by joining hidden table with resources table

    resource_store.get_random_resource()?;

    let mut resource_id = resource_store.get_random_resource().unwrap();

    let mut tries = 0;
    // Try at most 100 times to get a new random image, otherwise show nothing
    while tries < 100 && resource_store.is_hidden(resource_id.as_str()) {
        resource_id = resource_store.get_random_resource().unwrap();
        tries += 1;
        println!("{tries}");
    }

    if tries == 100 {
        None
    } else {
        Some(resource_id)
    }
}