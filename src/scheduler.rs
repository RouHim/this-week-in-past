use std::collections::HashMap;
use std::thread;

use std::time::{Duration, Instant};

use clokwerk::{Job, ScheduleHandle, Scheduler, TimeUnits};
use log::{info, warn};

use crate::resource_reader::ImageResource;
use crate::resource_store::ResourceStore;
use crate::ResourceReader;

/// Schedules the cache indexer at every day at midnight
pub fn schedule_indexer(
    resource_reader: ResourceReader,
    resource_store: ResourceStore,
) -> ScheduleHandle {
    let mut scheduler = Scheduler::new();

    let resource_reader_clone = resource_reader.clone();
    let resource_store_clone = resource_store.clone();

    // Fetch resources at midnight
    scheduler
        .every(1.day())
        .at("00:05")
        .run(move || index_resources(resource_reader.clone(), resource_store.clone()));

    // For the first time on application start
    thread::spawn(move || {
        index_resources(resource_reader_clone, resource_store_clone);
    });

    // Check the thread every minute
    scheduler.watch_thread(Duration::from_secs(60))
}

/// Id collisions between distinct paths: `(id, first_path, conflicting_path)`.
type IdCollisions = Vec<(String, String, String)>;

/// Serializes resources into the id -> JSON map stored in the resource store.
/// Returns the map and one tuple per id claimed by two distinct paths
/// (`id, first_path, conflicting_path`). An identical path repeated is
/// deduplicated silently; the last occurrence of an id wins in the map.
fn collect_resources(resources: &[ImageResource]) -> (HashMap<String, String>, IdCollisions) {
    let map = resources
        .iter()
        .map(|resource| {
            (
                resource.id.clone(),
                serde_json::to_string(resource).unwrap(),
            )
        })
        .collect();

    let mut seen_paths: HashMap<&str, &str> = HashMap::new();
    let collisions = resources
        .iter()
        .filter_map(|resource| {
            let seen_path = seen_paths.insert(resource.id.as_str(), resource.path.as_str())?;
            (seen_path != resource.path.as_str()).then(|| {
                (
                    resource.id.clone(),
                    seen_path.to_string(),
                    resource.path.clone(),
                )
            })
        })
        .collect();

    (map, collisions)
}

/// Fetches the resources from the configures paths and writes them to the resource store
pub fn index_resources(resource_reader: ResourceReader, resource_store: ResourceStore) {
    let s = Instant::now();
    info!("Begin resource indexing");

    info!("Indexing resources, this may take some time depending on the amount of resources...");
    let resources = resource_reader.read_all();

    info!("Found {} resources", resources.len());
    let (map, collisions) = collect_resources(&resources);
    for (id, first_path, conflicting_path) in &collisions {
        warn!(
            "Duplicate resource id {id}: {first_path:?} and {conflicting_path:?} both map to it; keeping the last indexed one"
        );
    }

    info!("Purging resources store");
    resource_store.clear_resources();

    info!("Pruning image cache entries of removed resources");
    let cache_dir = crate::image_cache::cache_dir(
        &std::env::var("DATA_FOLDER")
            .or_else(|_| std::env::var("CACHE_DIR"))
            .unwrap_or_else(|_| "./data".into()),
    );
    let keep_ids: std::collections::HashSet<String> = map.keys().cloned().collect();
    let _ = crate::image_cache::retain(&cache_dir, &keep_ids);

    info!("Inserting new resources");
    resource_store.add_resources(map);

    info!("Cleanup database");
    resource_store.vacuum();

    info!("Job done in {:?}!", s.elapsed());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_reader::ImageResource;

    #[test]
    fn collect_resources_flags_distinct_paths_sharing_one_id() {
        // GIVEN two resources whose ids collide but whose paths differ, plus one unique
        let a = ImageResource {
            id: "same".into(),
            path: "/lib/a/Picture 001.jpg".into(),
            ..Default::default()
        };
        let b = ImageResource {
            id: "same".into(),
            path: "/lib/b/Picture 001.jpg".into(),
            ..Default::default()
        };
        let c = ImageResource {
            id: "other".into(),
            path: "/lib/c/photo.jpg".into(),
            ..Default::default()
        };

        // WHEN collecting resources
        let (map, collisions) = collect_resources(&[a, b, c]);

        // THEN the map keeps the last occurrence and the collision names both paths
        assert_eq!(map.len(), 2);
        assert_eq!(collisions.len(), 1);
        assert_eq!(
            collisions[0],
            (
                "same".to_string(),
                "/lib/a/Picture 001.jpg".to_string(),
                "/lib/b/Picture 001.jpg".to_string()
            )
        );
        let kept: ImageResource = serde_json::from_str(map.get("same").unwrap()).unwrap();
        assert_eq!(kept.path, "/lib/b/Picture 001.jpg");
    }

    #[test]
    fn collect_resources_dedups_identical_paths_silently() {
        // GIVEN the same path discovered twice (e.g. overlapping RESOURCE_PATHS)
        let a = ImageResource {
            id: "same".into(),
            path: "/lib/a.jpg".into(),
            ..Default::default()
        };
        let b = a.clone();

        // WHEN collecting resources
        let (map, collisions) = collect_resources(&[a, b]);

        // THEN one entry remains and no collision is reported
        assert_eq!(map.len(), 1);
        assert!(collisions.is_empty());
    }
}
