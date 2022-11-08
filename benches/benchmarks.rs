use chrono::{NaiveDate, NaiveDateTime};
use regex::{Captures, Regex};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use exif::{Exif, In, Tag};
use lazy_static::lazy_static;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use rand::Rng;
use serde::{Deserialize, Serialize};

const TEST_JPEG_EXIF_URL: &str = "https://live.staticflickr.com/5333/8949667935_16576dc54b_o_d.jpg";

fn criterion_benchmark(crit: &mut Criterion) {
    let base_dir = create_base_dir();
    let base_dir_str = base_dir.as_path().to_str().unwrap();
    let image_file = download_image(base_dir_str);
    let imag_file_path = PathBuf::from_str(image_file.as_str()).unwrap();

    // Create persistent file store
    let persistent_file_store_pool =
        Pool::new(SqliteConnectionManager::file(&base_dir.join("test.db")))
            .expect("persistent storage pool creation failed");
    // Create in memory store
    let in_memory_pool =
        Pool::new(SqliteConnectionManager::memory()).expect("In memory pool creation failed");
    create_table_test(&persistent_file_store_pool);
    create_table_test(&in_memory_pool);

    let resource = read_resource(&imag_file_path);

    crit.bench_function("write to memory", |bencher| {
        bencher.iter(|| write_to_memory(black_box(in_memory_pool.clone()), black_box(&resource)))
    });
    crit.bench_function("write to fs", |bencher| {
        bencher.iter(|| {
            write_to_fs(
                black_box(persistent_file_store_pool.clone()),
                black_box(&resource),
            )
        })
    });

    crit.bench_function("hash file on fs", |bencher| {
        bencher.iter(|| hash_file_on_fs(black_box(resource.clone()), black_box(&image_file)))
    });
    crit.bench_function("read resource on fs", |bencher| {
        bencher.iter(|| read_resource(black_box(&imag_file_path)))
    });

    fs::remove_dir_all(base_dir).expect("Cleanup failed");
}

fn write_to_memory(pool: Pool<SqliteConnectionManager>, resource: &RemoteResource) {
    let connection = pool.get().unwrap();
    let mut stmt = connection
        .prepare("INSERT OR REPLACE INTO test(id, value) VALUES(?, ?)")
        .unwrap();
    stmt.execute((&resource.id, serde_json::to_string(&resource).unwrap()))
        .unwrap_or_else(|_| panic!("Insertion of failed"));
}

fn write_to_fs(pool: Pool<SqliteConnectionManager>, resource: &RemoteResource) {
    let connection = pool.get().unwrap();
    let mut stmt = connection
        .prepare("INSERT OR REPLACE INTO test(id, value) VALUES(?, ?)")
        .unwrap();
    stmt.execute((&resource.id, serde_json::to_string(&resource).unwrap()))
        .unwrap_or_else(|_| panic!("Insertion of failed"));
}

fn create_table_test(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS test (id TEXT PRIMARY KEY, value TEXT);",
            (),
        )
        .expect("table creation of 'test' failed");
}

fn hash_file_on_fs(resource: RemoteResource, image_file: &str) {
    let image_data = fs::read(image_file);
    let _ = md5(image_data.unwrap());
    let _ = fill_exif_data(&resource);
}

fn create_base_dir() -> PathBuf {
    let random_string = rand::thread_rng().gen::<u32>().to_string();
    let test_dir: PathBuf = env::temp_dir().join(&random_string);
    if !test_dir.exists() {
        fs::create_dir_all(&test_dir).unwrap();
    }
    test_dir
}

fn download_image(base_dir: &str) -> String {
    let response = ureq::get(TEST_JPEG_EXIF_URL).call().unwrap();

    let len: usize = response.header("Content-Length").unwrap().parse().unwrap();

    let mut data: Vec<u8> = Vec::with_capacity(len);
    response
        .into_reader()
        .read_to_end(&mut data)
        .expect("write fail");

    let file = PathBuf::from_str(base_dir).unwrap().join("test.jpg");
    let file_str = file.to_str();

    fs::write(&file, data).expect("TODO: panic message");

    file_str.unwrap().to_string()
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

/// Returns a md5 string based on a given string
pub fn md5(data: Vec<u8>) -> String {
    format!("{:x}", md5::compute(data))
}

/// Reads a single file and returns the found resource
/// Checks if the file is a supported resource currently all image types
fn read_resource(file_path: &PathBuf) -> RemoteResource {
    let file = fs::File::open(file_path).unwrap();
    let metadata = file.metadata().expect("Failed to read metadata");
    let file_name = file_path.as_path().file_name().unwrap().to_str().unwrap();

    let is_file = metadata.is_file();
    let mime_type: &str = mime_guess::from_path(file_name).first_raw().unwrap_or("");

    // Cancel if no image file
    if !is_file || !mime_type.starts_with("image/") {
        return RemoteResource {
            id: "".to_string(),
            path: "".to_string(),
            content_type: "".to_string(),
            name: "".to_string(),
            content_length: 0,
            last_modified: Default::default(),
            taken: None,
            location: None,
            orientation: None,
            resource_type: RemoteResourceType::Local,
            samba_client_index: 0,
        };
    }

    RemoteResource {
        id: md5(Vec::from(file_name.as_bytes())),
        path: file_path.to_str().unwrap().to_string(),
        content_type: mime_type.to_string(),
        name: file_name.to_string(),
        content_length: metadata.len(),
        last_modified: to_date_time(metadata.modified().unwrap()),
        taken: None,
        location: None,
        orientation: None,
        resource_type: RemoteResourceType::Local,
        samba_client_index: 0,
    }
}

/// A remote resource that is available on the filesystem
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteResource {
    pub id: String,
    pub path: String,
    pub content_type: String,
    pub name: String,
    pub content_length: u64,
    pub last_modified: NaiveDateTime,
    pub taken: Option<NaiveDateTime>,
    pub location: Option<GeoLocation>,
    pub orientation: Option<ImageOrientation>,
    pub resource_type: RemoteResourceType,
    pub samba_client_index: usize,
}

/// Represents the type of an resource, either on the local computer or a samba resource.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RemoteResourceType {
    Local,
    Samba,
}

/// Represents the orientation of an image in two dimensions
/// rotation:               0, 90, 180 or 270
/// mirror_vertically:      true, if the image is mirrored vertically
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub struct ImageOrientation {
    pub rotation: u16,
    pub mirror_vertically: bool,
}

/// Struct representing a geo location
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub struct GeoLocation {
    pub latitude: f32,
    pub longitude: f32,
}

/// Converts the type `SystemTime` to `NaiveDateTime`
pub fn to_date_time(system_time: SystemTime) -> NaiveDateTime {
    NaiveDateTime::from_timestamp(
        system_time
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::new(0, 0))
            .as_secs() as i64,
        0,
    )
}

/// Reads the exif data from the file and augments the remote resource with this information
pub fn fill_exif_data(resource: &RemoteResource) -> RemoteResource {
    let file_path = resource.path.as_str();
    let file = std::fs::File::open(file_path).unwrap();

    let mut bufreader = std::io::BufReader::new(&file);
    let exif_reader = exif::Reader::new();
    let maybe_exif_data = exif_reader.read_from_container(&mut bufreader).ok();

    fill_exif_data_fs(resource, maybe_exif_data)
}

/// Augments the provided resource with meta information
/// The meta information is extracted from the exif data
/// If the exif data is not available, the meta information is extracted from the gps data
/// If the gps data is not available, the meta information is extracted from the file name
pub fn fill_exif_data_fs(
    resource: &RemoteResource,
    maybe_exif_data: Option<Exif>,
) -> RemoteResource {
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

    let mut augmented_resource = resource.clone();
    augmented_resource.taken = taken_date;
    augmented_resource.location = location;
    augmented_resource.orientation = orientation;

    augmented_resource
}

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

/// Detects the location from the exif data
/// If the location is not found, the location is set to None
pub fn detect_location(exif_data: &Exif) -> Option<GeoLocation> {
    let maybe_latitude = exif_data.get_field(Tag::GPSLatitude, In::PRIMARY);
    let maybe_longitude = exif_data.get_field(Tag::GPSLongitude, In::PRIMARY);

    if let (Some(latitude), Some(longitude)) = (maybe_latitude, maybe_longitude) {
        return from_degrees_minutes_seconds(
            latitude.display_value().to_string(),
            longitude.display_value().to_string(),
        );
    }

    None
}

/// Converts latitude and longitude to a GeoLocation
/// If the latitude or longitude is not valid, None is returned
/// This is done by converting the latitude and longitude to degrees minutes seconds
pub fn from_degrees_minutes_seconds(latitude: String, longitude: String) -> Option<GeoLocation> {
    let maybe_dd_lat = dms_to_dd(&latitude);
    let maybe_dd_lon = dms_to_dd(&longitude);

    if let (Some(latitude), Some(longitude)) = (maybe_dd_lat, maybe_dd_lon) {
        Some(GeoLocation {
            latitude,
            longitude,
        })
    } else {
        None
    }
}

/// Converts Degrees Minutes Seconds To Decimal Degrees
/// See https://stackoverflow.com/questions/14906764/converting-gps-coordinates-to-decimal-degrees
fn dms_to_dd(dms_string: &str) -> Option<f32> {
    lazy_static! {
        static ref DMS_PARSE_PATTERN_1: Regex = Regex::new(
            // e.g.: 7 deg 33 min 55.5155 sec
            r"(?P<deg>\d+) deg (?P<min>\d+) min (?P<sec>\d+.\d+) sec"
        ).unwrap();
        static ref DMS_PARSE_PATTERN_2: Regex = Regex::new(
            // e.g.: 50/1, 25/1, 2519/100
            r"(?P<deg>\d+)/(?P<deg_fraction>\d+),\s*(?P<min>\d+)/(?P<min_fraction>\d+),\s*(?P<sec>\d+)/(?P<sec_fraction>\d+)"
        ).unwrap();
    }

    let dms_pattern_1_match: Option<Captures> = DMS_PARSE_PATTERN_1.captures(dms_string);
    let dms_pattern_2_match: Option<Captures> = DMS_PARSE_PATTERN_2.captures(dms_string);

    if let Some(pattern_match) = dms_pattern_1_match {
        parse_pattern_1(pattern_match)
    } else if let Some(pattern_match) = dms_pattern_2_match {
        parse_pattern_2(pattern_match)
    } else {
        None
    }
}

/// Parses Degrees minutes seconds for the following example pattern: "7 deg 33 min 55.5155 sec"
fn parse_pattern_1(caps: Captures) -> Option<f32> {
    let maybe_deg: Option<f32> = caps
        .name("deg")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_min: Option<f32> = caps
        .name("min")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_sec: Option<f32> = caps
        .name("sec")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());

    if let (Some(deg), Some(min), Some(sec)) = (maybe_deg, maybe_min, maybe_sec) {
        Some(deg + (min / 60.0) + (sec / 3600.0))
    } else {
        None
    }
}

/// Parses Degrees minutes seconds for the following example pattern: "50/1, 25/1, 2519/100"
fn parse_pattern_2(caps: Captures) -> Option<f32> {
    let maybe_deg: Option<f32> = caps
        .name("deg")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_deg_fraction: Option<f32> = caps
        .name("deg_fraction")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_min: Option<f32> = caps
        .name("min")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_min_fraction: Option<f32> = caps
        .name("min_fraction")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_sec: Option<f32> = caps
        .name("sec")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());
    let maybe_sec_fraction: Option<f32> = caps
        .name("sec_fraction")
        .map(|cap| cap.as_str().parse::<f32>().unwrap());

    if let (Some(deg), Some(deg_frac), Some(min), Some(min_frac), Some(sec), Some(sec_frac)) = (
        maybe_deg,
        maybe_deg_fraction,
        maybe_min,
        maybe_min_fraction,
        maybe_sec,
        maybe_sec_fraction,
    ) {
        Some((deg / deg_frac) + ((min / min_frac) / 60.0) + ((sec / sec_frac) / 3600.0))
    } else {
        None
    }
}

/// Detects the orientation from the exif data
/// If the orientation is not found, the orientation is set to None
/// Possible rotations are: 0, 90, 180, 270
pub fn detect_orientation(exif_data: &Exif) -> Option<ImageOrientation> {
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
pub fn detect_date_by_name(resource_path: &str) -> Option<NaiveDateTime> {
    let parsed: Vec<NaiveDate> = resource_path
        .replace(['/', ' ', '.'], "_")
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
