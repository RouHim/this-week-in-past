use std::fmt::{Display, Formatter};

use chrono::{Local, NaiveDateTime, TimeZone};
use now::DateTimeNow;
use serde::{Serialize, Deserialize};

use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;

#[derive(Clone)]
pub struct ResourceReader {
    pub paths: Vec<String>,
}

impl ResourceReader {
    pub fn request_resource_data(&self, resource: &RemoteResource) -> String {
        todo!()
    }

    pub fn list_all_resources(&self) -> Vec<RemoteResource> {
        todo!()
    }
}

pub fn new(paths: &str) -> ResourceReader {
    ResourceReader {
        paths: paths.split(',')
            .map(|s| s.to_string())
            .collect(),
    }
}


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
}

impl RemoteResource {
    pub fn is_this_week(&self) -> bool {
        if self.taken.is_none() {
            return false;
        }

        let current_date = Local::now();
        let resource_date = Local.from_local_datetime(&self.taken.unwrap());

        let current_week_of_year = current_date.week_of_year();
        let resource_week_of_year = resource_date.unwrap().week_of_year();

        current_week_of_year == resource_week_of_year
    }
}

impl Display for RemoteResource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {} {:?} {:?}",
            self.id,
            self.path,
            self.content_type,
            self.name,
            self.content_length,
            self.last_modified,
            self.taken,
            self.location,
        )
    }
}