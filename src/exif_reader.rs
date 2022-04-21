use chrono::{NaiveDate, NaiveDateTime};
use exif::{Exif, In, Reader, Tag};

use crate::geo_location;
use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;
use crate::resource_reader::{RemoteResource, ResourceReader};

pub fn get_exif_date(exif_data: &Exif) -> Option<NaiveDateTime> {
    let mut exif_date: Option<NaiveDateTime> = detect_exif_date(
        vec![Tag::DateTime, Tag::DateTimeOriginal, Tag::DateTimeDigitized],
        exif_data,
    );

    if exif_date.is_none() {
        exif_date = get_gps_date(exif_data);
    };

    exif_date
}

fn get_gps_date(exif_data: &Exif) -> Option<NaiveDateTime> {
    exif_data.get_field(Tag::GPSDateStamp, In::PRIMARY)
        .map(|gps_date| NaiveDate::parse_from_str(
            gps_date.display_value().to_string().as_str(),
            "%F",
        ).unwrap())
        .map(|gps_date| gps_date.and_hms(0, 0, 0))
}

fn detect_exif_date(tags_to_evaluate: Vec<Tag>, exif_data: &Exif) -> Option<NaiveDateTime> {
    let exit_dates: Vec<NaiveDateTime> = tags_to_evaluate.iter()
        .filter_map(|tag| exif_data.get_field(*tag, In::PRIMARY))
        .filter_map(|exif_date| parse_exit_date(exif_date.display_value().to_string()))
        .collect();

    if !exit_dates.is_empty() {
        Some(*exit_dates.first().unwrap())
    } else {
        None
    }
}

fn parse_exit_date(date: String) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(date.as_str(), "%F %T").ok()
}

pub fn load_exif(web_dav_client: &ResourceReader, resource: &RemoteResource) -> Option<Exif> {
    // Build the resource url and request resource data response
    let mut response = web_dav_client.request_resource_data(resource);

    // Read the exif metadata
    Reader::new().from_reader(&mut response).ok()
}

pub fn fill_exif_data(web_dav_client: &ResourceReader, resource: &RemoteResource) -> RemoteResource {
    let mut augmented_resource = resource.clone();

    let maybe_exif_data = load_exif(web_dav_client, resource);

    let mut taken_date = None;
    let mut location = None;
    let mut orientation = None;

    if let Some(exif_data) = maybe_exif_data {
        taken_date = get_exif_date(&exif_data);
        location = detect_location(&exif_data);
        orientation = detect_orientation(&exif_data);
    }

    if taken_date.is_none() {
        taken_date = detect_date_by_name(&resource.path);
    }

    augmented_resource.taken = taken_date;
    augmented_resource.location = location;
    augmented_resource.orientation = orientation;

    augmented_resource
}

fn detect_location(exif_data: &Exif) -> Option<GeoLocation> {
    let maybe_latitude = exif_data.get_field(Tag::GPSLatitude, In::PRIMARY);
    let maybe_longitude = exif_data.get_field(Tag::GPSLongitude, In::PRIMARY);

    if let (Some(latitude), Some(longitude)) = (maybe_latitude, maybe_longitude) {
        return geo_location::from_degrees_minutes_seconds(
            latitude.display_value().to_string(),
            longitude.display_value().to_string(),
        );
    }

    None
}

fn detect_orientation(exif_data: &Exif) -> Option<ImageOrientation> {
    let maybe_orientation = exif_data.get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|field| field.value.get_uint(0));

    match maybe_orientation {
        Some(1) => Some(ImageOrientation { rotation: 0, mirror_vertically: false }),
        Some(2) => Some(ImageOrientation { rotation: 0, mirror_vertically: true }),
        Some(3) => Some(ImageOrientation { rotation: 180, mirror_vertically: false }),
        Some(4) => Some(ImageOrientation { rotation: 180, mirror_vertically: true }),
        Some(5) => Some(ImageOrientation { rotation: 90, mirror_vertically: true }),
        Some(6) => Some(ImageOrientation { rotation: 90, mirror_vertically: false }),
        Some(7) => Some(ImageOrientation { rotation: 270, mirror_vertically: true }),
        Some(8) => Some(ImageOrientation { rotation: 270, mirror_vertically: false }),
        _ => None,
    }
}

fn detect_date_by_name(resource_path: &str) -> Option<NaiveDateTime> {
    let parsed: Vec<NaiveDate> = resource_path
        .replace('/', "_")
        .replace(' ', "_")
        .split('_')
        .into_iter()
        .filter_map(parse_from_str)
        .collect();

    if parsed.is_empty() {
        None
    } else {
        Some(
            parsed.first().unwrap()
                .and_hms(0, 0, 0)
        )
    }
}

fn parse_from_str(shard: &str) -> Option<NaiveDate> {
    // https://docs.rs/chrono/latest/chrono/format/strftime/index.html
    let parse_results: Vec<NaiveDate> = vec![
        "%F", // 2001-07-08
        "%Y%m%d", // 20010708
        "signal-%Y-%m-%d-%Z",
    ].iter()
        .filter_map(|format| NaiveDate::parse_from_str(shard, format).ok())
        .collect();

    if parse_results.is_empty() {
        None
    } else {
        Some(*parse_results.first().unwrap())
    }
}