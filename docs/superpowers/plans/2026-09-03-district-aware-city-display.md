# District-aware hierarchical city display — Implementation Plan (Issue #209)

> **For agentic workers:** REQUIRED: Follow this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Each task is independently committable. No new network/API, no nightly, keep `rstar 0.12` + musl static.

**Goal:** Replace bare district name (`Bayenthal`) with hierarchical display (`Bayenthal, Köln`) by enriching offline `cities500` entries with `feature_code/country_code`, adding parent-city lookup (`PPLX → nearest PPL*`). Strictly two-level `District, City` or `City` — no country suffix, no env var.

**Architecture:** Extend `CityEntry` to hold 4 extra GeoNames columns (`feature_code`, `country_code`, `admin1`, `population`). Build two `RTree`s in `CityIndex { full_tree, parent_tree }` (second tree ~120k filtered entries, no string duplication beyond entries). `resolve_city_name` path: nearest `full_tree` (existing 20-nearest haversine + antimeridian probe) → if `PPLX` then second 20-nearest scan over `parent_tree` within `MAX_PARENT_DISTANCE_KM=30` → format `District, City`. Country code parsed but not rendered (future use).

**Tech Stack:** Rust stable, `rstar 0.12` (Euclidean `distance_2` + haversine post-filter), `r2d2 0.8 / r2d2_sqlite 0.35 / rusqlite 0.40 bundled`, `actix-web::web::block` for non-blocking load, `OnceLock` index, `regex/lazy_static` existing. No new env var, no new crate.

**Spec:** `.spec/district-aware-city-display.md` (covers FR-001…010, SC-001…005 — rev 2026-09-03: env var dropped).

---

## Global Constraints

- Parse extended columns: `feature_class` col 6, `feature_code` col 7, `country_code` col 8, `admin1` col 10, `population` col 14. Skip malformed with `warn!`, do not panic (FR-001).
- Subdivision predicate exactly `P:PPLX` in v1 (FR-002).
- Parent candidates: `feature_code ∈ {PPL, PPLA, PPLA2, PPLA3, PPLA4, PPLC, PPLG, PPLS}`; pick nearest haversine within 30 km from **photo** coordinate; fallback to district alone (FR-003).
- Formatting: strictly `District, City` or `City` — no country suffix (FR-004/005).
- **Remove persistent cache**: `geo_location_cache` is dropped via migration `04`; `get_city_name` becomes direct `resolve_city_name` passthrough (FR-006, Option 2).
- `BIGDATA_CLOUD_API_KEY` stays deprecated, `CITIES500_PATH` unchanged, no new env var (FR-007).
- Load via `web::block`, `<1 s / <50 MB`, parent scan ≤20 haversines, `p95 <1 ms` extra, no nightly/new heavy dep (FR-008). Removing cache reduces WAL writes.
- Observability: `info! "loaded N cities (M parents, K districts)"`, `debug!` for district→parent, migration `04` `info!` (FR-009).
- Existing 9 `resolve_*` tests stay green (updated to not expect cache); new tests off real `cities500.txt` + migration `04` drop check (FR-010).
Historical offline-migration decisions (heap <20 MB, `hash32/heapless/libm` only, nightly rejected) remain.

---

## File Structure

**New files:**
- `migrations/04-drop_geo_location_cache/up.sql` — `DROP TABLE IF EXISTS geo_location_cache;`

**Modified files:**
- `src/geo_location.rs` — `CityEntry` extension, `CityIndex { full_tree, parent_tree }`, `load_city_index` extended parsing, `MAX_PARENT_DISTANCE_KM`, `resolve_city_name` hierarchical.
- `src/resource_processor.rs` — delete `location_exists/get_location/add_location` usage; `get_city_name` → `geo_location::resolve_city_name(loc).await` (clean up `resource_store` helpers if unused elsewhere, keep table helpers for migration compatibility or delete).
- `src/resource_store.rs` — helpers `add_location/get_location/location_exists` can be removed or kept as dead code behind migration (prefer remove + update tests); `MIGRATIONS` auto-picks new `04`.
- `src/resource_processor_test.rs` — new district tests (`Bayenthal`, `Christianshavn`, `Volksdorf`), migration drop test.
- `migrations/01-initial/up.sql` — unchanged (still creates `geo_location_cache`, `04` drops it right after for idempotent fresh-vs-migrated).
- `README.md` — no env row; optional `BREAKING` note that `geo_location_cache` is auto-dropped (no manual `DELETE` needed).
- `CHANGELOG.md` — entry `feat: hierarchical District, City + drop persistent geo cache`.
- `Cargo.toml`/`Containerfile` — no change.

**No new files** otherwise. Keep diff minimal.
---

### Task 1: Extend CityEntry and loader (FR-001 + half FR-003)

**Files:**
- Modify: `src/geo_location.rs`

**Interfaces:**
- Consumes: `/cities500.txt` tab-delimited 19 cols.
- Produces: `CityEntry { name, lat, lon, feature_code, country_code, admin1_code, population }`, `CityIndex { full_tree, parent_tree }`, `load_city_index() -> Option<CityIndex>`.

- [ ] **Step 1: Read current `geo_location.rs` `CityEntry`, `load_city_index`, `CityIndex`, `resolve_city_name`.**
  Confirm `OnceLock<Option<CityIndex>>`, `web::block`, `RTree::bulk_load`, Euclidean `distance_2`, haversine clamp, antimeridian `alt_lon` probe.

- [ ] **Step 2: Write failing test for extended parsing (pre-change should fail to find fields).**
  ```rust
  #[actix_rt::test]
  async fn city_entry_has_country_and_feature() {
      // synthetic cities500 with PPLX Bayenthal + PPL Köln
      // assert load returns entry where feature_code=="PPLX" and country=="DE"
  }
  ```
  Run `cargo test city_entry_has_country_and_feature -- --nocapture` → expect FAIL (fields missing).

- [ ] **Step 3: Extend struct and loader.**
  ```rust
  struct CityEntry {
      name: String,
      lat: f64,
      lon: f64,
      feature_code: String,   // e.g. "PPLX", "PPL"
      country_code: String,   // e.g. "DE"
      admin1_code: String,
      population: i64,
  }
  // keep RTreeObject / PointDistance unchanged (lon/lat)
  struct CityIndex { full_tree: RTree<CityEntry>, parent_tree: RTree<CityEntry> }
  const MAX_PARENT_DISTANCE_KM: f64 = 30.0;
  ```
  In `load_city_index()` parse `cols[6]`=feature_class, `cols[7]`=feature_code, `cols[8]`=country, `cols[10]`=admin1, `cols[14]`=population. Require `cols.len() >= 15` else `warn!` skip; `name` trimmed empty → skip; `lat/lon` parse → skip on err. Defaults: `feature_code=""` if empty (treated as plain city), `country_code` 2-char uppercased, `population` 0 on parse err. Log:
  ```rust
  log::info!("loaded {} cities ({} parents, {} districts) from {}", len, parent_count, district_count, path);
  ```
  Build `parents: Vec<CityEntry>` clone/filter where `feature_code` ∈ parent set; `districts` count where `feature_code=="PPLX"`. Duplicate entries for parent_tree by cloning (cheap, ~120k). Alternative zero-copy `Vec<usize>` indices considered and rejected for simplicity and extra indirection cost. Heap delta estimate ~15 MB parents.

- [ ] **Step 4: Verify compilation and existing tests still pass (loader change only, no formatting yet).**
  ```bash
  cargo test resolve_koblenz resolve_amsterdam resolve_kottenheim -- --nocapture
  CITIES500_PATH=$(pwd)/cities500.txt cargo test -- --nocapture  # if local file present
  cargo check
  ```
  Expected: PASS (parent_tree built but not yet queried).

- [ ] **Step 5: Commit**
  ```bash
  git add src/geo_location.rs
  git commit -m "feat(geo): extend CityEntry with feature_code/country_code and split parent_tree"
  ```

---

### Task 2: Hierarchical resolution + formatting (FR-002, FR-003, FR-004/005)

**Files:**
- Modify: `src/geo_location.rs`

**Interfaces:**
- Consumes: `CityIndex` from Task 1.
- Produces: `resolve_city_name() -> Option<String>` returning `"District, City"` or `"City"`.

- [ ] **Step 1: Write failing tests for hierarchical behavior.**
  ```rust
  #[actix_rt::test]
  async fn resolve_bayenthal_returns_hierarchical() {
      // Bayenthal approx 50.9049, 6.9606
      let bayenthal = GeoLocation { latitude: 50.9049, longitude: 6.9606 };
      let name = geo_location::resolve_city_name(bayenthal).await.unwrap();
      // dataset-tolerant: if Bayenthal PPLX present → "Bayenthal, Köln", else fallback "Köln"
      assert!(name.contains("Köln"), "expected Köln in '{name}'");
      if name.contains("Bayenthal") { assert_eq!(name, "Bayenthal, Köln"); }
  }
  #[actix_rt::test]
  async fn resolve_koln_dom_plain() {
      let koln = GeoLocation { latitude: 50.941, longitude: 6.958 };
      assert_eq!(geo_location::resolve_city_name(koln).await, Some("Köln".into()));
  }
  #[actix_rt::test]
  async fn resolve_christianshavn_hierarchical() {
      let ch = GeoLocation { latitude: 55.676, longitude: 12.593 };
      let name = geo_location::resolve_city_name(ch).await.unwrap();
      assert!(name.contains("København"), "expected København in '{name}'");
  }
  ```
  Run → FAIL (still returns bare district).

- [ ] **Step 2: Implement `is_district` + `is_parent_city` helpers and parent lookup.**
  ```rust
  fn is_district(e: &CityEntry) -> bool { e.feature_code == "PPLX" }
  fn is_parent_city(code: &str) -> bool {
      matches!(code, "PPL"|"PPLA"|"PPLA2"|"PPLA3"|"PPLA4"|"PPLC"|"PPLG"|"PPLS")
  }
  ```
  After existing `best` selection (photo coordinate haversine within 50 km, antimeridian probe), add:
  ```rust
  let Some((best_entry, _)) = best else { return None };
  if !is_district(best_entry) {
      return Some(best_entry.name.clone());
  }
  // district → parent lookup
  let mut best_parent: Option<(&CityEntry, f64)> = None;
  for q in [point, alt_point] {
      for cand in index.parent_tree.nearest_neighbor_iter(&q).take(20) {
          let d = haversine_km(lat, lon, cand.lat, cand.lon);
          if d <= MAX_PARENT_DISTANCE_KM {
              match &best_parent { Some((_, bd)) if d >= *bd => {}, _ => best_parent = Some((cand, d)) }
          }
      }
  }
  if let Some((parent, dist)) = best_parent {
      log::debug!("district '{}' -> parent '{}' {:.1}km", best_entry.name, parent.name, dist);
      return Some(format!("{}, {}", best_entry.name, parent.name));
  }
  log::debug!("district '{}' has no parent within {}km", best_entry.name, MAX_PARENT_DISTANCE_KM);
  Some(best_entry.name.clone())
  ```

  **Edge:** Use photo coordinate for parent distance, not district centroid. Probe both `point` and `alt_point`. Do not fallback to far parent >30 km.

- [ ] **Step 3: Verify new tests pass, old tests still green.**
  ```bash
  cargo test resolve_bayenthal resolve_koln_dom_plain resolve_christianshavn resolve_koblenz resolve_amsterdam -- --nocapture
  ```
  Expected: PASS. `resolve_koblenz` (Koblenz is PPL, not PPLX) stays `Koblenz`. `resolve_kottenheim` etc. unchanged.

- [ ] **Step 4: Edge test — no parent within range.**
  Synthetic test constructing minimal `CityIndex` with single `PPLX` isolated >100 km from any parent, assert returns district alone.

- [ ] **Step 5: Commit**
  ```bash
  git add src/geo_location.rs src/resource_processor_test.rs
  git commit -m "feat(geo): hierarchical district display District, City with 30km parent lookup"
  ```
### Task 3: Drop persistent geo cache + docs (FR-006, FR-007, SC-005)

**Files:**
- Create: `migrations/04-drop_geo_location_cache/up.sql`
- Modify: `src/resource_processor.rs`, `src/resource_store.rs`, `README.md`, `CHANGELOG.md`, `src/resource_processor_test.rs`

**Interfaces:**
- Consumes: hierarchical string from Task 2.
- Produces: `geo_location_cache` absent after `MIGRATIONS.to_latest()`; `get_city_name` live lookup.

- [ ] **Step 1: Create migration `04`.**
  Create `migrations/04-drop_geo_location_cache/up.sql`:
  ```sql
  -- 04-drop_geo_location_cache: offline RTree <1ms, persistent cache obsolete + stale after hierarchical fix
  DROP TABLE IF EXISTS geo_location_cache;
  ```
  Verify `cargo test` picks it via `include_dir!` + `build.rs` (`rerun-if-changed=migrations/`).

- [ ] **Step 2: Remove cache usage in `src/resource_processor.rs`.**
  ```rust
  // Before:
  async fn get_city_name(resource: &ImageResource, store: &ResourceStore) -> Option<String> {
      let loc = resource.location?;
      let key = loc.to_string();
      if store.location_exists(&key) { store.get_location(&key) } else {
          let city = geo_location::resolve_city_name(loc).await?;
          store.add_location(key, city.clone()); Some(city)
      }
  }
  // After (Option 2):
  async fn get_city_name(resource: &ImageResource, _store: &ResourceStore) -> Option<String> {
      geo_location::resolve_city_name(resource.location?).await
  }
  ```
  Keep `_store` param for API compat or change call sites (`build_display_value` passes `&ResourceStore` — can keep unused arg to avoid churn). Remove imports if dead.

- [ ] **Step 3: Clean `src/resource_store.rs` helpers.**
  `pub fn add_location/get_location/location_exists` are now dead — delete them and any `geo_location_cache` references outside `migrations/01-initial`. If tests reference them, update to assert table absent instead:
  ```rust
  let cnt: i32 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='geo_location_cache'", [], |r| r.get(0)).unwrap();
  assert_eq!(cnt, 0);
  ```

- [ ] **Step 4: Dataset-tolerant test for Volksdorf.**
  ```rust
  #[actix_rt::test]
  async fn resolve_volksdorf_hierarchical() {
      let v = GeoLocation { latitude: 53.651, longitude: 10.166 };
      let name = geo_location::resolve_city_name(v).await.unwrap();
      assert!(name.contains("Hamburg"), "{name}");
  }
  #[test]
  fn migration_04_drops_geo_cache() {
      assert!(MIGRATIONS.validate().is_ok());
      let mut conn = Connection::open_in_memory().unwrap();
      conn.execute_batch("CREATE TABLE geo_location_cache (id TEXT PRIMARY KEY, value TEXT); PRAGMA user_version=3;").unwrap();
      MIGRATIONS.to_latest(&mut conn).unwrap();
      let cnt: i32 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='geo_location_cache'", [], |r| r.get(0)).unwrap();
      assert_eq!(cnt, 0);
  }
  ```

- [ ] **Step 5: Update README + CHANGELOG.**
  `README.md`: no new env; optional `> **BREAKING (auto):** `geo_location_cache` table is dropped automatically on next startup (offline lookup is <1ms, no manual \`DELETE\` needed).`  
  `CHANGELOG.md`: `feat!: hierarchical District, City (Bayenthal→Bayenthal, Köln) + drop persistent geo cache (auto-migrated)`

- [ ] **Step 6: Run full suite.**
  ```bash
  cargo fmt --all -- --check
  cargo clippy --all-targets
  cargo test -- --test-threads=1 --nocapture
  cargo test -- --nocapture # without cities500 should warn not panic
  ```

- [ ] **Step 7: Commit**
  ```bash
  git add migrations/04-drop_geo_location_cache/up.sql src/resource_processor.rs src/resource_store.rs README.md CHANGELOG.md src/resource_processor_test.rs
  git commit -m "feat!: drop persistent geo_location_cache, live RTree lookup for hierarchical display"
  ```

---

### Task 4: Integration & release readiness

**Files:** none (verification only)

- [ ] **Step 1: Download real `cities500` if absent and smoke test.**
  ```bash
  curl -fL https://download.geonames.org/export/dump/cities500.zip -o /tmp/cities500.zip && unzip -p /tmp/cities500.zip > /tmp/cities500.txt
  wc -l /tmp/cities500.txt && grep -c $'\tP\tPPLX\t' /tmp/cities500.txt
  CITIES500_PATH=/tmp/cities500.txt cargo test resolve_bayenthal_returns_hierarchical resolve_christianshavn_hierarchical resolve_volksdorf_hierarchical migration_04_drops_geo_cache -- --nocapture --test-threads=1
  ```

- [ ] **Step 2: Build and check size.**
  ```bash
  cargo build --release
  ls -lh target/release/this-week-in-past
  ```

- [ ] **Step 3: Verify migration 04 on both fresh and migrated DBs.**
  ```bash
  # Fresh
  rm -rf /tmp/twip_test && mkdir -p /tmp/twip_test && DATA_FOLDER=/tmp/twip_test CITIES500_PATH=/tmp/cities500.txt cargo test -- --nocapture
  sqlite3 /tmp/twip_test/resources.db "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='geo_location_cache'; SELECT user_version FROM pragma_user_version;"
  # Migrated (seed old cache)
  sqlite3 /tmp/twip_test/resources.db "CREATE TABLE IF NOT EXISTS geo_location_cache(id TEXT PRIMARY KEY,value TEXT); INSERT OR REPLACE INTO geo_location_cache VALUES('x','old');"
  # restart app/test → table gone
  ```

- [ ] **Step 4: Final `cargo fmt` + `clippy` + `cargo test` green, prepare PR description referencing #209 and linking spec/plan.**
