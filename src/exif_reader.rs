use std::io::{BufReader, Cursor, Read};

use chrono::{NaiveDate, NaiveDateTime};
use exif::{Exif, In, Reader, Tag};

use crate::{geo_location, WebDavClient, WebDavResource};
use crate::geo_location::GeoLocation;

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
    let result = NaiveDateTime::parse_from_str(date.as_str(), "%F %T");

    if result.is_err() {
        println!("broken date format: {date}");
        return None;
    };

    Some(result.unwrap())
}

pub fn load_exif(web_dav_client: &WebDavClient, resource: &WebDavResource) -> Option<Exif> {
    // Build the resource url and request data pointer
    let resource_url = format!("{}{}", web_dav_client.base_url, &resource.path);
    let mut response = web_dav_client.request_resource_data(resource_url);

    // If there is no content-length there might be something broken, exit here.
    let content_length = response.content_length();

    // Return none if not present
    content_length?;

    // Just read the very first bytes of the resource that very likely contains the exif data
    let buf_length = content_length.unwrap() as f32 * 0.004;
    let mut data_buffer = vec![0; buf_length as usize];
    response.read_exact(&mut data_buffer).unwrap();
    let mut buf_reader = BufReader::new(
        Cursor::new(data_buffer)
    );

    // Read the exif metadata
    let maybe_exif = Reader::new().read_from_container(&mut buf_reader);

    maybe_exif.ok()
}

pub fn fill_exif_data(web_dav_client: &WebDavClient, resource: &WebDavResource) -> WebDavResource {
    let mut augmented_resource = resource.clone();

    let maybe_exif_data = load_exif(web_dav_client, resource);

    let mut taken_date = None;
    let mut location = None;

    if let Some(exif_data) = maybe_exif_data {
        taken_date = get_exif_date(&exif_data);
        location = detect_location(&exif_data);

        if location.is_none() {
            println!("No location found for: {}", &resource.path)
        }
    }

    if taken_date.is_none() {
        taken_date = detect_date_by_name(&resource.path);
    }

    augmented_resource.taken = taken_date;
    augmented_resource.location = location;

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