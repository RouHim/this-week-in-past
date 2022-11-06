use std::collections::HashMap;

use std::time::{Duration, Instant};

use clokwerk::{ScheduleHandle, Scheduler, TimeUnits};

use crate::resource_store::ResourceStore;
use crate::ResourceReader;

/// Schedules the cache indexer at every day at midnight
pub fn schedule_indexer(
    app_config: ResourceReader,
    resource_store: ResourceStore,
) -> ScheduleHandle {
    let mut scheduler = Scheduler::new();

    // Fetch resources at midnight
    scheduler
        .every(1.day())
        .at("00:00")
        .run(move || fetch_resources(app_config.clone(), resource_store.clone()));

    // Check the thread every minute
    scheduler.watch_thread(Duration::from_secs(60))
}

/// Fetches the resources from the configures paths and writes them to the cache
pub fn fetch_resources(resource_reader: ResourceReader, resource_store: ResourceStore) {
    let s = Instant::now();
    println!("Begin fetching resources");

    println!("Purging resources store");
    resource_store.clear_resources();

    println!("Indexing resources, this may take some time depending on the amount of resources...");
    let resources = resource_reader.list_all_resources();

    println!("Found {} resources", resources.len());
    let map: HashMap<String, String> = resources
        .iter()
        .map(|resource| (resource.id.clone(), serde_json::to_string(resource).unwrap()))
        .collect();
    resource_store.add_resources(map);

    println!("Cleanup cache");
    resource_store.clear_data_cache();

    println!("Job done in {}s!", s.elapsed().as_secs());
}
