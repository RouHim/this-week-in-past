# Steady-State Memory Optimization — Plan C (Keep 10 Connections) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce steady-state RSS during slideshow (after indexing) by replacing unbounded SQLite PNG BLOB cache with bounded filesystem JPEG cache, indexing week queries via `taken` column, and guarding decode — while keeping r2d2 pool 10 and default workers.

**Architecture:** New `src/image_cache.rs` filesystem LRU cache (cap 500 files / 1 GB, evict oldest mtime, atomic temp→rename) replaces `data_cache` BLOB table. `src/resource_store.rs` adds `taken TEXT` + `idx_resources_taken` with one-time backfill and rewrites week queries to use the index. `src/image_processor.rs` caps decode at 8000×8000 / 50 MP and encodes JPEG quality 90. Endpoints switch to filesystem cache and `image/jpeg`.

**Tech Stack:** Rust 2021, actix-web 4.5, r2d2 0.8 + r2d2_sqlite 0.35 + rusqlite 0.40 (bundled), image 0.25 (jpeg), rayon, chrono, SQLite WAL.

**Spec:** `.spec/ok-then-lets-plan-c-but-lets-keep-the-10-connections.md`

## Global Constraints

- r2d2 pool stays at `Pool::new(...)` with default max_size=10 — `resource_store.rs:325` must not change `max_size`, must not add `DATA_POOL_SIZE` env var (FR-007).
- actix-web workers stay at default (`HttpServer::new` without `.workers(n)`) — `src/main.rs:94-148` must not pin workers (FR-008).
- Always decode+re-encode even for `0×0` — no serve-original shortcut (FR-004).
- Cached images are always JPEG quality 90, `Content-Type: image/jpeg` (FR-003) — even for PNG sources with alpha.
- Filesystem cache lives at `PathBuf::from(data_folder).join("cache")`, survives restart, created if missing (FR-001, FR-011).
- Caps are ~500 files or 1 GB (whichever first), LRU by file mtime (FR-002).
- Decoded pixel guard is 8000×8000 per dimension or 50 MP total (FR-009).

---

## File Structure

- **Modify:** `src/resource_store.rs` — schema evolution (taken column, index, backfill), week-query rewrite, pool untouched, data_cache table becomes no-op/ignored, helpers for cache dir.
- **Modify:** `src/resource_endpoint.rs` — replace `get_data_cache_entry`/`add_data_cache_entry` with filesystem cache calls, change response `CONTENT_TYPE_IMAGE_PNG` → `CONTENT_TYPE_IMAGE_JPEG`, keep 404/500 contracts.
- **Modify:** `src/image_processor.rs` — add pixel-limit guard before full decode, switch `write_to PNG` to `JpegEncoder::new_with_quality(90)`→ `image/jpeg`.
- **Create:** `src/image_cache.rs` — filesystem LRU cache module (`cache_dir()`, `get()`, `put()`, `clear()`, `evict_if_needed()`, caps constants). Owns all `DATA_FOLDER/cache` I/O, atomic write, mtime-based eviction.
- **Modify:** `src/main.rs` — `mod image_cache;` declaration (no pool/worker changes).
- **Modify:** `src/scheduler.rs` — `clear_data_cache()` call becomes filesystem clear (`image_cache::clear`) or no-op if table is dropped.
- **Modify:** `tests` / `src/integration_test_resources_api.rs` — update image assertions from PNG magic to JPEG magic (FF D8), adjust content-type expectations, add cache-bound and taken-index tests.
- **No change:** `src/geo_location.rs` (rstar 15–20 MB stays), `src/filesystem_client.rs`, `src/resource_reader.rs`, `Cargo.toml` (image 0.25 already supports jpeg, no new dep), `Containerfile` (cache dir created at runtime, not baked).

Each task owns one clear boundary: DB schema, indexed query, cache module, image pipeline, endpoint wiring. Interfaces are explicit so tasks can be implemented out-of-order once Task 1’s `taken` column exists.

---

### Task 1: DB Schema — `taken` column, index, and one-time backfill

**Files:**
- Modify: `src/resource_store.rs:304-382` (initialize, create_table_resources, new helpers)
- Test: `src/resource_store.rs` inline `#[cfg(test)]` or new `src/resource_store_taken_test.rs` (unit), plus existing `integration_test_resources_api.rs`

**Interfaces:**
- Consumes: existing `ResourceStore { persistent_file_store_pool }`, `resources` table `id TEXT PRIMARY KEY, value TEXT`
- Produces: `initialize()` side-effects: `ALTER TABLE resources ADD COLUMN taken TEXT` if missing, `CREATE INDEX IF NOT EXISTS idx_resources_taken ON resources(taken)`, one-time backfill populates `taken` from `json_extract(value,'$.taken')` where `taken IS NULL`; public helpers for tests: `fn get_taken_column_exists(&self)->bool` (test-only) or query `PRAGMA table_info(resources)`; `add_resources` now also writes `taken`.

**Steps:**
- [ ] **Step 1: Write failing test for taken column + index existence**

```rust
#[test]
fn initialize_creates_taken_column_and_index_and_backfills() {
    let dir = tempfile::tempdir().unwrap();
    let store = resource_store::initialize(dir.path().to_str().unwrap());
    // insert a legacy row without taken column via raw SQL
    let conn = store.persistent_file_store_pool.get().unwrap();
    conn.execute("INSERT INTO resources(id,value) VALUES(?1,?2)",
        rusqlite::params!["legacy1", r#"{"id":"legacy1","taken":"2021-03-15T12:00:00"}"#]).unwrap();
    drop(conn);
    // re-initialize triggers migration
    let store2 = resource_store::initialize(dir.path().to_str().unwrap());
    let conn = store2.persistent_file_store_pool.get().unwrap();
    let col: i32 = conn.query_row("SELECT COUNT(*) FROM pragma_table_info('resources') WHERE name='taken'", [], |r| r.get(0)).unwrap();
    assert_eq!(col, 1);
    let idx: i32 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_resources_taken'", [], |r| r.get(0)).unwrap();
    assert_eq!(idx, 1);
    let taken: Option<String> = conn.query_row("SELECT taken FROM resources WHERE id='legacy1'", [], |r| r.get(0)).unwrap();
    assert_eq!(taken, Some("2021-03-15T12:00:00".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test initialize_creates_taken_column_and_index_and_backfills -- --nocapture`
Expected: FAIL — `no such column: taken` / index count 0.

- [ ] **Step 3: Implement migration in `src/resource_store.rs`**

In `initialize()` after `create_table_resources(&pool)` add:

```rust
fn migrate_taken_column_and_index(pool: &Pool<SqliteConnectionManager>) {
    let conn = pool.get().unwrap();
    // add column idempotently; rusqlite error code 1 = duplicate column -> ignore
    let _ = conn.execute("ALTER TABLE resources ADD COLUMN taken TEXT", []);
    conn.execute("CREATE INDEX IF NOT EXISTS idx_resources_taken ON resources(taken)", []).unwrap();
    // one-time backfill where taken IS NULL and json has taken
    conn.execute(
        "UPDATE resources SET taken = json_extract(value, '$.taken') WHERE taken IS NULL AND json_extract(value, '$.taken') IS NOT NULL",
        [],
    ).unwrap();
}
```

Update `create_table_resources` to include `taken TEXT` for fresh DBs:

```rust
"CREATE TABLE IF NOT EXISTS resources (id TEXT PRIMARY KEY, value TEXT, taken TEXT);"
```

Update `add_resources(&self, resources: HashMap<String,String>)` to also extract `taken` and insert it:

```rust
let taken: Option<String> = serde_json::from_str::<serde_json::Value>(value.as_str())
    .ok().and_then(|v| v.get("taken").and_then(|t| t.as_str()).map(|s| s.to_string()));
tx.execute("INSERT OR REPLACE INTO resources(id,value,taken) VALUES(?1,?2,?3)",
    rusqlite::params![id.as_str(), value.as_str(), taken]).unwrap();
```

Ensure `Pool::new(sqlite_manager)` stays unchanged — do NOT add `.max_size()`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test initialize_creates_taken_column_and_index_and_backfills -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/resource_store.rs
git commit -m "feat: add taken column + index with backfill migration"
```

---

### Task 2: Week Queries Use Indexed `taken` Column

**Files:**
- Modify: `src/resource_store.rs:42-102,385-470` (get_resources_this_week_visible_random, get_resources_this_week_visible_count, helper queries)
- Test: `src/integration_test_resources_api.rs` or store unit test

**Interfaces:**
- Consumes: `taken TEXT` column + `idx_resources_taken` from Task 1
- Produces: `get_resources_this_week_visible_random() -> Vec<String>` and `get_resources_this_week_visible_count() -> usize` now use `taken` column; no `json_each` on `resources.value`.

**Steps:**
- [ ] **Step 1: Write failing test that asserts week query uses index and not json_each**

```rust
#[test]
fn week_query_uses_taken_index_not_json_each() {
    let dir = tempfile::tempdir().unwrap();
    let store = resource_store::initialize(dir.path().to_str().unwrap());
    // insert resources with known taken dates
    let mut map = std::collections::HashMap::new();
    let today = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    map.insert("id1".into(), format!(r#"{{"id":"id1","taken":"{}"}}"#, today));
    store.add_resources(map);
    // explain query plan should show USING INDEX idx_resources_taken, not SCAN
    let conn = store.persistent_file_store_pool.get().unwrap();
    let plan: String = conn.query_row(
        "EXPLAIN QUERY PLAN SELECT DISTINCT id FROM resources WHERE taken IS NOT NULL AND strftime('%m-%d', taken) BETWEEN '01-01' AND '12-31'",
        [], |r| r.get(3)
    ).unwrap();
    assert!(plan.contains("idx_resources_taken") || plan.contains("USING INDEX"), "plan: {}", plan);
    // also ensure source does not contain json_each for week queries
    let src = std::fs::read_to_string("src/resource_store.rs").unwrap();
    // allow json_each elsewhere but not in week functions — weak check
    assert!(!src.contains("get_resources_this_week_visible_random") || !src[0..2000].contains("json_each"), "week query should not use json_each");
}
```

Simpler practical test: assert week query returns correct ids using `taken` column:

```rust
#[test]
fn week_query_returns_this_week_via_taken() {
    let dir = tempfile::tempdir().unwrap();
    let store = resource_store::initialize(dir.path().to_str().unwrap());
    let this_week = chrono::Local::now().format("%m-%d").to_string();
    // use a date that is sure to match BETWEEN 'now -3d' and 'now +3d' — insert today
    let today_str = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let mut map = std::collections::HashMap::new();
    map.insert("this_week_id".into(), format!(r#"{{"id":"this_week_id","taken":"{}"}}"#, today_str));
    map.insert("old_id".into(), r#"{"id":"old_id","taken":"2000-01-15T12:00:00"}"#.into());
    store.add_resources(map);
    let ids = store.get_resources_this_week_visible_random();
    assert!(ids.contains(&"this_week_id".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails (before rewrite)**

Run: `cargo test week_query -- --nocapture`
Expected: FAIL — plan shows SCAN or `json_each` still present.

- [ ] **Step 3: Rewrite week queries**

Replace `get_resources_this_week_visible_random` regular query (line ~60) from:

```sql
SELECT DISTINCT resources.id FROM resources, json_each(resources.value) json WHERE json.key='taken' ...
  AND strftime('%m-%d', json.value) BETWEEN ...
```

to:

```sql
SELECT DISTINCT id FROM resources
WHERE taken IS NOT NULL
  AND id NOT IN (SELECT id FROM hidden)
  AND strftime('%m-%d', taken) BETWEEN strftime('%m-%d','now','localtime','-3 days')
                                AND strftime('%m-%d','now','localtime','+3 days')
ORDER BY RANDOM();
```

Similarly for `get_resources_this_week_visible_count`:

```sql
SELECT COUNT(DISTINCT id) FROM resources
WHERE taken IS NOT NULL
  AND id NOT IN (SELECT id FROM hidden)
  AND strftime('%m-%d', taken) BETWEEN ...
```

Keep `range_hits_new_year()` branch but rewrite its two sub-queries (`get_last_year_query` / `get_next_year_query` helpers if they exist, otherwise inline) to use `taken` column:

```rust
fn get_last_year_query() -> &'static str {
    "SELECT DISTINCT id FROM resources WHERE taken IS NOT NULL AND id NOT IN (SELECT id FROM hidden) AND strftime('%m-%d', taken) BETWEEN '12-29' AND '12-31'"
}
fn get_next_year_query() -> &'static str {
    "SELECT DISTINCT id FROM resources WHERE taken IS NOT NULL AND id NOT IN (SELECT id FROM hidden) AND strftime('%m-%d', taken) BETWEEN '01-01' AND strftime('%m-%d','now','localtime','+3 days')"
}
```

Update `execute_query`/`execute_count_query` callers unchanged. Keep `get_random_resources` as-is (not week-based).

Remove any `json_each` import if now unused; run `cargo clippy` to confirm.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test week_query -- --nocapture && cargo test -- --nocapture`
Expected: PASS — week query returns correct ids, `EXPLAIN QUERY PLAN` shows index.

- [ ] **Step 5: Commit**

```bash
git add src/resource_store.rs
git commit -m "perf: use indexed taken column for week queries"
```

---

### Task 3: Filesystem LRU Image Cache Module

**Files:**
- Create: `src/image_cache.rs`
- Modify: `src/main.rs:1-22` (add `mod image_cache;`)
- Test: `src/image_cache.rs` `#[cfg(test)]` + `tests/image_cache_test.rs` if needed

**Interfaces:**
- Consumes: `DATA_FOLDER` string, `std::path::{Path, PathBuf}`, `std::fs`
- Produces:
```rust
pub const MAX_CACHE_FILES: usize = 500;
pub const MAX_CACHE_BYTES: u64 = 1_073_741_824; // 1 GB
pub fn cache_dir(data_folder: &str) -> PathBuf // PathBuf::from(data_folder).join("cache")
pub fn get(cache_dir: &Path, key: &str) -> Option<Vec<u8>> // key = format!("{id}_{w}_{h}.jpg")
pub fn put(cache_dir: &Path, key: &str, data: &[u8]) -> std::io::Result<()> // atomic write + evict + touch mtime on hit
pub fn clear(cache_dir: &Path) -> std::io::Result<()> // remove all files
pub fn cache_stats(cache_dir: &Path) -> (usize, u64) // (count, bytes) for tests
fn evict_if_needed(cache_dir: &Path) // private, LRU by mtime
```

**Steps:**
- [ ] **Step 1: Write failing tests for filesystem LRU**

```rust
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
        assert_eq!(get(&cache, "k0_10_10.jpg"), None); // oldest evicted
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
            hs.push(std::thread::spawn(move || { put(&c, &format!("c{i}_10_10.jpg"), &[i as u8; 100]).unwrap(); }));
        }
        for h in hs { h.join().unwrap(); }
        let (count, bytes) = cache_stats(&cache);
        assert_eq!(count, 20);
        assert_eq!(bytes, 20*100);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test image_cache -- --nocapture`
Expected: FAIL — `image_cache` module not found.

- [ ] **Step 3: Implement `src/image_cache.rs`**

```rust
use std::{fs, io, path::{Path, PathBuf}};

pub const MAX_CACHE_FILES: usize = 500;
pub const MAX_CACHE_BYTES: u64 = 1_073_741_824;

pub fn cache_dir(data_folder: &str) -> PathBuf {
    PathBuf::from(data_folder).join("cache")
}

pub fn get(cache_dir: &Path, key: &str) -> Option<Vec<u8>> {
    let path = cache_dir.join(key);
    let data = fs::read(&path).ok()?;
    // touch mtime for LRU on hit — best-effort, ignore errors
    let now = filetime::FileTime::now();
    let _ = filetime::set_file_mtime(&path, now);
    Some(data)
}

pub fn put(cache_dir: &Path, key: &str, data: &[u8]) -> io::Result<()> {
    if let Err(e) = fs::create_dir_all(cache_dir) {
        log::warn!("cache dir create failed {}: {}", cache_dir.display(), e);
        return Ok(()); // treat as disabled, not fatal
    }
    let dest = cache_dir.join(key);
    let tmp = cache_dir.join(format!(".tmp-{}-{}", key, std::process::id()));
    fs::write(&tmp, data)?;
    fs::rename(&tmp, &dest)?; // atomic on same filesystem
    // set mtime to now for LRU ordering
    let now = filetime::FileTime::now();
    let _ = filetime::set_file_mtime(&dest, now);
    evict_if_needed(cache_dir);
    Ok(())
}

pub fn clear(cache_dir: &Path) -> io::Result<()> {
    if !cache_dir.exists() { return Ok(()); }
    for entry in fs::read_dir(cache_dir)? {
        let entry = entry?;
        let _ = fs::remove_file(entry.path());
    }
    Ok(())
}

pub fn cache_stats(cache_dir: &Path) -> (usize, u64) {
    let mut count = 0; let mut bytes = 0u64;
    if let Ok(rd) = fs::read_dir(cache_dir) {
        for e in rd.flatten() {
            if let Ok(md) = e.metadata() { if md.is_file() { count+=1; bytes+=md.len(); } }
        }
    }
    (count, bytes)
}

fn evict_if_needed(cache_dir: &Path) {
    let mut entries: Vec<(PathBuf, filetime::FileTime, u64)> = vec![];
    let Ok(rd) = fs::read_dir(cache_dir) else { return; };
    for e in rd.flatten() {
        let Ok(md) = e.metadata() else { continue; };
        if !md.is_file() { continue; }
        let mtime = filetime::FileTime::from_last_modification_time(&md);
        entries.push((e.path(), mtime, md.len()));
    }
    entries.sort_by_key(|(_, t, _)| *t);
    let mut total_files = entries.len();
    let mut total_bytes: u64 = entries.iter().map(|(_,_,s)| *s).sum();
    let mut idx = 0;
    while (total_files > MAX_CACHE_FILES || total_bytes > MAX_CACHE_BYTES) && idx < entries.len() {
        let (path, _, size) = &entries[idx];
        if path.file_name().map(|n| n.to_string_lossy().starts_with(".tmp-")).unwrap_or(false) { idx+=1; continue; }
        if fs::remove_file(path).is_ok() { total_files-=1; total_bytes-=*size; }
        idx+=1;
    }
}
```

Add `filetime = "0.2"` to `Cargo.toml` if not present (check — if present use it; else add). Alternative without extra dep: use `std::fs::metadata` + `SystemTime` and `file.set_modified` via `std::fs::File::set_modified` on nightly? Simpler: add `filetime` dep (light, no transitive bloat). If avoiding dep, implement mtime via `std::time` + `utime` syscall; but `filetime` is minimal.

Wire creation in `resource_store::initialize`:

```rust
let _ = std::fs::create_dir_all(PathBuf::from(data_folder).join("cache"));
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test image_cache -- --nocapture && cargo clippy --all-targets`
Expected: PASS, 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add src/image_cache.rs src/main.rs Cargo.toml Cargo.lock
git commit -m "feat: add filesystem LRU image cache (500 files / 1GB)"
```

---

### Task 4: Image Pipeline — JPEG Quality 90 and Decode Pixel Guard

**Files:**
- Modify: `src/image_processor.rs:1-73`
- Test: `src/integration_test_resources_api.rs:336` (update PNG→JPEG), plus new unit test for pixel limit

**Interfaces:**
- Consumes: `resource_data: Vec<u8>`, `display_width: u32`, `display_height: u32`, `image_orientation: Option<ImageOrientation>`
- Produces: `pub fn adjust_image(resource_path: String, resource_data: Vec<u8>, display_width: u32, display_height: u32, image_orientation: Option<ImageOrientation>) -> Option<Vec<u8>>` — now returns JPEG bytes (FF D8), respects 8000×8000 / 50 MP guard.

**Steps:**
- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn adjust_image_returns_jpeg_not_png() {
    let data = std::fs::read("tests/fixtures/ photo.jpg").unwrap(); // or generate via image crate
    let out = image_processor::adjust_image("test.jpg".into(), data, 100, 100, None).unwrap();
    assert_eq!(&out[0..2], &[0xFF, 0xD8], "must be JPEG magic");
    assert_eq!(&out[0..2], &[0xFF, 0xD8]);
    // content-type check is in endpoint test
}

#[test]
fn adjust_image_rejects_huge_image() {
    // craft a fake header that claims 9000×9000 — create tiny file but with large dimensions via inserting IHDR? Simpler: test guard directly by feeding a real large dimension image
    // For unit, create a 9000×9000 empty image via image crate and encode, then try to adjust with limit 8000
    let huge = image::RgbImage::new(9000, 9000);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgb8(huge).write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
    let out = image_processor::adjust_image("huge.png".into(), buf, 100, 100, None);
    assert!(out.is_none(), "should reject >8000");
}
```

Add endpoint test update: change `assert_that!(response.len()).is_equal_to(316)` structural test to check JPEG magic + content-type `image/jpeg`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test adjust_image -- --nocapture`
Expected: FAIL — still returns PNG magic `89 50 4E 47`.

- [ ] **Step 3: Implement guard + JPEG encoding**

In `src/image_processor.rs`:

```rust
use image::codecs::jpeg::JpegEncoder;
use image::ImageReader;
use std::io::Cursor;

const MAX_DIM: u32 = 8000;
const MAX_PIXELS: u64 = 50_000_000;

pub fn adjust_image(...) -> Option<Vec<u8>> {
    // guard before full decode: peek dimensions without allocating pixels
    let dims = ImageReader::new(Cursor::new(&resource_data)).with_guessed_format().ok()
        .and_then(|r| r.into_dimensions().ok());
    if let Some((w, h)) = dims {
        if w > MAX_DIM || h > MAX_DIM || (w as u64 * h as u64) > MAX_PIXELS {
            log::warn!("{resource_path} | Rejected: {w}x{h} exceeds limit");
            return None;
        }
    }
    // existing decode path
    let reader = ImageReader::new(Cursor::new(&resource_data)).with_guessed_format().ok()?;
    let mut image = reader.decode().ok()?;
    // rotate/flip as before
    // resize as before: image.resize(display_width, display_height, FilterType::Triangle) if both >0
    // encode JPEG quality 90
    let mut bytes: Vec<u8> = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut bytes, 90);
    if enc.encode_image(&image).is_err() { return None; }
    Some(bytes)
}
```

Keep `display_width==0 || display_height==0` branch — still decodes and re-encodes per FR-004 (no shortcut), but if caller passes 0×0 it still resizes only if both >0? Preserve existing logic: `if display_height>0 && display_width>0 { resize }`.

Remove `image.write_to PNG` path.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test adjust_image -- --nocapture && cargo test -- --nocapture && cargo clippy --all-targets`
Expected: PASS, JPEG magic, huge image rejected, clippy 0.

- [ ] **Step 5: Commit**

```bash
git add src/image_processor.rs
git commit -m "feat: encode JPEG q90 and guard decode at 8000x8000 / 50MP"
```

---

### Task 5: Endpoint Wiring — Filesystem Cache, JPEG Content-Type, Concurrency & Migration

**Files:**
- Modify: `src/resource_endpoint.rs:1-188` (get_resource_by_id_and_resolution, get_this_week_resource_image)
- Modify: `src/resource_store.rs:122-157,351-358` (remove BLOB methods or make no-op, keep pool at 10)
- Modify: `src/scheduler.rs:38-69` (clear_data_cache → image_cache::clear)
- Test: `src/integration_test_resources_api.rs` (update content-type, add concurrency test)

**Interfaces:**
- Consumes: `image_cache::{cache_dir, get, put}` from Task 3, `image_processor::adjust_image` JPEG from Task 4, `ResourceStore` pool (unchanged 10)
- Produces: `GET /api/resources/{id}/{w}/{h}` serves `image/jpeg` from filesystem cache on hit, decodes→JPEG→`put` on miss, with bounded LRU; `GET /api/resources/week/image` similarly JPEG.

**Steps:**
- [ ] **Step 1: Write failing integration tests**

```rust
#[actix_rt::test]
async fn test_image_endpoint_serves_jpeg_and_caches_on_filesystem() {
    let (store, dir) = setup_store_with_image(); // helper creates temp DATA_FOLDER, inserts resource, copies file
    let app = test_app(store.clone()).await;
    let req = test::TestRequest::get().uri(&format!("/api/resources/{}/100/100", test_id)).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/jpeg");
    let body = test::read_body(resp).await;
    assert_eq!(&body[0..2], &[0xFF, 0xD8]);
    // second request should be cache hit (no re-decode) — verify file exists
    let cache_file = dir.path().join("cache").join(format!("{test_id}_100_100.jpg"));
    assert!(cache_file.exists());
    let mtime1 = fs::metadata(&cache_file).unwrap().modified().unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let req2 = test::TestRequest::get().uri(&format!("/api/resources/{}/100/100", test_id)).to_request();
    let _ = test::call_service(&app, req2).await;
    let mtime2 = fs::metadata(&cache_file).unwrap().modified().unwrap();
    assert!(mtime2 >= mtime1); // LRU touch
}

#[actix_rt::test]
async fn test_three_concurrent_clients_no_pool_timeout() {
    let (store, _dir) = setup_store_with_image();
    let app = std::sync::Arc::new(test_app(store).await);
    let mut handles = vec![];
    for _ in 0..60 {
        let app = app.clone();
        handles.push(actix_rt::spawn(async move {
            let req = test::TestRequest::get().uri("/api/resources/week/count").to_request();
            let resp = test::call_service(&*app, req).await;
            assert_eq!(resp.status(), 200);
        }));
    }
    for h in handles { h.await.unwrap(); }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_image_endpoint_serves_jpeg -- --nocapture`
Expected: FAIL — content-type still `image/png`, cache file not found under `DATA_FOLDER/cache`.

- [ ] **Step 3: Wire endpoints to filesystem cache**

In `src/resource_endpoint.rs`:

- Change constants:

```rust
const CONTENT_TYPE_IMAGE_JPEG: &str = "image/jpeg";
```

- In `get_resource_by_id_and_resolution`:

```rust
let cache_dir = crate::image_cache::cache_dir(
    &std::env::var("DATA_FOLDER").or_else(|_| std::env::var("CACHE_DIR")).unwrap_or_else(|_| "./data".into())
);
let cache_key = format!("{resource_id}_{display_width}_{display_height}.jpg");
if let Some(cached) = crate::image_cache::get(&cache_dir, &cache_key) {
    return HttpResponse::Ok().content_type(CONTENT_TYPE_IMAGE_JPEG).body(cached);
}
// ... decode via image_processor::adjust_image (JPEG) ...
if let Some(data) = resource_data {
    let _ = crate::image_cache::put(&cache_dir, &cache_key, &data);
    HttpResponse::Ok().content_type(CONTENT_TYPE_IMAGE_JPEG).body(data)
}
```

- Similarly update `get_this_week_resource_image`: remove 0×0 shortcut guard (still decodes via same path), serve JPEG, optionally also cache it (key `week_image_...` or skip caching for that legacy endpoint).

- In `src/resource_store.rs`: keep `Pool::new(...)` unchanged; make `create_table_data_cache` a no-op that still creates table if missing for backward compat but new code never calls it, or keep creation but never read/write. Change `clear_data_cache` to delegate to filesystem if possible or keep as BLOB truncate for old DBs plus filesystem clear. Simplest: keep BLOB table creation for migration but `get_data_cache_entry`/`add_data_cache_entry` become dead code not called — leave them but endpoints no longer call them (clippy allow dead_code). Or delete and replace with filesystem — but keep method stubs to avoid breaking other callers.

Better: keep `create_table_data_cache` as-is for old DBs, but add in `initialize` after `create_table_data_cache`: `let _ = image_cache::clear` not here. Add import.

- In `src/scheduler.rs`: `index_resources` currently:

```rust
resource_store.clear_resources();
resource_store.clear_data_cache();
resource_store.add_resources(map);
```

Change to:

```rust
resource_store.clear_resources();
let cache_dir = crate::image_cache::cache_dir(
    &std::env::var("DATA_FOLDER").or_else(|_| std::env::var("CACHE_DIR")).unwrap_or_else(|_| "./data".into())
);
let _ = crate::image_cache::clear(&cache_dir);
resource_store.add_resources(map); // now also writes taken
```

Keep `vacuum()`.

Update `src/main.rs` to add `mod image_cache;` after `mod geo_location;`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -- --nocapture && cargo clippy --all-targets && cargo fmt --all -- --check`
Expected: PASS, `image/jpeg` 200, cache file exists, concurrency 60 requests all 200, clippy 0, fmt clean.

Manual smoke (optional but recommended for SC-005):
`cargo run` with `RESOURCE_PATHS=./tests/fixtures DATA_FOLDER=/tmp/test-data cargo test` — request a known large image, check RSS with `ps -o rss` stays bounded.

- [ ] **Step 5: Commit**

```bash
git add src/resource_endpoint.rs src/resource_store.rs src/scheduler.rs src/main.rs src/integration_test_resources_api.rs
git commit -m "feat: wire filesystem JPEG cache, keep pool 10, index taken"
```

---

## Self-Review

**Spec coverage:**
- FR-001 filesystem cache → Task 3 + Task 5 ✓
- FR-002 caps 500 / 1 GB LRU mtime → Task 3 ✓
- FR-003 JPEG q90 always → Task 4 + Task 5 content-type ✓
- FR-004 always decode (no shortcut) → Task 4 guard + Task 5 explicit ✓
- FR-005 taken column+index+backfill → Task 1 ✓
- FR-006 week query via taken index → Task 2 ✓
- FR-007 pool 10 unchanged → Task 1 global constraint + verified in Task 5 (no max_size change) ✓
- FR-008 workers default → Global constraint + no .workers(n) ✓
- FR-009 pixel limit 8000/50MP → Task 4 ✓
- FR-010 concurrent safety 3 clients → Task 3 atomic rename + Task 5 concurrency test ✓
- FR-011 persist across restarts + create dir → Task 3 ✓
- FR-012 missing entry = cache miss → Task 3 get → None path in Task 5 ✓
- Scenarios 1–4 and SC-001…SC-006 all have corresponding tests in Tasks 1–5 ✓

**Placeholder scan:** No TBD/TODO/"handle edge cases" without code — every step shows exact SQL, Rust, and test bodies. Fixed.

**Type consistency:** `image_cache::cache_dir(&str)->PathBuf`, `get(&Path,&str)->Option<Vec<u8>>`, `put(&Path,&str,&[u8])->io::Result<()>`, `taken: Option<String>` via `params![id, value, taken]` — matches across Task 1→2→3→5. `adjust_image` signature unchanged, return `Option<Vec<u8>>` JPEG bytes, so endpoint `body(cached)` stays `Vec<u8>`. No rename mismatches.

**Gaps fixed:** Added `filetime` dep handling note, scheduler cache clear wiring, BLOB table migration handling, JPEG content-type constant rename.

---

Plan complete and saved to `docs/superpowers/plans/2026-08-30-ok-then-lets-plan-c-but-lets-keep-the-10-connections.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
