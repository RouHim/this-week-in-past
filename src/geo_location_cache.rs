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
    /// Checks if the geo location cache contains the given key
    /// Returns true if the key is present, false otherwise
    /// # Arguments
    /// * `key` - The key to check
    ///
    /// # Example
    /// ```
    /// use geo_location_cache::GeoLocationCache;
    /// let mut geo_location_cache = GeoLocationCache::init();
    /// let key = "key".to_string();
    /// geo_location_cache.insert(key.clone(), "value".to_string());
    /// assert_eq!(geo_location_cache.contains_key(key.as_str()), true);
    /// ```
    ///
    pub fn contains_key(&self, key: &str) -> bool {
        self.kv_reader.contains_key(key)
    }

    /// Returns the value for the given key
    /// Returns `None` if the key is not in the cache
    /// # Arguments
    /// * `key` - The key to get the value for
    /// # Example
    /// ```
    /// use geo_location_cache::GeoLocationCache;
    /// let mut geo_location_cache = GeoLocationCache::init();
    /// let key = "key".to_string();
    /// geo_location_cache.insert(key.clone(), "value".to_string());
    /// assert_eq!(geo_location_cache.get(key.as_str()), Some("value".to_string()));
    /// ```
    pub fn get(&self, key: &str) -> Option<String> {
        self.kv_reader.get_one(key).map(|t| t.to_string())
    }

    /// Inserts the given key and value into the geo location cache
    /// Refreshes the cache after the insert
    /// # Arguments
    /// * `key` - The key to insert
    /// * `value` - The value to insert
    /// # Example
    /// ```
    /// use geo_location_cache::GeoLocationCache;
    /// let mut geo_location_cache = GeoLocationCache::init();
    /// let key = "key".to_string();
    /// let value = "value".to_string();
    /// geo_location_cache.insert(key.clone(), value.clone());
    /// assert_eq!(geo_location_cache.get(key.as_str()), Some(value));
    /// ```
    pub fn insert(&mut self, key: String, value: String) {
        self.kv_writer.insert(key, value);
        self.kv_writer.refresh();
    }
}
