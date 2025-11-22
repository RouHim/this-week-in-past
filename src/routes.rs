use std::convert::Infallible;
use std::sync::Arc;

use warp::filters::BoxedFilter;
use warp::reply::Response;
use warp::Filter;

use crate::config_endpoint;
use crate::resource_endpoint;
use crate::resource_store::ResourceStore;
use crate::weather_endpoint;
use crate::web_app_endpoint;
use crate::ResourceReader;

/// Build all Warp routes for the application.
pub fn build_routes(
    resource_store: ResourceStore,
    _resource_reader: ResourceReader,
) -> BoxedFilter<(Response,)> {
    let resource_store = Arc::new(resource_store);

    let store_filter = warp::any().map({
        let resource_store = resource_store.clone();
        move || resource_store.clone()
    });

    let resources = resource_routes(store_filter.clone());
    let weather = weather_routes();
    let config = config_routes();
    let version = version_route();
    let health = health_route();
    let web_app = web_app_routes();

    resources
        .or(weather)
        .unify()
        .or(config)
        .unify()
        .or(version)
        .unify()
        .or(health)
        .unify()
        .or(web_app)
        .unify()
        .boxed()
}

fn resource_routes(
    store_filter: impl Filter<Extract = (Arc<ResourceStore>,), Error = Infallible>
        + Clone
        + Send
        + Sync
        + 'static,
) -> BoxedFilter<(Response,)> {
    let get_all = warp::path!("api" / "resources")
        .and(warp::path::end())
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::get_all_resources);

    let get_week = warp::path!("api" / "resources" / "week")
        .and(warp::path::end())
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::get_this_week_resources);

    let get_week_count = warp::path!("api" / "resources" / "week" / "count")
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::get_this_week_resources_count);

    let get_week_metadata = warp::path!("api" / "resources" / "week" / "metadata")
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::get_this_week_resources_metadata);

    let get_week_image = warp::path!("api" / "resources" / "week" / "image")
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::get_this_week_resource_image);

    let get_random = warp::path!("api" / "resources" / "random")
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::random_resources);

    let get_by_id_and_resolution = warp::path!("api" / "resources" / String / u32 / u32)
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::get_resource_by_id_and_resolution);

    let get_metadata = warp::path!("api" / "resources" / String / "metadata")
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::get_resource_metadata_by_id);

    let get_description = warp::path!("api" / "resources" / String / "description")
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::get_resource_metadata_description_by_id);

    let get_all_hidden = warp::path!("api" / "resources" / "hide")
        .and(warp::get())
        .and(store_filter.clone())
        .and_then(resource_endpoint::get_all_hidden_resources);

    let set_hidden = warp::path!("api" / "resources" / "hide" / String)
        .and(warp::post())
        .and(store_filter.clone())
        .and_then(resource_endpoint::set_resource_hidden);

    let delete_hidden = warp::path!("api" / "resources" / "hide" / String)
        .and(warp::delete())
        .and(store_filter)
        .and_then(resource_endpoint::delete_resource_hidden);

    get_all
        .or(get_week)
        .unify()
        .or(get_week_count)
        .unify()
        .or(get_week_metadata)
        .unify()
        .or(get_week_image)
        .unify()
        .or(get_random)
        .unify()
        .or(get_by_id_and_resolution)
        .unify()
        .or(get_metadata)
        .unify()
        .or(get_description)
        .unify()
        .or(get_all_hidden)
        .unify()
        .or(set_hidden)
        .unify()
        .or(delete_hidden)
        .unify()
        .boxed()
}

fn weather_routes() -> BoxedFilter<(Response,)> {
    let is_enabled = warp::path!("api" / "weather")
        .and(warp::path::end())
        .and(warp::get())
        .and_then(weather_endpoint::get_is_weather_enabled);

    let current = warp::path!("api" / "weather" / "current")
        .and(warp::get())
        .and_then(weather_endpoint::get_current_weather);

    let home_assistant_enabled = warp::path!("api" / "weather" / "homeassistant")
        .and(warp::get())
        .and_then(weather_endpoint::get_is_home_assistant_enabled);

    let home_assistant_temperature =
        warp::path!("api" / "weather" / "homeassistant" / "temperature")
            .and(warp::get())
            .and_then(weather_endpoint::get_home_assistant_entity_data);

    let unit = warp::path!("api" / "weather" / "unit")
        .and(warp::get())
        .and_then(weather_endpoint::get_weather_unit);

    is_enabled
        .or(current)
        .unify()
        .or(home_assistant_enabled)
        .unify()
        .or(home_assistant_temperature)
        .unify()
        .or(unit)
        .unify()
        .boxed()
}

fn config_routes() -> BoxedFilter<(Response,)> {
    let slideshow = warp::path!("api" / "config" / "interval" / "slideshow")
        .and(warp::get())
        .and_then(config_endpoint::get_slideshow_interval);

    let refresh = warp::path!("api" / "config" / "interval" / "refresh")
        .and(warp::get())
        .and_then(config_endpoint::get_refresh_interval);

    let hide_button = warp::path!("api" / "config" / "show-hide-button")
        .and(warp::get())
        .and_then(config_endpoint::get_hide_button_enabled);

    let random = warp::path!("api" / "config" / "random-slideshow")
        .and(warp::get())
        .and_then(config_endpoint::get_random_slideshow_enabled);

    let preload = warp::path!("api" / "config" / "preload-images")
        .and(warp::get())
        .and_then(config_endpoint::get_preload_images_enabled);

    slideshow
        .or(refresh)
        .unify()
        .or(hide_button)
        .unify()
        .or(random)
        .unify()
        .or(preload)
        .unify()
        .boxed()
}

fn version_route() -> BoxedFilter<(Response,)> {
    warp::path!("api" / "version")
        .and(warp::get())
        .map(|| {
            warp::http::Response::builder()
                .header("content-type", "plain/text")
                .body(warp::hyper::Body::from(env!("CARGO_PKG_VERSION")))
                .unwrap()
        })
        .boxed()
}

fn health_route() -> BoxedFilter<(Response,)> {
    warp::path!("api" / "health")
        .and(warp::get())
        .map(|| warp::http::Response::new(warp::hyper::Body::empty()))
        .boxed()
}

fn web_app_routes() -> BoxedFilter<(Response,)> {
    let index = warp::path::end()
        .and(warp::get())
        .and_then(web_app_endpoint::index);

    let style = warp::path!("style.css")
        .and(warp::get())
        .and_then(web_app_endpoint::style_css);

    let script = warp::path!("script.js")
        .and(warp::get())
        .and_then(web_app_endpoint::script_js);

    let hide = warp::path!("images" / "hide.png")
        .and(warp::get())
        .and_then(web_app_endpoint::hide_png);

    let icon = warp::path!("icon.png")
        .and(warp::get())
        .and_then(web_app_endpoint::icon_png);

    let font = warp::path!("font.ttf")
        .and(warp::get())
        .and_then(web_app_endpoint::font);

    index
        .or(style)
        .unify()
        .or(script)
        .unify()
        .or(hide)
        .unify()
        .or(icon)
        .unify()
        .or(font)
        .unify()
        .boxed()
}
