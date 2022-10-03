use chrono::{NaiveDate, NaiveDateTime};
use exif::{Exif, In, Tag};
use pavao::SmbClient;

use crate::{AppConfig, filesystem_client, geo_location, samba_client};
use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;
use crate::resource_reader::{RemoteResource, RemoteResourceType};

/// Reads the exif date from a given exif data entry
/// Primarily the exif date is used to determine the date the image was taken
/// If the exif date is not available, the gps date is used
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

/// Reads the gps date from a given exif data entry
/// The gps date is used to determine the date the image was taken
fn get_gps_date(exif_data: &Exif) -> Option<NaiveDateTime> {
    exif_data
        .get_field(Tag::GPSDateStamp, In::PRIMARY)
        .map(|gps_date| {
            NaiveDate::parse_from_str(gps_date.display_value().to_string().as_str(), "%F").unwrap()
        })
        .map(|gps_date| gps_date.and_hms(0, 0, 0))
}

/// Finds the exif date in for the given tags
/// Returns the first date found or None if no date was found
fn detect_exif_date(tags_to_evaluate: Vec<Tag>, exif_data: &Exif) -> Option<NaiveDateTime> {
    let exit_dates: Vec<NaiveDateTime> = tags_to_evaluate
        .iter()
        .filter_map(|tag| exif_data.get_field(*tag, In::PRIMARY))
        .filter_map(|exif_date| parse_exif_date(exif_date.display_value().to_string()))
        .collect();

    if !exit_dates.is_empty() {
        Some(*exit_dates.first().unwrap())
    } else {
        None
    }
}

/// Parses the exif date from a given string
fn parse_exif_date(date: String) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(date.as_str(), "%F %T").ok()
}

/// Reads the exif data from a given resource
pub fn load_exif(resource: &RemoteResource, smb_client: &SmbClient) -> Option<Exif> {
    match resource.resource_type {
        RemoteResourceType::Samba => {
            samba_client::read_exif(resource, smb_client)
        }
        RemoteResourceType::Local => {
            filesystem_client::read_exif(&resource.path)
        }
    }
}

/// Augments the provided resource with meta information
/// The meta information is extracted from the exif data
/// If the exif data is not available, the meta information is extracted from the gps data
/// If the gps data is not available, the meta information is extracted from the file name
pub fn fill_exif_data(resource: &RemoteResource, smb_client: SmbClient) -> RemoteResource {
    let mut augmented_resource = resource.clone();

    let maybe_exif_data = load_exif(resource, smb_client);

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

/// Detects the location from the exif data
/// If the location is not found, the location is set to None
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

/// Detects the orientation from the exif data
/// If the orientation is not found, the orientation is set to None
/// Possible rotations are: 0, 90, 180, 270
fn detect_orientation(exif_data: &Exif) -> Option<ImageOrientation> {
    let maybe_orientation = exif_data
        .get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|field| field.value.get_uint(0));

    match maybe_orientation {
        Some(1) => Some(ImageOrientation {
            rotation: 0,
            mirror_vertically: false,
        }),
        Some(2) => Some(ImageOrientation {
            rotation: 0,
            mirror_vertically: true,
        }),
        Some(3) => Some(ImageOrientation {
            rotation: 180,
            mirror_vertically: false,
        }),
        Some(4) => Some(ImageOrientation {
            rotation: 180,
            mirror_vertically: true,
        }),
        Some(5) => Some(ImageOrientation {
            rotation: 90,
            mirror_vertically: true,
        }),
        Some(6) => Some(ImageOrientation {
            rotation: 90,
            mirror_vertically: false,
        }),
        Some(7) => Some(ImageOrientation {
            rotation: 270,
            mirror_vertically: true,
        }),
        Some(8) => Some(ImageOrientation {
            rotation: 270,
            mirror_vertically: false,
        }),
        _ => None,
    }
}

/// Detects the date from the file name
/// If the date is not found, the date is set to None
/// The chars '/', ' ', '.', '_' are replaced with '_'
fn detect_date_by_name(resource_path: &str) -> Option<NaiveDateTime> {
    let parsed: Vec<NaiveDate> = resource_path
        .replace('/', "_")
        .replace(' ', "_")
        .replace('.', "_")
        .split('_')
        .into_iter()
        .filter_map(parse_from_str)
        .collect();

    if parsed.is_empty() {
        None
    } else {
        Some(parsed.first().unwrap().and_hms(0, 0, 0))
    }
}

/// Parses a string into a date
/// Returns None if the string could not be parsed
fn parse_from_str(shard: &str) -> Option<NaiveDate> {
    // https://docs.rs/chrono/latest/chrono/format/strftime/index.html
    let parse_results: Vec<NaiveDate> = vec![
        "%F",     // 2001-07-08
        "%Y%m%d", // 20010708
        "signal-%Y-%m-%d-%Z",
    ]
        .iter()
        .filter_map(|format| NaiveDate::parse_from_str(shard, format).ok())
        .collect();

    if parse_results.is_empty() {
        None
    } else {
        Some(*parse_results.first().unwrap())
    }
}
