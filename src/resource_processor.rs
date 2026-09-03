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
/// Directly resolves via offline `cities500` RTree (<1ms) — no persistent cache.
/// The historic `geo_location_cache` SQLite table is dropped via migration 04.
async fn get_city_name(
    resource: &ImageResource,
    _resource_store: &ResourceStore,
) -> Option<String> {
    let resource_location = resource.location?;
    geo_location::resolve_city_name(resource_location).await
}
