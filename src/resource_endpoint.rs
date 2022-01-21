use actix_http::Response as HttpResponse;
use actix_web::web;
use evmap::ReadHandle;

use crate::{get, resource_processor, WebDavClient};

#[get("")]
pub async fn list_resources(kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let keys: Vec<String> = resource_processor::get_all(kv_reader.as_ref());

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

#[get("week")]
pub async fn list_this_week_resources(kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let keys: Vec<String> = resource_processor::get_this_week_in_past(kv_reader.as_ref());

    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&keys).unwrap())
}

#[get("{resource_id}")]
pub async fn get_resource(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>, web_dav_client: web::Data<WebDavClient>) -> HttpResponse {
    let resource_data = kv_reader.get_one(resources_id.as_str())
        .map(|value| value.to_string())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok())
        .and_then(|web_dav_resource| web_dav_client.request_resource_data(&web_dav_resource).bytes().ok());

    if let Some(resource_data) = resource_data {
        HttpResponse::Ok()
            .content_type("image/jpeg")
            .body(resource_data.to_vec())
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/base64")]
pub async fn get_resource_base64(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>, web_dav_client: web::Data<WebDavClient>) -> HttpResponse {
    // TODO: serve in display resolution of client
    let base64_image = kv_reader.get_one(resources_id.as_str())
        .map(|value| value.to_string())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok())
        .and_then(|web_dav_resource| web_dav_client.request_resource_data(&web_dav_resource).bytes().ok())
        .map(|resource_data| base64::encode(&resource_data))
        .map(|base64_string| format!("data:image/jpeg;base64,{}", base64_string));

    if let Some(base64_image) = base64_image {
        HttpResponse::Ok()
            .content_type("plain/text")
            .body(base64_image)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/metadata")]
pub async fn get_resource_metadata(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let metadata = kv_reader.get_one(resources_id.as_str())
        .map(|value| value.to_string());

    if let Some(metadata) = metadata {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(metadata)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[get("{resource_id}/display")]
pub async fn get_resource_metadata_display(resources_id: web::Path<String>, kv_reader: web::Data<ReadHandle<String, String>>) -> HttpResponse {
    let display_value = kv_reader.get_one(resources_id.as_str())
        .map(|value| value.to_string())
        .and_then(|resource_json_string| serde_json::from_str(resource_json_string.as_str()).ok())
        .map(resource_processor::build_display_value);

    if let Some(display_value) = display_value {
        HttpResponse::Ok()
            .content_type("plain/text")
            .body(display_value)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}
