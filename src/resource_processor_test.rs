use assertor::*;
use std::env;

use crate::geo_location;
use crate::geo_location::GeoLocation;

#[tokio::test]
async fn resolve_koblenz() {
    if !geo_lookup_available() {
        return;
    }
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

#[tokio::test]
async fn resolve_amsterdam() {
    if !geo_lookup_available() {
        return;
    }
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

#[tokio::test]
async fn resolve_kottenheim() {
    if !geo_lookup_available() {
        return;
    }
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

#[tokio::test]
async fn resolve_negative_dms() {
    if !geo_lookup_available() {
        return;
    }
    // GIVEN are the degree minutes seconds coordinates for San Bartolomé de Tirajana
    let lat = "27 deg 45 min 22.22 sec";
    let long = "15 deg 34 min 13.76 sec";
    let lat_ref = "N";
    let long_ref = "W";

    // WHEN resolving the city name
    let dms = geo_location::from_degrees_minutes_seconds(lat, long, lat_ref, long_ref);

    // THEN the resolved city name should be San Bartolomé de Tirajana
    let city_name = geo_location::resolve_city_name(dms.unwrap()).await;
    assert_that!(city_name).is_equal_to(Some("San Bartolomé de Tirajana".to_string()));
}

#[tokio::test]
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

fn geo_lookup_available() -> bool {
    if env::var("BIGDATA_CLOUD_API_KEY").is_err() {
        eprintln!("Skipping geo lookup test: BIGDATA_CLOUD_API_KEY not set");
        return false;
    }
    true
}
