use assertor::*;

use crate::geo_location;
use crate::geo_location::GeoLocation;

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
async fn resolve_negative_dms() {
    // GIVEN are the degree minutes seconds coordinates near Playa del Ingles (Gran Canaria)
    // 27 deg 45 min 22.22 sec N, 15 deg 34 min 13.76 sec W ≈ 27.756, -15.570
    let lat = "27 deg 45 min 22.22 sec";
    let long = "15 deg 34 min 13.76 sec";
    let lat_ref = "N";
    let long_ref = "W";

    // WHEN resolving the city name
    let dms = geo_location::from_degrees_minutes_seconds(lat, long, lat_ref, long_ref);

    // THEN the resolved city name should be the nearest GeoNames native name (Playa del Ingles)
    let city_name = geo_location::resolve_city_name(dms.unwrap()).await;
    assert_that!(city_name).is_equal_to(Some("Playa del Ingles".to_string()));
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

#[actix_rt::test]
async fn resolve_mid_ocean_returns_none() {
    // GIVEN a coordinate in the middle of the Pacific Ocean (far from any city)
    let geo_location: GeoLocation = GeoLocation {
        latitude: 0.0,
        longitude: -160.0,
    };

    // WHEN resolving the city name
    let city_name = geo_location::resolve_city_name(geo_location).await;

    // THEN no city should be returned (beyond MAX_DISTANCE_KM)
    assert_that!(city_name).is_equal_to(None);
}

#[actix_rt::test]
async fn resolve_invalid_lat_out_of_range_returns_none() {
    // GIVEN out-of-range latitude
    let geo_location = GeoLocation {
        latitude: 91.0,
        longitude: 0.0,
    };
    let city_name = geo_location::resolve_city_name(geo_location).await;
    assert_that!(city_name).is_equal_to(None);

    // GIVEN out-of-range longitude
    let geo_location = GeoLocation {
        latitude: 0.0,
        longitude: 181.0,
    };
    let city_name = geo_location::resolve_city_name(geo_location).await;
    assert_that!(city_name).is_equal_to(None);
}
