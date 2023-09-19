use actix_web::delete;
use actix_web::get;
use actix_web::post;
use actix_web::web;
use actix_web::HttpResponse;
use std::fs;

use crate::resource_reader::ImageResource;
use crate::resource_store::ResourceStore;
use crate::{image_processor, resource_processor, ResourceReader};

#[get("")]
pub async fn get_all_resources(resource_store: web::Data<ResourceStore>) -> HttpResponse {
    let keys: Vec<String> = resource_store.get_ref().get_all_resource_ids();

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

#[get("week")]
pub async fn get_this_week_resources(resource_store: web::Data<ResourceStore>) -> HttpResponse {
    let resource_ids = resource_store
        .as_ref()
        .get_resources_this_week_visible_random();

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&resource_ids).unwrap())
}

#[get("random")]
pub async fn random_resource(resource_store: web::Data<ResourceStore>) -> HttpResponse {
    let resource_id: Option<String> = resource_store.get_random_resource();

    if let Some(resource_id) = resource_id {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(serde_json::to_string(&resource_id).unwrap())
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

// TODO: Refactor me
#[get("{resource_id}/{display_width}/{display_height}")]
pub async fn get_resource_by_id_and_resolution(
    resources_id: web::Path<(String, u32, u32)>,
    resource_reader: web::Data<ResourceReader>,
    resource_store: web::Data<ResourceStore>,
) -> HttpResponse {
    let path_params = resources_id.into_inner();
    let resource_id = path_params.0.as_str();
    let display_width = path_params.1;
    let display_height = path_params.2;

    // Check cache, if successful return it
    let cached_data = resource_store
        .get_ref()
        .get_data_cache_entry(format!("{resource_id}_{display_width}_{display_height}"));
    if let Some(cached_data) = cached_data {
        return HttpResponse::Ok()
            .content_type("image/png")
            .body(cached_data);
    }

    // if not in cache, load resource metadata from database
    let image_resource: Option<ImageResource> = resource_store
        .get_resource(resource_id)
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok());
    // If we can't find the requested resource by id, return with an error
    if image_resource.is_none() {
        return HttpResponse::NotFound().finish();
    }

    // If we found the requested resource, read the image data and adjust the image to the display
    let image_resource = image_resource.unwrap();
    let _resource_reader1 = resource_reader.as_ref();
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
        resource_store.get_ref().add_data_cache_entry(
            format!("{resource_id}_{display_width}_{display_height}"),
            &resource_data,
        );

        HttpResponse::Ok()
            .content_type("image/png")
            .body(resource_data)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/metadata")]
pub async fn get_resource_metadata_by_id(
    resource_id: web::Path<String>,
    resource_store: web::Data<ResourceStore>,
) -> HttpResponse {
    let metadata = resource_store.get_resource(resource_id.as_ref());

    if let Some(metadata) = metadata {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(metadata)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/description")]
pub async fn get_resource_metadata_description_by_id(
    resources_id: web::Path<String>,
    resource_store: web::Data<ResourceStore>,
) -> HttpResponse {
    let resource = resource_store
        .get_resource(resources_id.as_str())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok());

    let display_value = resource
        .map(|resource| resource_processor::build_display_value(resource, resource_store.as_ref()));

    if let Some(display_value) = display_value {
        HttpResponse::Ok()
            .content_type("plain/text")
            .body(display_value.await)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[post("/hide/{resource_id}")]
pub async fn set_resource_hidden(
    resources_id: web::Path<String>,
    resource_store: web::Data<ResourceStore>,
) -> HttpResponse {
    resource_store.get_ref().add_hidden(resources_id.as_str());
    HttpResponse::Ok().finish()
}

#[delete("/hide/{resource_id}")]
pub async fn delete_resource_hidden(
    resources_id: web::Path<String>,
    resource_store: web::Data<ResourceStore>,
) -> HttpResponse {
    resource_store
        .get_ref()
        .remove_hidden(resources_id.as_str());
    HttpResponse::Ok().finish()
}

#[get("/hide")]
pub async fn get_all_hidden_resources(resource_store: web::Data<ResourceStore>) -> HttpResponse {
    let hidden_ids: Vec<String> = resource_store.as_ref().get_all_hidden();
    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&hidden_ids).unwrap())
}
