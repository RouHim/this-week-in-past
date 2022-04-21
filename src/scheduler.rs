use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clokwerk::{ScheduleHandle, Scheduler, TimeUnits};
use evmap::WriteHandle;

use crate::CACHE_DIR;
use crate::resource_reader::ResourceReader;

pub fn run_webdav_indexer(resource_reader: ResourceReader, kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>) -> ScheduleHandle {
    let mut scheduler = Scheduler::new();

    // Fetch webdav resources at midnight
    scheduler.every(1.day()).at("13:06").run(
        move || fetch_resources(resource_reader.clone(), kv_writer_mutex.clone())
    );

    // Check the thread every minute
    scheduler.watch_thread(Duration::from_secs(60))
}

pub fn fetch_resources(resource_reader: ResourceReader, kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>) {
    let s = Instant::now();
    println!("Begin fetch_images");

    let mut kv_writer = kv_writer_mutex.lock().unwrap();

    println!("Purging kv store");
    kv_writer.purge();

    println!("Fetching resources from webdav...");
    let resources = resource_reader.list_all_resources();

    println!("Storing {} items", resources.len());
    for resource in resources {
        kv_writer.insert(
            resource.id.clone(),
            serde_json::to_string(&resource).unwrap(),
        );
    };
    kv_writer.refresh();

    println!("Cleanup cache");
    cacache::clear_sync(CACHE_DIR).expect("Cleaning cache");

    println!("Job done in {}s!", s.elapsed().as_secs());
}