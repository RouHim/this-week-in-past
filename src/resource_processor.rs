use chrono::{Local, NaiveDate};
use rand::{thread_rng, Rng};
use std::cmp::Ordering;
use std::env;

use crate::geo_location;
use crate::resource_reader::ImageResource;
use crate::resource_store::ResourceStore;

/// Builds the display value for the specified resource
/// The display value contains the date and location of a resource
pub async fn build_display_value(
    resource: ImageResource,
    resource_store: &ResourceStore,
) -> String {
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
    let city_name = get_city_name(&resource, resource_store).await;
    if let Some(city_name) = city_name {
        display_value.push_str(", ");
        display_value.push_str(city_name.as_str());
    }

    display_value.trim().to_string()
}

/// Returns the city name for the specified resource
/// The city name is taken from the cache, if available
/// If not, the city name is taken from the geo location service
async fn get_city_name(resource: &ImageResource, resource_store: &ResourceStore) -> Option<String> {
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

/// Shuffles the resources based on the date they were taken.
/// The resources that were taken closer to today are more likely to be at the beginning of the list.
/// The resources that were taken further away from today are more likely to be at the end of the list.
pub fn shuffle_date_weighted(resources: Vec<(String, Option<NaiveDate>)>) -> Vec<String> {
    let today = Local::now().naive_local().date();

    // Calculate weights based on the distance in days to today
    let weights: Vec<f64> = resources
        .iter()
        .map(|(_, date)| {
            let taken_date = match date {
                Some(date) => *date,
                None => return 0.0,
            };
            let distance = (today - taken_date).num_days().abs();
            1.0 / (1.0 + distance as f64)
        })
        .collect();

    // Sort the vector based on the calculated weights
    let mut sorted_resources = resources.clone();
    sorted_resources.sort_by(|a, b| {
        let a_weight = weights[resources.iter().position(|x| x == a).unwrap()];
        let b_weight = weights[resources.iter().position(|x| x == b).unwrap()];
        b_weight.partial_cmp(&a_weight).unwrap_or(Ordering::Equal)
    });

    // Shuffle the resources with a bias towards the beginning
    let mut shuffled_ids: Vec<String> = sorted_resources.into_iter().map(|(id, _)| id).collect();
    for i in (1..shuffled_ids.len()).rev() {
        let j = thread_rng().gen_range(0..=i);
        shuffled_ids.swap(i, j);
    }

    shuffled_ids
}
