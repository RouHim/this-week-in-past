use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clokwerk::{ScheduleHandle, Scheduler, TimeUnits};
use evmap::WriteHandle;

use crate::resource_reader::ResourceReader;
use crate::CACHE_DIR;

/// Initializes the scheduler by creating the cache directory
pub fn init() {
    // create cache dir
    std::fs::create_dir_all(CACHE_DIR).expect("Creating cache dir");
}

/// Schedules the cache indexer at every day at midnight
pub fn schedule_indexer(
    resource_reader: ResourceReader,
    kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>,
) -> ScheduleHandle {
    let mut scheduler = Scheduler::new();

    // Fetch resources at midnight
    scheduler
        .every(1.day())
        .at("00:00")
        .run(move || fetch_resources(resource_reader.clone(), kv_writer_mutex.clone()));

    // Check the thread every minute
    scheduler.watch_thread(Duration::from_secs(60))
}

/// Fetches the resources from the configures paths and writes them to the cache
pub fn fetch_resources(
    resource_reader: ResourceReader,
    kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>,
) {
    let s = Instant::now();
    println!("Begin fetch_images");

    let mut kv_writer = kv_writer_mutex.lock().unwrap();

    println!("Purging kv store");
    kv_writer.purge();

    println!("Fetching resources...");
    let resources = resource_reader.list_all_resources();

    println!("Storing {} items", resources.len());
    for resource in resources {
        kv_writer.insert(
            resource.id.clone(),
            serde_json::to_string(&resource).unwrap(),
        );
    }
    kv_writer.refresh();

    println!("Cleanup cache");
    cacache::clear_sync(CACHE_DIR).expect("Cleaning cache");

    println!("Job done in {}s!", s.elapsed().as_secs());
}
