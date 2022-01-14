use std::time::Duration;
use clokwerk::{ScheduleHandle, Scheduler, TimeUnits};
use evmap::WriteHandle;
use crate::web_dav_client;

pub fn run(kv_writer: &mut WriteHandle<String, WebDavResource>) -> ScheduleHandle {
    let mut scheduler = Scheduler::new();
    scheduler.every(1.day()).at("00:00:00").run(|| fetch_images(kv_writer));
    scheduler.watch_thread(Duration::from_millis(100))
}

pub fn fetch_images(kv_writer: &mut WriteHandle<String, WebDavResource>) {
    kv_writer.purge();
    let photos = web_dav_client::fetch_all_resources();
    photos.iter().for_each(|resource| kv_writer)
}