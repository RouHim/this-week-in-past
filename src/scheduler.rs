use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clokwerk::{ScheduleHandle, Scheduler, TimeUnits};
use evmap::WriteHandle;

use crate::web_dav_client;

pub fn run(kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>) -> ScheduleHandle {
    let mut scheduler = Scheduler::new();
    scheduler.every(1.day()).at("13:53:00").run(move || fetch_images(kv_writer_mutex.clone()));
    scheduler.watch_thread(Duration::from_millis(100))
}

pub fn fetch_images(kv_writer_mutex: Arc<Mutex<WriteHandle<String, String>>>) {
    let s = Instant::now();
    println!("Begin fetch_images");

    let mut kv_writer = kv_writer_mutex.lock().unwrap();

    println!("Purging kv store");
    kv_writer.purge();

    println!("Fetching resources from webdav");
    let resources = web_dav_client::fetch_all_resources();

    println!("Storing {} items", resources.len());
    for resource in resources {
        kv_writer.insert(
            format!("{:x}", md5::compute(resource.path.as_bytes())),
            serde_json::to_string(&resource).unwrap(),
        );
    };
    kv_writer.refresh();

    println!("Job done in {}ms!", s.elapsed().as_millis());
}