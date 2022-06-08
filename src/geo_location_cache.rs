use evmap::{ReadHandle, WriteHandle};

/// Holds the geo location cache reader and writer
pub struct GeoLocationCache {
    kv_reader: ReadHandle<String, String>,
    kv_writer: WriteHandle<String, String>,
}

/// Initializes the geo location cache
pub fn init() -> GeoLocationCache {
    let (kv_reader, kv_writer) = evmap::new::<String, String>();
    GeoLocationCache {
        kv_reader,
        kv_writer,
    }
}

impl GeoLocationCache {
    pub fn contains_key(&self, key: &str) -> bool {
        self.kv_reader.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.kv_reader.get_one(key).map(|t| t.to_string())
    }

    pub fn insert(&mut self, key: String, value: String) {
        self.kv_writer.insert(key, value);
        self.kv_writer.refresh();
    }
}
