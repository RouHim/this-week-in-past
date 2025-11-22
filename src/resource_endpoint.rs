use log::{debug, log_enabled};
use std::convert::Infallible;
use std::fs;
use std::sync::Arc;
use warp::http::{Response, StatusCode};
use warp::hyper::Body;

use crate::resource_reader::ImageResource;
use crate::resource_store::ResourceStore;
use crate::{image_processor, resource_processor};

const CONTENT_TYPE_APPLICATION_JSON: &str = "application/json";
const CONTENT_TYPE_TEXT_PLAIN: &str = "text/plain";
const CONTENT_TYPE_IMAGE_PNG: &str = "image/png";

pub async fn get_all_resources(
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let keys: Vec<String> = resource_store.get_all_resource_ids();

    Ok(json_response(serde_json::to_string(&keys).unwrap()))
}

pub async fn get_this_week_resources(
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let resource_ids = resource_store.get_resources_this_week_visible_random();

    Ok(json_response(serde_json::to_string(&resource_ids).unwrap()))
}

pub async fn get_this_week_resources_count(
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let resource_count = resource_store.get_resources_this_week_visible_count();

    Ok(text_response(resource_count.to_string()))
}

pub async fn get_this_week_resources_metadata(
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let resource_ids = resource_store.get_resources_this_week_visible_random();

    // WARNING: this is not very efficient, but it's ok for a debug endpoint
    let resources_metadata: Vec<serde_json::Value> = resource_ids
        .iter()
        .flat_map(|id| resource_store.get_resource(id))
        .map(|resource_string| {
            serde_json::from_str::<serde_json::Value>(resource_string.as_str()).unwrap()
        })
        .collect();

    Ok(json_response(
        serde_json::to_string(&resources_metadata).unwrap(),
    ))
}

pub async fn get_this_week_resource_image(
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let resource_image: Option<ImageResource> = resource_store
        .get_resources_this_week_visible_random()
        .first()
        .and_then(|resource_id| resource_store.get_resource(resource_id))
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok());

    if resource_image.is_none() {
        return Ok(empty_response(StatusCode::NOT_FOUND));
    }

    // Read the image data from the file system and adjust the image to the display
    let image_resource = resource_image.unwrap();
    let resource_data = fs::read(&image_resource.path)
        .ok()
        .and_then(|resource_data| {
            image_processor::adjust_image(
                image_resource.path,
                resource_data,
                0,
                0,
                image_resource.orientation,
            )
        });

    if let Some(resource_data) = resource_data {
        Ok(image_response(resource_data))
    } else {
        Ok(empty_response(StatusCode::INTERNAL_SERVER_ERROR))
    }
}

pub async fn random_resources(
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let resource_ids: Vec<String> = resource_store.get_random_resources();

    Ok(json_response(serde_json::to_string(&resource_ids).unwrap()))
}

// TODO: Refactor me
pub async fn get_resource_by_id_and_resolution(
    resource_id: String,
    display_width: u32,
    display_height: u32,
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let resource_id_str = resource_id.as_str();

    // If RUST_LOG is DEBUG, print resource metadata
    if log_enabled!(log::Level::Debug) {
        let image_resource: Option<ImageResource> = resource_store
            .get_resource(resource_id_str)
            .and_then(|resource_json_string| {
                serde_json::from_str(resource_json_string.as_str()).ok()
            });
        if let Some(image_resource) = image_resource {
            debug!("Resource: {:?}", image_resource);
        }
    }

    // Check cache, if successful return it
    let cached_data = resource_store.get_data_cache_entry(format!(
        "{resource_id_str}_{display_width}_{display_height}"
    ));
    if let Some(cached_data) = cached_data {
        return Ok(image_response(cached_data));
    }

    // if not in cache, load resource metadata from database
    let image_resource: Option<ImageResource> = resource_store
        .get_resource(resource_id_str)
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok());
    // If we can't find the requested resource by id, return with an error
    if image_resource.is_none() {
        return Ok(empty_response(StatusCode::NOT_FOUND));
    }

    // If we found the requested resource, read the image data and adjust the image to the display
    let image_resource = image_resource.unwrap();
    let resource_data = fs::read(image_resource.path.clone())
        .ok()
        .and_then(|resource_data| {
            image_processor::adjust_image(
                image_resource.path,
                resource_data,
                display_width,
                display_height,
                image_resource.orientation,
            )
        });

    // If image adjustments were successful, return the data, otherwise return with error
    if let Some(resource_data) = resource_data {
        resource_store.add_data_cache_entry(
            format!("{resource_id_str}_{display_width}_{display_height}"),
            &resource_data,
        );

        Ok(image_response(resource_data))
    } else {
        Ok(empty_response(StatusCode::INTERNAL_SERVER_ERROR))
    }
}

pub async fn get_resource_metadata_by_id(
    resource_id: String,
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let metadata = resource_store.get_resource(resource_id.as_str());

    if let Some(metadata) = metadata {
        Ok(json_response(metadata))
    } else {
        Ok(empty_response(StatusCode::INTERNAL_SERVER_ERROR))
    }
}

pub async fn get_resource_metadata_description_by_id(
    resources_id: String,
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let resource = resource_store
        .get_resource(resources_id.as_str())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok());

    let display_value = resource
        .map(|resource| resource_processor::build_display_value(resource, resource_store.as_ref()));

    if let Some(display_value) = display_value {
        Ok(text_response(display_value.await))
    } else {
        Ok(empty_response(StatusCode::INTERNAL_SERVER_ERROR))
    }
}

pub async fn set_resource_hidden(
    resources_id: String,
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    resource_store.add_hidden(resources_id.as_str());
    Ok(empty_response(StatusCode::OK))
}

pub async fn delete_resource_hidden(
    resources_id: String,
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    resource_store.remove_hidden(resources_id.as_str());
    Ok(empty_response(StatusCode::OK))
}

pub async fn get_all_hidden_resources(
    resource_store: Arc<ResourceStore>,
) -> Result<Response<Body>, Infallible> {
    let hidden_ids: Vec<String> = resource_store.get_all_hidden();
    Ok(json_response(serde_json::to_string(&hidden_ids).unwrap()))
}

fn json_response(body: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", CONTENT_TYPE_APPLICATION_JSON)
        .body(Body::from(body))
        .unwrap()
}

fn text_response(body: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", CONTENT_TYPE_TEXT_PLAIN)
        .body(Body::from(body))
        .unwrap()
}

fn image_response(body: Vec<u8>) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", CONTENT_TYPE_IMAGE_PNG)
        .body(Body::from(body))
        .unwrap()
}

fn empty_response(status: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}
