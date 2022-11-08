use actix_web::delete;
use actix_web::get;
use actix_web::post;
use actix_web::web;
use actix_web::HttpResponse;

use crate::resource_reader::RemoteResource;
use crate::resource_store::ResourceStore;
use crate::{image_processor, resource_processor, resource_reader, ResourceReader};

#[get("")]
pub async fn get_all_resources(resource_store: web::Data<ResourceStore>) -> HttpResponse {
    let keys: Vec<String> = resource_store.get_ref().get_all_resource_ids();

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

#[get("week")]
pub async fn get_this_week_resources(resource_store: web::Data<ResourceStore>) -> HttpResponse {
    let keys: Vec<String> = resource_processor::get_this_week_in_past(resource_store.as_ref());

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
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

    // check cache
    let cached_data = resource_store
        .get_ref()
        .get_data_cache_entry(format!("{resource_id}_{display_width}_{display_height}"));

    if let Some(cached_data) = cached_data {
        return HttpResponse::Ok()
            .content_type("image/png")
            .body(cached_data);
    }

    // if no cache, fetch from remote
    let remote_resource = resource_store
        .get_resource(resource_id)
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok());

    let orientation = remote_resource
        .clone()
        .and_then(|remote_resource: RemoteResource| remote_resource.orientation);

    let resource_data = remote_resource
        .map(|remote_resource| {
            resource_reader::read_resource_data(resource_reader.as_ref(), &remote_resource)
        })
        .map(|resource_data| {
            image_processor::optimize_image(
                resource_data,
                display_width,
                display_height,
                orientation,
            )
        });

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
