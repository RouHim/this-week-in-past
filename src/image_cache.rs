use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub const MAX_CACHE_FILES: usize = 500;
pub const MAX_CACHE_BYTES: u64 = 1_073_741_824;

pub fn cache_dir(data_folder: &str) -> PathBuf {
    PathBuf::from(data_folder).join("cache")
}

pub fn get(cache_dir: &Path, key: &str) -> Option<Vec<u8>> {
    let path = cache_dir.join(key);
    let data = fs::read(&path).ok()?;
    let now = filetime::FileTime::now();
    let _ = filetime::set_file_mtime(&path, now);
    Some(data)
}

pub fn put(cache_dir: &Path, key: &str, data: &[u8]) -> io::Result<()> {
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
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
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
    let now = filetime::FileTime::now();
    let _ = filetime::set_file_mtime(&dest, now);
    evict_if_needed(cache_dir);
    Ok(())
}

pub fn clear(cache_dir: &Path) -> io::Result<()> {
    if !cache_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(cache_dir)? {
        let entry = entry?;
        let _ = fs::remove_file(entry.path());
    }
    Ok(())
}

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
    let mut entries: Vec<(PathBuf, filetime::FileTime, u64)> = vec![];
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
        let mtime = filetime::FileTime::from_last_modification_time(&md);
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
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        put(&cache, "abc_100_100.jpg", b"hello").unwrap();
        assert_eq!(get(&cache, "abc_100_100.jpg"), Some(b"hello".to_vec()));
    }
    #[test]
    fn lru_evicts_oldest_when_over_count() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        for i in 0..501 {
            put(&cache, &format!("k{i}_10_10.jpg"), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let (count, _) = cache_stats(&cache);
        assert!(count <= 500, "count {}", count);
        assert_eq!(get(&cache, "k0_10_10.jpg"), None);
        assert!(get(&cache, "k500_10_10.jpg").is_some());
    }
    #[test]
    fn missing_entry_is_cache_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        assert_eq!(get(&cache, "nope.jpg"), None);
    }
    #[test]
    fn concurrent_put_no_corruption() {
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
        let (count, bytes) = cache_stats(&cache);
        assert_eq!(count, 20);
        assert_eq!(bytes, 20 * 100);
    }
}
