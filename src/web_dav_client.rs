use std::fmt::{Display, Formatter};
use std::str::FromStr;

use chrono::NaiveDateTime;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use reqwest::blocking::{Body, Response};
use reqwest::Method;
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};

use crate::{exif_reader, resource_processor};

#[derive(Clone)]
pub struct WebDavClient {
    pub username: String,
    pub password: String,
    pub base_url: String,
    pub http_client: reqwest::blocking::Client,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct Location {
    pub latitude: f32,
    pub longitude: f32,
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
    pub location: Option<Location>,
}

impl WebDavResource {
    pub fn is_this_week(&self) -> bool {
        // build get image stream
        // read exif metadata
        // filter for this week

        return true;
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

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[lat={} lon={}]",
            self.latitude,
            self.longitude,
        )
    }
}

impl WebDavClient {
    pub fn request_resource_data(&self, url: String) -> Response {
        self.http_client.request(Method::GET, url)
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

        let response = self.http_client.request(
            Method::from_str("PROPFIND").unwrap(),
            format!("{}/originals", self.base_url),
        )
            .basic_auth(&self.username, Some(&self.password))
            .body(Body::from(body))
            .send();

        if response.is_err() {
            return vec![];
        }

        let response_body = response.unwrap()
            .text().unwrap();

        return parse_propfind_result(self, response_body);
    }
}

pub fn new(base_url: &str, username: &str, password: &str) -> WebDavClient {
    return WebDavClient {
        username: username.to_string(),
        password: password.to_string(),
        base_url: base_url.to_string(),
        http_client: reqwest::blocking::Client::new(),
    };
}

fn parse_propfind_result(web_dav_client: &WebDavClient, xml: String) -> Vec<WebDavResource> {
    let doc = Document::parse(&xml).unwrap();
    let multi_status = doc.descendants()
        .find(|node| node.tag_name().name().eq("multistatus"))
        .unwrap();

    let xml_nodes: Vec<WebDavResource> = multi_status.descendants()
        .filter(|node| node.tag_name().name().eq("response"))
        .filter_map(|response_node| parse_resource_node(response_node))
        .collect();

    xml_nodes.par_iter()
        // TODO: allow all type of media and convert on get, to an image
        .filter(|resource| resource.content_type.eq("image/jpeg"))
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
        id: resource_processor::md5(&path.unwrap().text().unwrap().to_string()),
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
    })
}
