use std::io::{BufReader, Cursor};

use chrono::{NaiveDate, NaiveDateTime};
use exif::{Exif, In, Reader, Tag};

use crate::{WebDavClient, WebDavResource};

pub fn get_exif_date(exif_data: Exif, path: &String) -> Option<NaiveDateTime> {
    let mut exif_date: Option<NaiveDateTime> = detect_exif_date(
        vec![Tag::DateTime, Tag::DateTimeOriginal, Tag::DateTimeDigitized],
        &exif_data,
    );

    if exif_date.is_none() {
        exif_date = get_gps_date(&exif_data);
    };

    if exif_date.is_none() {
        return None;
    };

    return exif_date;
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

    if exit_dates.len() > 0 {
        Some(exit_dates.first().unwrap().clone())
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

    return Some(result.unwrap());
}

pub fn load_exif(resource_data: Vec<u8>) -> Option<Exif> {
    let cursor = Cursor::new(resource_data);
    let mut buf_reader = BufReader::new(cursor);
    let maybe_exif = Reader::new()
        .read_from_container(&mut buf_reader);
    maybe_exif.ok()
}

pub fn fill_exif_data(web_dav_client: &WebDavClient, resource: &WebDavResource) -> WebDavResource {
    let mut augmented_resource = resource.clone();

    let resource_url = web_dav_client.build_resource_url(&resource.path);
    let photo_data = web_dav_client.request_resource_data(resource_url);

    let exif_data = load_exif(photo_data);

    let taken_date: Option<NaiveDateTime> = detect_taken_date(exif_data, &resource.path);

    if taken_date.is_none() {
        println!("no date found for: {}", &resource.path)
    }

    augmented_resource.taken = taken_date;

    return augmented_resource;
}

fn detect_taken_date(exif_data: Option<Exif>, resource_path: &String) -> Option<NaiveDateTime> {
    let mut taken_date = None;

    if exif_data.is_some() {
        taken_date = get_exif_date(exif_data.unwrap(), resource_path);
    }

    if taken_date.is_none() {
        taken_date = detect_date_by_name(resource_path);
    }

    return taken_date;
}

fn detect_date_by_name(resource_path: &String) -> Option<NaiveDateTime> {
    let parsed: Vec<NaiveDate> = resource_path
        .replace("/", "_")
        .replace(" ", "_")
        .split('_')
        .into_iter()
        .filter_map(|shard| parse_from_str(shard))
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