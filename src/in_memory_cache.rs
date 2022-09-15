use std::sync::{Arc, Mutex};

use evmap::{ReadHandle, WriteHandle};

/// Holds the cache reader and writer
/// Writer is stored as an arc mutex for protected shared writing
#[derive(Clone)]
pub struct InMemoryCache {
    kv_reader: ReadHandle<String, String>,
    kv_writer: Arc<Mutex<WriteHandle<String, String>>>,
}

/// Initializes a thread safe in memory cache
pub fn new() -> InMemoryCache {
    let (kv_reader, kv_writer) = evmap::new::<String, String>();
    InMemoryCache {
        kv_reader,
        kv_writer: Arc::new(Mutex::new(kv_writer)),
    }
}

impl InMemoryCache {
    /// Checks if the cache contains the given key
    /// Returns true if the key is present, false otherwise
    /// # Arguments
    /// * `key` - The key to check
    ///
    /// # Example
    /// ```
    /// use in_memory_cache::InMemoryCache;
    /// let mut in_memory_cache = InMemoryCache::init();
    /// let key = "key".to_string();
    /// in_memory_cache.insert(key.clone(), "value".to_string());
    /// assert_eq!(in_memory_cache.contains_key(key.as_str()), true);
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
    /// use in_memory_cache::InMemoryCache;
    /// let mut in_memory_cache = InMemoryCache::init();
    /// let key = "key".to_string();
    /// in_memory_cache.insert(key.clone(), "value".to_string());
    /// assert_eq!(in_memory_cache.get(key.as_str()), Some("value".to_string()));
    /// ```
    pub fn get(&self, key: &str) -> Option<String> {
        self.kv_reader.get_one(key).map(|t| t.to_string())
    }

    /// Inserts the given key and value into the cache
    /// Refreshes the cache after the insert
    /// # Arguments
    /// * `key` - The key to insert
    /// * `value` - The value to insert
    /// # Example
    /// ```
    /// use in_memory_cache::InMemoryCache;
    /// let mut in_memory_cache = InMemoryCache::init();
    /// let key = "key".to_string();
    /// let value = "value".to_string();
    /// in_memory_cache.insert(key.clone(), value.clone());
    /// assert_eq!(in_memory_cache.get(key.as_str()), Some(value));
    /// ```
    pub fn insert(&self, key: String, value: String) {
        let mut kv_writer = self.kv_writer.lock().unwrap();
        kv_writer.insert(key, value);
        kv_writer.refresh();
    }
}
