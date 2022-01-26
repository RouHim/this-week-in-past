use std::env;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use chrono::{Local, NaiveDateTime, TimeZone};
use now::DateTimeNow;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use reqwest::blocking::{Body, Response};
use reqwest::Method;
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};

use crate::{exif_reader, resource_processor};
use crate::geo_location::GeoLocation;
use crate::image_processor::ImageOrientation;

#[derive(Clone)]
pub struct WebDavClient {
    pub username: String,
    pub password: String,
    pub base_url: String,
    pub http_client: reqwest::blocking::Client,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebDavResource {
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

impl WebDavResource {
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

impl Display for WebDavResource {
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

impl WebDavClient {
    pub fn request_resource_data(&self, resource: &WebDavResource) -> Response {
        let resource_url = format!("{}{}", self.base_url, &resource.path);
        self.http_client.request(Method::GET, resource_url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .unwrap()
    }

    pub fn list_all_resources(&self) -> Vec<WebDavResource> {
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
            <D:propfind xmlns:D="DAV:">
                <D:allprop/>
            </D:propfind>
        "#;

        let sub_path = env::var("TWIP_WEBDAV_RESOURCE_LOCATION")
            .expect("TWIP_WEBDAV_RESOURCE_LOCATION is missing");

        let response = self.http_client.request(
            Method::from_str("PROPFIND").unwrap(),
            format!("{}{}", self.base_url, sub_path),
        )
            .basic_auth(&self.username, Some(&self.password))
            .body(Body::from(body))
            .send();

        if response.is_err() {
            return vec![];
        }

        let response_body = response.unwrap()
            .text().unwrap();

        parse_propfind_result(self, response_body)
    }
}

pub fn new(base_url: &str, username: &str, password: &str) -> WebDavClient {
    WebDavClient {
        username: username.to_string(),
        password: password.to_string(),
        base_url: base_url.to_string(),
        http_client: reqwest::blocking::Client::new(),
    }
}

fn parse_propfind_result(web_dav_client: &WebDavClient, xml: String) -> Vec<WebDavResource> {
    let doc = Document::parse(&xml).unwrap();
    let multi_status = doc.descendants()
        .find(|node| node.tag_name().name().eq("multistatus"))
        .unwrap();

    let xml_nodes: Vec<WebDavResource> = multi_status.descendants()
        .filter(|node| node.tag_name().name().eq("response"))
        .filter_map(parse_resource_node)
        .collect();

    xml_nodes.par_iter()
        .filter(|resource| resource.content_type.starts_with("image/"))
        .filter(|resource| !resource.path.contains("thumbnail"))
        .map(|resource| exif_reader::fill_exif_data(web_dav_client, resource))
        .collect()
}

fn parse_resource_node(response_node: Node) -> Option<WebDavResource> {
    let path = response_node.descendants()
        .find(|descendant| descendant.tag_name().name().eq("href"));

    let content_length = response_node.descendants()
        .find(|descendant| descendant.tag_name().name().eq("getcontentlength"));

    let last_modified = response_node.descendants()
        .find(|descendant| descendant.tag_name().name().eq("getlastmodified"));

    let content_type = response_node.descendants()
        .find(|descendant| descendant.tag_name().name().eq("getcontenttype"));

    let e_tag = response_node.descendants()
        .find(|descendant| descendant.tag_name().name().eq("getetag"));

    let name = response_node.descendants()
        .find(|descendant| descendant.tag_name().name().eq("displayname"));

    if path.is_none()
        || content_length.is_none()
        || last_modified.is_none()
        || content_type.is_none()
        || e_tag.is_none()
        || name.is_none()
    {
        return None;
    }

    Some(WebDavResource {
        id: resource_processor::md5(path.unwrap().text().unwrap()),
        path: path.unwrap().text().unwrap().to_string(),
        content_type: content_type.unwrap().text().unwrap().to_string(),
        name: name.unwrap().text().unwrap().to_string(),
        content_length: content_length.unwrap().text().unwrap().to_string().parse().unwrap(),
        last_modified: NaiveDateTime::parse_from_str(
            last_modified.unwrap().text().unwrap(),
            "%a, %d %h %Y %T %Z",
        ).unwrap(),
        taken: None,
        location: None,
        orientation: None,
    })
}
