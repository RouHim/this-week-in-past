use std::sync::{Arc, Mutex};

use actix_web::get;
use actix_web::web;
use actix_web::HttpResponse;
use evmap::{ReadHandle, WriteHandle};

use crate::resource_reader::{RemoteResource, ResourceReader};
use crate::{image_processor, resource_processor};

#[get("")]
pub async fn list_all_resources(kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let keys: Vec<String> = resource_processor::get_all(kv_reader.as_ref());

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

#[get("week")]
pub async fn list_this_week_resources(
    kv_reader: web::Data<ReadHandle<String, String>>,
) -> HttpResponse {
    let keys: Vec<String> = resource_processor::get_this_week_in_past(kv_reader.as_ref());

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

#[get("random")]
pub async fn random_resource(kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let resource_id: Option<String> = resource_processor::random_entry(kv_reader.as_ref());

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
    kv_reader: web::Data<ReadHandle<String, String>>,
    resource_reader: web::Data<ResourceReader>,
) -> HttpResponse {
    let path_params = resources_id.into_inner();
    let resource_id = path_params.0.as_str();
    let display_width = path_params.1;
    let display_height = path_params.2;

    // check cache
    let cached_data = cacache::read(
        resource_processor::get_cache_dir(),
        format!("{resource_id}_{display_width}_{display_height}"),
    );
    if let Ok(cached_data) = cached_data.await {
        return HttpResponse::Ok()
            .content_type("image/png")
            .body(cached_data);
    }

    // if no cache, fetch from remote
    let remote_resource = kv_reader
        .get_one(resource_id)
        .map(|value| value.to_string())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok());

    let orientation = remote_resource
        .clone()
        .and_then(|remote_resource: RemoteResource| remote_resource.orientation);

    let resource_data = remote_resource
        .map(|remote_resource| resource_reader.read_resource_data(&remote_resource))
        .map(|resource_data| {
            image_processor::optimize_image(
                resource_data,
                display_width,
                display_height,
                orientation,
            )
        });

    if let Some(resource_data) = resource_data {
        cacache::write(
            resource_processor::get_cache_dir(),
            format!("{resource_id}_{display_width}_{display_height}"),
            &resource_data,
        )
        .await
        .expect("writing to cache failed");

        HttpResponse::Ok()
            .content_type("image/png")
            .body(resource_data)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/{display_width}/{display_height}/base64")]
pub async fn get_resource_base64_by_id_and_resolution(
    path_variables: web::Path<(String, u32, u32)>,
    kv_reader: web::Data<ReadHandle<String, String>>,
    resource_reader: web::Data<ResourceReader>,
) -> HttpResponse {
    let path_params = path_variables.into_inner();
    let resource_id = path_params.0.as_str();
    let display_width = path_params.1;
    let display_height = path_params.2;

    // check cache
    let cached_data = cacache::read(
        resource_processor::get_cache_dir(),
        format!("{resource_id}_{display_width}_{display_height}_base64"),
    );
    if let Ok(cached_data) = cached_data.await {
        return HttpResponse::Ok()
            .content_type("plain/text")
            .body(cached_data);
    }

    // Read image from dir
    let remote_resource = kv_reader
        .get_one(resource_id)
        .map(|value| value.to_string())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok());
    let orientation = remote_resource
        .clone()
        .and_then(|remote_resource: RemoteResource| remote_resource.orientation);

    let base64_image = remote_resource
        .map(|remote_resource| resource_reader.read_resource_data(&remote_resource))
        .map(|resource_data| {
            image_processor::optimize_image(
                resource_data.to_vec(),
                display_width,
                display_height,
                orientation,
            )
        })
        .map(|scaled_image| base64::encode(&scaled_image))
        .map(|base64_string| format!("data:image/png;base64,{}", base64_string));

    if let Some(base64_image) = base64_image {
        cacache::write(
            resource_processor::get_cache_dir(),
            format!("{resource_id}_{display_width}_{display_height}_base64"),
            base64_image.as_bytes(),
        )
        .await
        .expect("writing to cache failed");

        HttpResponse::Ok()
            .content_type("plain/text")
            .body(base64_image)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/metadata")]
pub async fn get_resource_metadata_by_id(
    resources_id: web::Path<String>,
    kv_reader: web::Data<ReadHandle<String, String>>,
) -> HttpResponse {
    let metadata = kv_reader
        .get_one(resources_id.as_str())
        .map(|value| value.to_string());

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
    kv_reader: web::Data<ReadHandle<String, String>>,
    kv_writer_mutex: web::Data<Arc<Mutex<WriteHandle<String, String>>>>,
) -> HttpResponse {
    let resource = kv_reader
        .get_one(resources_id.as_str())
        .map(|value| value.to_string())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok());

    let display_value = resource.map(|resource| {
        resource_processor::build_display_value(resource, kv_writer_mutex.get_ref().clone())
    });

    if let Some(display_value) = display_value {
        HttpResponse::Ok()
            .content_type("plain/text")
            .body(display_value.await)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}
