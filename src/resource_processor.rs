use std::env;

use crate::geo_location;
use crate::resource_reader::ImageResource;
/// Builds the display value for the specified resource
/// The display value contains the date and location of a resource
///
/// `_resource_store` is intentionally retained for API compatibility per
/// FR-006 and plan `2026-09-03-district-aware-city-display.md` (keep `_store`
/// param to avoid churn). Removing it would require editing
/// `src/resource_endpoint.rs:248` (`resource_store.as_ref()` call site) which
/// is outside the allowed diff scope for iteration 2 and would break
/// compilation if this signature alone changed. The offline `cities500` RTree
/// needs no store — the param is unused by design (`_`-prefixed to suppress
/// `unused_variables`).
pub async fn build_display_value(
    resource: ImageResource,
    _resource_store: &crate::resource_store::ResourceStore,
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
    let city_name = get_city_name(&resource).await;
    if let Some(city_name) = city_name {
        display_value.push_str(", ");
        display_value.push_str(city_name.as_str());
    }

    display_value.trim().to_string()
}

/// Returns the city name for the specified resource
/// Directly resolves via offline `cities500` RTree (<1ms) — no persistent cache.
/// The historic `geo_location_cache` SQLite table is dropped via migration 04.
async fn get_city_name(resource: &ImageResource) -> Option<String> {
    let resource_location = resource.location?;
    geo_location::resolve_city_name(resource_location).await
}
