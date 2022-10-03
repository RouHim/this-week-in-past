use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clokwerk::{ScheduleHandle, Scheduler, TimeUnits};
use evmap::WriteHandle;

use crate::{AppConfig, resource_processor, resource_reader};

/// Initializes the scheduler by creating the cache directory
pub fn init() {
    // create cache dir
    std::fs::create_dir_all(resource_processor::get_cache_dir()).expect("Creating cache dir");

    // Check if cache is writeable
    cacache::write_sync(resource_processor::get_cache_dir(), "test", b"test")
        .expect("Cache is not writeable");
}

/// Schedules the cache indexer at every day at midnight
pub fn schedule_indexer(
    app_config: AppConfig,
    kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>,
) -> ScheduleHandle {
    let mut scheduler = Scheduler::new();

    // Fetch resources at midnight
    scheduler
        .every(1.day())
        .at("00:00")
        .run(move || fetch_resources(app_config.clone(), kv_writer_mutex.clone()));

    // Check the thread every minute
    scheduler.watch_thread(Duration::from_secs(60))
}

/// Fetches the resources from the configures paths and writes them to the cache
pub fn fetch_resources(
    app_config: AppConfig,
    kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>,
) {
    let s = Instant::now();
    println!("Begin fetch_images");

    let mut kv_writer = kv_writer_mutex.lock().unwrap();

    println!("Purging kv store");
    kv_writer.purge();

    println!("Indexing resources, this may take some time depending on the amount of resources...");
    let resources = resource_reader::list_all_resources(app_config);
    resources.iter().for_each(|x| println!("{}", x)); // TODO: remove me

    println!("Storing {} items", resources.len());
    for resource in resources {
        kv_writer.insert(
            resource.id.clone(),
            serde_json::to_string(&resource).unwrap(),
        );
    }
    kv_writer.refresh();

    println!("Cleanup cache");
    let _ = cacache::clear_sync(resource_processor::get_cache_dir());

    println!("Job done in {}s!", s.elapsed().as_secs());
}
