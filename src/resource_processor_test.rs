use assertor::*;

use crate::geo_location::GeoLocation;
use crate::{geo_location, resource_processor};

#[actix_rt::test]
async fn resolve_koblenz() {
    // GIVEN are the geo coordinates for Koblenz
    let geo_location: GeoLocation = GeoLocation {
        latitude: 50.35357,
        longitude: 7.57883,
    };

    // WHEN resolving the city name
    let city_name = geo_location::resolve_city_name(geo_location).await;

    // THEN the resolved city name should be Koblenz
    assert_that!(city_name).is_equal_to(Some("Koblenz".to_string()));
}

#[actix_rt::test]
async fn resolve_amsterdam() {
    // GIVEN are the geo coordinates for Amsterdam
    let geo_location: GeoLocation = GeoLocation {
        latitude: 52.37403,
        longitude: 4.88969,
    };

    // WHEN resolving the city name
    let city_name = geo_location::resolve_city_name(geo_location).await;

    // THEN the resolved city name should be Amsterdam
    assert_that!(city_name).is_equal_to(Some("Amsterdam".to_string()));
}

#[actix_rt::test]
async fn resolve_kottenheim() {
    // GIVEN are the geo coordinates for Kottenheim
    let geo_location: GeoLocation = GeoLocation {
        latitude: 50.34604,
        longitude: 7.25359,
    };

    // WHEN resolving the city name
    let city_name = geo_location::resolve_city_name(geo_location).await;

    // THEN the resolved city name should be Kottenheim
    assert_that!(city_name).is_equal_to(Some("Kottenheim".to_string()));
}

#[actix_rt::test]
async fn resolve_invalid_data() {
    // GIVEN are invalid geo coordinates
    let geo_location: GeoLocation = GeoLocation {
        latitude: -100.0,
        longitude: -100.0,
    };

    // WHEN resolving the city name
    let city_name = geo_location::resolve_city_name(geo_location).await;

    // THEN the resolved city name should be None
    assert_that!(city_name).is_equal_to(None);
}
