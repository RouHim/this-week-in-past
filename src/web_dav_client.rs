use std::fmt::{Display, Formatter};
use std::str::FromStr;

use chrono::NaiveDateTime;
use reqwest::blocking::Body;
use reqwest::Method;
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct WebDavResource {
    pub path: String,
    pub content_type: String,
    pub name: String,
    pub content_length: u64,
    pub last_modified: NaiveDateTime,
    pub e_tag: String,
}

impl Display for WebDavResource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {} ",
            self.path,
            self.content_type,
            self.name,
            self.content_length,
            self.last_modified,
            self.e_tag,
        )
    }
}

pub fn fetch_all_resources() -> Vec<WebDavResource> {
    let client = reqwest::blocking::Client::new();
    let request_body = r#"
    <?xml version="1.0" encoding="utf-8" ?>
    <D:propfind xmlns:D="DAV:">
            <D:allprop/>
    </D:propfind>
    "#;

    let response = client.request(
        Method::from_str("PROPFIND").unwrap(),
        "https://photos.himmelstein.info/originals",
    )
        .basic_auth("admin", Some("hPjCqWh5#P8c*r9XijqE"))
        .body(Body::from(request_body))
        .send();

    if response.is_err() {
        return vec![];
    }

    let response_body = response.unwrap()
        .text().unwrap();

    return parse_propfind_result(response_body);
}

fn parse_propfind_result(xml: String) -> Vec<WebDavResource> {
    let doc = Document::parse(&xml).unwrap();
    let multi_status = doc.descendants()
        .find(|node| node.tag_name().name().eq("multistatus"))
        .unwrap();

    multi_status.descendants()
        .filter(|node| node.tag_name().name().eq("response"))
        .filter_map(|response_node| parse_resource_node(response_node))
        // TODO: allow all type of media and convert on get, to an image
        .filter(|resource| resource.content_type.eq("image/jpeg"))
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
        path: path.unwrap().text().unwrap().to_string(),
        content_type: content_type.unwrap().text().unwrap().to_string(),
        name: name.unwrap().text().unwrap().to_string(),
        content_length: content_length.unwrap().text().unwrap().to_string().parse().unwrap(),
        last_modified: NaiveDateTime::parse_from_str(
            last_modified.unwrap().text().unwrap(),
            "%a, %d %h %Y %T %Z",
        ).unwrap(),
        e_tag: e_tag.unwrap().text().unwrap().to_string(),
    })
}