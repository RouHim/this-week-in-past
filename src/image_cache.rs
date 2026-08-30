use parking_lot::Mutex;
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub const MAX_CACHE_FILES: usize = 500;
pub const MAX_CACHE_BYTES: u64 = 1_073_741_824;
static CACHE_MUTEX: Mutex<()> = Mutex::new(());

fn is_valid_key(key: &str) -> bool {
    !key.is_empty() && !key.contains('/') && !key.contains('\\') && !key.contains("..")
}

pub fn cache_dir(data_folder: &str) -> PathBuf {
    PathBuf::from(data_folder).join("cache")
}

pub fn get(cache_dir: &Path, key: &str) -> Option<Vec<u8>> {
    if !is_valid_key(key) {
        return None;
    }
    let path = cache_dir.join(key);
    let data = fs::read(&path).ok()?;
    let now = SystemTime::now();
    let _ = fs::File::open(&path).and_then(|f| f.set_modified(now));
    Some(data)
}

pub fn put(cache_dir: &Path, key: &str, data: &[u8]) -> io::Result<()> {
    if !is_valid_key(key) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid cache key",
        ));
    }
    let _guard = CACHE_MUTEX.lock();
    if let Err(e) = fs::create_dir_all(cache_dir) {
        log::warn!("cache dir create failed {}: {}", cache_dir.display(), e);
        return Ok(());
    }
    let dest = cache_dir.join(key);
    let tmp = cache_dir.join(format!(
        ".tmp-{}-{}-{:?}-{}",
        key,
        std::process::id(),
        std::thread::current().id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    if let Err(e) = fs::write(&tmp, data) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp, &dest) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    let now = SystemTime::now();
    let _ = fs::File::open(&dest).and_then(|f| f.set_modified(now));
    evict_if_needed(cache_dir);
    Ok(())
}

pub fn clear(cache_dir: &Path) -> io::Result<()> {
    let _guard = CACHE_MUTEX.lock();
    if !cache_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(cache_dir)? {
        let entry = entry?;
        let _ = fs::remove_file(entry.path());
    }
    Ok(())
}

#[allow(dead_code)]
pub fn cache_stats(cache_dir: &Path) -> (usize, u64) {
    let mut count = 0;
    let mut bytes = 0u64;
    if let Ok(rd) = fs::read_dir(cache_dir) {
        for e in rd.flatten() {
            if let Ok(md) = e.metadata() {
                if md.is_file() {
                    if e.file_name().to_string_lossy().starts_with(".tmp-") {
                        continue;
                    }
                    count += 1;
                    bytes += md.len();
                }
            }
        }
    }
    (count, bytes)
}

fn evict_if_needed(cache_dir: &Path) {
    let mut entries: Vec<(PathBuf, SystemTime, u64)> = vec![];
    let Ok(rd) = fs::read_dir(cache_dir) else {
        return;
    };
    for e in rd.flatten() {
        let Ok(md) = e.metadata() else {
            continue;
        };
        if !md.is_file() {
            continue;
        }
        if e.file_name().to_string_lossy().starts_with(".tmp-") {
            continue;
        }
        let mtime = md.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        entries.push((e.path(), mtime, md.len()));
    }
    entries.sort_by_key(|(_, t, _)| *t);
    let mut total_files = entries.len();
    let mut total_bytes: u64 = entries.iter().map(|(_, _, s)| *s).sum();
    let mut idx = 0;
    while (total_files > MAX_CACHE_FILES || total_bytes > MAX_CACHE_BYTES) && idx < entries.len() {
        let (path, _, size) = &entries[idx];
        if fs::remove_file(path).is_ok() {
            total_files -= 1;
            total_bytes -= *size;
        }
        idx += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn put_and_get_roundtrip() {
        // GIVEN a fresh filesystem cache directory
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();

        // WHEN putting and then getting the same key
        put(&cache, "abc_100_100.jpg", b"hello").unwrap();
        let result = get(&cache, "abc_100_100.jpg");

        // THEN the roundtrip returns the original bytes
        assert_eq!(result, Some(b"hello".to_vec()));
    }
    #[test]
    fn lru_evicts_oldest_when_over_count() {
        // GIVEN a cache exceeding the 500-file limit
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        for i in 0..501 {
            put(&cache, &format!("k{i}_10_10.jpg"), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }

        // WHEN checking cache stats after eviction
        let (count, _) = cache_stats(&cache);

        // THEN the oldest entry is evicted and newest remains
        assert!(count <= 500, "count {}", count);
        assert_eq!(get(&cache, "k0_10_10.jpg"), None);
        assert!(get(&cache, "k500_10_10.jpg").is_some());
    }
    #[test]
    fn missing_entry_is_cache_miss() {
        // GIVEN an empty cache directory
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();

        // WHEN requesting a non-existent key
        let result = get(&cache, "nope.jpg");

        // THEN the result is a cache miss (None)
        assert_eq!(result, None);
    }
    #[test]
    fn concurrent_put_no_corruption() {
        // GIVEN a shared cache directory with concurrent writers
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        let cache = std::sync::Arc::new(cache);
        let mut hs = vec![];
        for i in 0..20 {
            let c = cache.clone();
            hs.push(std::thread::spawn(move || {
                put(&c, &format!("c{i}_10_10.jpg"), &[i as u8; 100]).unwrap();
            }));
        }
        for h in hs {
            h.join().unwrap();
        }

        // WHEN all writers have finished
        let (count, bytes) = cache_stats(&cache);

        // THEN no corruption occurred and all entries are present
        assert_eq!(count, 20);
        assert_eq!(bytes, 20 * 100);
    }
}
