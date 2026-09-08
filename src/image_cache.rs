use parking_lot::Mutex;
use std::{
    collections::HashSet,
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

/// Returns the filesystem cache directory for the given data folder.
/// Joins `cache` onto `data_folder` (e.g. `/data/cache`).
pub fn cache_dir(data_folder: &str) -> PathBuf {
    PathBuf::from(data_folder).join("cache")
}

/// Retrieves a cached entry by `key`.
/// Returns `None` if the key is invalid or the file does not exist.
/// Touches the file's mtime on hit to maintain LRU order.
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

/// Stores `data` under `key` atomically (tmp file + rename).
/// Validates the key, creates the cache directory if needed, updates mtime,
/// and evicts oldest entries when `MAX_CACHE_FILES` or `MAX_CACHE_BYTES` is exceeded.
/// Thread-safe via a global mutex; concurrent writers use unique tmp names.
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
/// Removes cached entries whose resource id is not in `keep_ids`.
/// Cache keys are `{id}_{width}_{height}.jpg`; the id is everything before
/// the trailing `_{width}_{height}.jpg` (parsed from the right, since
/// sanitized ids may themselves contain '_'). Files of unknown shape or with
/// invalid keys are kept (the LRU bounds in `put` still apply to them).
/// Thread-safe via the global mutex; returns `Ok(())` if the directory does not exist.
pub fn retain(cache_dir: &Path, keep_ids: &HashSet<String>) -> io::Result<()> {
    let _guard = CACHE_MUTEX.lock();
    let Ok(rd) = fs::read_dir(cache_dir) else {
        return Ok(());
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".tmp-") || !is_valid_key(&name) {
            continue;
        }
        let Some(id) = cache_key_id(&name) else {
            continue;
        };
        if !keep_ids.contains(id) {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

/// Extracts the resource id from a `{id}_{width}_{height}.jpg` cache key.
/// Returns `None` for keys of unknown shape, which callers keep.
fn cache_key_id(key: &str) -> Option<&str> {
    let base = key.strip_suffix(".jpg")?;
    let (rest, height) = base.rsplit_once('_')?;
    let (id, width) = rest.rsplit_once('_')?;
    if id.is_empty()
        || !width.chars().all(|c| c.is_ascii_digit())
        || !height.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(id)
}

/// Returns cache statistics as `(file_count, total_bytes)`, skipping `.tmp-` files.
/// Test-only helper for verifying LRU eviction bounds.
#[cfg(test)]
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
    #[test]
    fn retain_removes_only_unknown_ids() {
        // GIVEN a cache with entries for kept and removed ids
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        put(&cache, "keep1_10_10.jpg", b"a").unwrap();
        put(&cache, "keep1_0_0.jpg", b"b").unwrap();
        put(&cache, "gone9_10_10.jpg", b"c").unwrap();

        // WHEN retaining only keep1
        let keep: std::collections::HashSet<String> = ["keep1".to_string()].into_iter().collect();
        retain(&cache, &keep).unwrap();

        // THEN kept entries survive and unknown ids are removed
        assert!(get(&cache, "keep1_10_10.jpg").is_some());
        assert!(get(&cache, "keep1_0_0.jpg").is_some());
        assert_eq!(get(&cache, "gone9_10_10.jpg"), None);
    }
    #[test]
    fn retain_parses_id_from_right_for_underscored_ids() {
        // GIVEN entries with '_' inside the id and one of unknown shape
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        put(&cache, "we_ird_10_10.jpg", b"a").unwrap();
        put(&cache, "plain_10_10.jpg", b"b").unwrap();
        fs::write(cache.join("oddname.jpg"), b"c").unwrap();

        // WHEN retaining only the underscored id
        let keep: std::collections::HashSet<String> = ["we_ird".to_string()].into_iter().collect();
        retain(&cache, &keep).unwrap();

        // THEN the full underscored id is kept, others pruned, unknown kept
        assert!(get(&cache, "we_ird_10_10.jpg").is_some());
        assert_eq!(get(&cache, "plain_10_10.jpg"), None);
        assert!(cache.join("oddname.jpg").exists());
    }
}
