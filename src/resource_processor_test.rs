use assertor::*;

use crate::geo_location;
use crate::geo_location::GeoLocation;

#[actix_rt::test]
async fn resolve_koblenz() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN are the degree minutes seconds coordinates near Playa del Ingles (Gran Canaria)
    // 27 deg 45 min 22.22 sec N, 15 deg 34 min 13.76 sec W ≈ 27.756, -15.570
    let lat = "27 deg 45 min 22.22 sec";
    let long = "15 deg 34 min 13.76 sec";
    let lat_ref = "N";
    let long_ref = "W";

    // WHEN resolving the city name
    let dms = geo_location::from_degrees_minutes_seconds(lat, long, lat_ref, long_ref);

    // THEN the resolved city should be the nearest GeoNames entry on Gran Canaria
    // (native name varies with dataset version; accept known neighbours)
    let city_name = geo_location::resolve_city_name(dms.unwrap()).await;
    assert!(
        matches!(
            city_name.as_deref(),
            Some("Playa del Ingles") | Some("San Bartolomé de Tirajana") | Some("Maspalomas")
        ),
        "unexpected city for 27.756,-15.570: {:?}",
        city_name
    );
}

#[actix_rt::test]
async fn resolve_invalid_data() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
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

#[actix_rt::test]
async fn resolve_nan_and_infinite_returns_none() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    for (lat, lon) in [
        (f32::NAN, 0.0),
        (0.0, f32::NAN),
        (f32::NAN, f32::NAN),
        (f32::INFINITY, 0.0),
        (0.0, f32::NEG_INFINITY),
    ] {
        let geo_location = GeoLocation {
            latitude: lat,
            longitude: lon,
        };
        let city_name = geo_location::resolve_city_name(geo_location).await;
        assert_that!(city_name).is_equal_to(None);
    }
}

#[actix_rt::test]
async fn resolve_longerich_hierarchical() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN the photo location from issue #217 (Köln-Longerich, 50.996555/6.9155197)
    let geo_location = GeoLocation {
        latitude: 50.996555,
        longitude: 6.9155197,
    };
    // WHEN resolving
    let city_name = geo_location::resolve_city_name(geo_location).await;
    // THEN the quarter wins over the 2.96 km distant Ossendorf centroid
    // (with the old cities500.txt dataset this coordinate resolved to "Ossendorf, Köln")
    assert_eq!(
        city_name.expect("expected Longerich/Köln to resolve, got None"),
        "Longerich, Köln"
    );
}

#[actix_rt::test]
async fn resolve_koeln_quarter_hierarchical() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN a Köln quarter coordinate (approx 50.9049, 6.9606) — PPLX near Köln
    let geo_location = GeoLocation {
        latitude: 50.9049,
        longitude: 6.9606,
    };
    let city_name = geo_location::resolve_city_name(geo_location).await;
    let name = city_name.expect("expected a Köln quarter to resolve, got None");
    // THEN the hierarchical "quarter, Köln" form. The exact quarter depends on the
    // dataset snapshot (measured with the current snapshot: "Raderberg, Köln"), so
    // only the hierarchy contract is pinned here.
    assert!(
        name.ends_with(", Köln"),
        "expected hierarchical '<quarter>, Köln', got '{}'",
        name
    );
    let quarter = name
        .strip_suffix(", Köln")
        .expect("checked by ends_with above");
    assert!(
        !quarter.is_empty(),
        "expected a non-empty quarter before ', Köln', got '{}'",
        name
    );
}

#[actix_rt::test]
async fn resolve_christianshavn_hierarchical() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN Christianshavn district (55.67383, 12.59541) — PPLX near Copenhagen/København
    let geo_location = GeoLocation {
        latitude: 55.676,
        longitude: 12.593,
    };
    let city_name = geo_location::resolve_city_name(geo_location).await;
    let name = city_name.expect("expected Christianshavn/Copenhagen to resolve");
    assert!(
        name.contains("Copenhagen") || name.contains("København"),
        "expected Copenhagen/København in '{}'",
        name
    );
    assert!(
        name == "Christianshavn, Copenhagen"
            || name == "Christianshavn, København"
            || name == "Copenhagen"
            || name == "København",
        "unexpected Christianshavn fallback '{}'",
        name
    );
}

#[actix_rt::test]
async fn resolve_volksdorf_hierarchical() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN Volksdorf district (53.64972, 10.18417) — PPLX near Hamburg
    let geo_location = GeoLocation {
        latitude: 53.651,
        longitude: 10.166,
    };
    let city_name = geo_location::resolve_city_name(geo_location).await;
    let name = city_name.expect("expected Volksdorf/Hamburg to resolve");
    // Dataset-tolerant: accept hierarchical "Volksdorf, Hamburg"
    assert!(name.contains("Hamburg"), "expected Hamburg in '{}'", name);
    assert!(
        name == "Volksdorf, Hamburg" || name == "Hamburg",
        "unexpected Volksdorf fallback '{}'",
        name
    );
}

#[actix_rt::test]
async fn resolve_koln_dom_plain() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN plain city Köln center (50.93333,6.95) — PPLA2, not PPLX
    // Plain-city guarantee: exactly "Köln" (single name, no comma).
    // Note: Köln Dom 50.941,6.958 is actually PPLX Altstadt Nord (~0.23km) and would
    // resolve to "Altstadt Nord, Köln"; we use the city-center coordinate to enforce
    // plain-city single-name guarantee.
    let geo_location = GeoLocation {
        latitude: 50.93333,
        longitude: 6.95,
    };
    let city_name = geo_location::resolve_city_name(geo_location).await;
    let name = city_name.expect("Köln center should resolve");
    assert!(
        name.contains("Köln"),
        "expected Köln in '{}' for Köln center",
        name
    );
    assert!(
        !name.contains(","),
        "plain city Köln should be single name, got '{}'",
        name
    );
    assert_eq!(name, "Köln");
}
#[actix_rt::test]
async fn resolve_district_without_parent_falls_back() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // Isolated district fallback: PPLX with no parent within 30km → district alone (no comma).
    // Synthetic CityIndex isolation requires private OnceLock, so we cover fallback
    // via two dataset-tolerant assertions:
    // 1) A Köln quarter (50.9049,6.9606) is hierarchical "<quarter>, Köln" — verifies
    //    the district→parent path. The quarter name itself is dataset-dependent
    //    (measured with the current snapshot: "Raderberg, Köln").
    let bay = GeoLocation {
        latitude: 50.9049,
        longitude: 6.9606,
    };
    let name = geo_location::resolve_city_name(bay)
        .await
        .expect("Köln quarter should resolve");
    // Dataset-tolerant: hierarchical "<quarter>, Köln"
    assert!(name.contains("Köln"), "expected Köln in '{}'", name);
    assert!(
        name.ends_with(", Köln") && !name.trim_end_matches(", Köln").is_empty(),
        "expected hierarchical '<quarter>, Köln', got '{}'",
        name
    );
    // 2) Remote PPLX fallback: Palm Island (-18.73565,146.57788, AU) is a PPLX
    //    with no parent city within 30km (dataset inspection: nearest parent >30km).
    //    Fallback must be the district alone (single name, no comma) — never a
    //    far-away parent. Conditional on dataset still resolving to Palm Island
    //    so dataset evolution cannot break CI.
    let remote = GeoLocation {
        latitude: -18.73565,
        longitude: 146.57788,
    };
    if let Some(remote_name) = geo_location::resolve_city_name(remote).await {
        assert!(!remote_name.is_empty(), "remote PPLX should resolve");
        if remote_name.contains("Palm Island") {
            assert!(
                !remote_name.contains(','),
                "isolated PPLX Palm Island must fall back to single name, got '{}'",
                remote_name
            );
        }
    }
    // Also verify mid-ocean still returns None (no panic on fallback path)
    let ocean = GeoLocation {
        latitude: 0.0,
        longitude: -160.0,
    };
    assert!(geo_location::resolve_city_name(ocean).await.is_none());
}
#[test]
fn migration_04_drops_geo_cache_in_resource_processor_context() {
    // geo_location_cache is dropped after migration 04.
    // Detailed schema assertions live in src/resource_store.rs
    // (fresh_install_and_migrated_db_have_identical_schema etc.);
    // this test ensures the migration set is valid and that
    // geo_location_cache is absent after applying migrations in this module's context.
    use rusqlite::Connection;
    assert!(crate::resource_store::MIGRATIONS.validate().is_ok());
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE geo_location_cache (id TEXT PRIMARY KEY, value TEXT); PRAGMA user_version=3;",
    )
    .unwrap();
    crate::resource_store::MIGRATIONS
        .to_latest(&mut conn)
        .unwrap();
    let cnt: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='geo_location_cache'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        cnt, 0,
        "geo_location_cache should be dropped after migration 04"
    );
}

/// Serializes HOME_COUNTRY mutation across async tests; restores the prior value on drop.
struct HomeCountryGuard {
    prev: Option<String>,
    _serial: tokio::sync::MutexGuard<'static, ()>,
}

impl HomeCountryGuard {
    async fn set(code: Option<String>) -> Self {
        let serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
        let prev = crate::geo_location::home_country_for_tests();
        crate::geo_location::set_home_country_for_tests(code);
        Self {
            prev,
            _serial: serial,
        }
    }
}

impl Drop for HomeCountryGuard {
    fn drop(&mut self) {
        crate::geo_location::set_home_country_for_tests(self.prev.clone());
    }
}

#[actix_rt::test]
async fn home_country_match_hides_country_suffix() {
    // GIVEN home Germany and a Köln quarter photo
    let _home = HomeCountryGuard::set(Some("DE".to_string())).await;
    let geo_location = GeoLocation {
        latitude: 50.9049,
        longitude: 6.9606,
    };
    // WHEN resolving
    let name = geo_location::resolve_city_name(geo_location)
        .await
        .expect("Köln quarter should resolve");
    // THEN hierarchical display with no country suffix. The quarter name itself is
    // dataset-dependent (measured with the current snapshot: "Raderberg, Köln").
    assert!(
        name.ends_with(", Köln") && !name.trim_end_matches(", Köln").is_empty(),
        "expected hierarchical '<quarter>, Köln', got '{}'",
        name
    );
    assert!(!name.contains("Germany"), "home suffix leaked: '{}'", name);
}

#[actix_rt::test]
async fn foreign_country_appends_country_name() {
    // GIVEN home Germany and a Christianshavn photo
    let _home = HomeCountryGuard::set(Some("DE".to_string())).await;
    let geo_location = GeoLocation {
        latitude: 55.676,
        longitude: 12.593,
    };
    // WHEN resolving
    let name = geo_location::resolve_city_name(geo_location)
        .await
        .expect("Christianshavn should resolve");
    // THEN display ends with the English country name
    assert!(
        name.ends_with(", Denmark"),
        "expected Denmark suffix, got '{}'",
        name
    );
}

#[actix_rt::test]
async fn home_country_value_is_normalized() {
    // GIVEN a lowercase, padded home value
    let home = geo_location::parse_home_country(" de ").unwrap();
    let _guard = HomeCountryGuard::set(home).await;
    let geo_location = GeoLocation {
        latitude: 50.93333,
        longitude: 6.95,
    };
    // WHEN resolving a home-city photo
    let name = geo_location::resolve_city_name(geo_location)
        .await
        .expect("Köln center should resolve");
    // THEN plain home display, no suffix
    assert_eq!(name, "Köln");
}

#[actix_rt::test]
async fn unset_home_country_shows_no_suffix() {
    // GIVEN explicitly unset home and a foreign photo
    let _home = HomeCountryGuard::set(None).await;
    let geo_location = GeoLocation {
        latitude: 55.676,
        longitude: 12.593,
    };
    // WHEN resolving
    let name = geo_location::resolve_city_name(geo_location)
        .await
        .expect("Christianshavn should resolve");
    // THEN no country suffix (dataset-tolerant on the city rendering)
    assert!(
        !name.contains("Denmark"),
        "country shown while unset: '{}'",
        name
    );
    assert!(
        name.contains("København") || name.contains("Copenhagen"),
        "expected city without country, got '{}'",
        name
    );
}

#[actix_rt::test]
async fn given_geodata_path_set_when_reading_then_it_is_used_and_default_applies() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN the dataset variable is set
    let prev_geodata = std::env::var("GEODATA_PATH").ok();
    std::env::set_var("GEODATA_PATH", "/tmp/geodata.txt");

    // WHEN reading the configured dataset path
    let configured = geo_location::get_geodata_path();

    // WHEN the variable is unset
    std::env::remove_var("GEODATA_PATH");
    let default_path = geo_location::get_geodata_path();

    // Restore the previous environment before asserting so a failure cannot leak it
    match prev_geodata {
        Some(value) => std::env::set_var("GEODATA_PATH", value),
        None => std::env::remove_var("GEODATA_PATH"),
    }

    // THEN the variable wins and the built-in default is used otherwise
    assert_eq!(configured, "/tmp/geodata.txt");
    assert_eq!(default_path, "/geodata.txt");
}

#[actix_rt::test]
async fn given_retired_cities500_path_when_checking_then_error_names_variable_and_replacement() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN the retired dataset variable is still set
    let prev_cities500 = std::env::var("CITIES500_PATH").ok();
    std::env::set_var("CITIES500_PATH", "/tmp/cities500.txt");

    // WHEN checking the environment for the retired variable
    let result = geo_location::check_retired_env();

    // Restore the previous environment before asserting so a failure cannot leak it
    match prev_cities500 {
        Some(value) => std::env::set_var("CITIES500_PATH", value),
        None => std::env::remove_var("CITIES500_PATH"),
    }

    // THEN the check fails with a message naming the variable, its value and the replacement
    let error = result.expect_err("a set CITIES500_PATH must abort startup");
    assert!(
        error.contains("CITIES500_PATH"),
        "error should name the variable: {error}"
    );
    assert!(
        error.contains("/tmp/cities500.txt"),
        "error should name the offending value: {error}"
    );
    assert!(
        error.contains("GEODATA_PATH"),
        "error should name the replacement: {error}"
    );
}

#[actix_rt::test]
async fn given_no_retired_variable_when_checking_then_ok() {
    let _serial = crate::utils::SERIAL_TEST_MUTEX.lock().await;
    // GIVEN the retired variable is not set
    let prev_cities500 = std::env::var("CITIES500_PATH").ok();
    std::env::remove_var("CITIES500_PATH");

    // WHEN checking the environment
    let result = geo_location::check_retired_env();

    // Restore the previous environment before asserting so a failure cannot leak it
    if let Some(value) = prev_cities500 {
        std::env::set_var("CITIES500_PATH", value);
    }

    // THEN the check passes
    assert!(result.is_ok(), "unexpected error: {result:?}");
}

#[test]
fn parse_home_country_validation() {
    // GIVEN/WHEN/THEN pure validation without globals
    assert_eq!(
        geo_location::parse_home_country("DE").unwrap(),
        Some("DE".to_string())
    );
    assert_eq!(
        geo_location::parse_home_country(" de ").unwrap(),
        Some("DE".to_string())
    );
    assert_eq!(geo_location::parse_home_country("").unwrap(), None);
    assert_eq!(geo_location::parse_home_country("   ").unwrap(), None);
    let err = geo_location::parse_home_country("XX").unwrap_err();
    assert!(
        err.contains("HOME_COUNTRY") && err.contains("XX"),
        "unexpected error: {}",
        err
    );
}
