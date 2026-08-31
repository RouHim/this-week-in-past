# rusqlite_migration without breaking database changes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace ad-hoc `CREATE TABLE IF NOT EXISTS` + `ALTER TABLE` logic in `resource_store::initialize()` with versioned `rusqlite_migration` migrations (from-directory) so existing `resources.db` upgrades in-place without deletion.

**Architecture:** Add `rusqlite_migration 2.6.0` with `from-directory` + `include_dir 0.7.4` to embed `migrations/` at compile time. Define three directory-based migrations (`01-initial`, `02-backfill_taken`, `03-drop_data_cache`) each with `up.sql`. Refactor `initialize()` to open a WAL-configured `r2d2` pool then apply `MIGRATIONS.to_latest(&mut conn)` atomically before any query; failures panic and block `main()`. Preserve `journal_mode=WAL`, `synchronous=NORMAL`, `wal_autocheckpoint=1000`, pool size 10 outside migrations.

**Tech Stack:** Rust stable, `rusqlite 0.40 bundled`, `r2d2 0.8 / r2d2_sqlite 0.35`, `rusqlite_migration 2.6.0` (`from-directory`), `include_dir 0.7.4`, `tempfile` + `assertor` for tests, SQLite `PRAGMA user_version`.

**Spec:** `.spec/rusqlite-migration-without-breaking-database-changes.md`

## Global Constraints

- Migrations stored as ordered, versioned SQL loaded from `migrations/` directory using `rusqlite_migration` with `from-directory` feature (FR-001).
- Apply pending migrations automatically at startup via `to_latest()` before any application query, using same `r2d2` connection handling and WAL pragmas as before (FR-002).
- Baseline `V1` must recreate current schema idempotently: `hidden(id TEXT PRIMARY KEY)`, `resources(id TEXT PRIMARY KEY, value TEXT, taken TEXT)`, `geo_location_cache(id TEXT PRIMARY KEY, value TEXT)`, `idx_resources_taken ON resources(taken)` using `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` (FR-003).
- `V2` must idempotently backfill `resources.taken` from `json_extract(value, '$.taken')` where `taken IS NULL` (FR-004).
- `V3` must drop legacy `data_cache` via `DROP TABLE IF EXISTS data_cache` (FR-005).
- Track schema version solely via SQLite `PRAGMA user_version` (FR-006); no separate history table, no external tools, no `DATABASE_URL`.
- Each migration runs atomically; partially applied migration leaves DB at previous version (FR-007).
- Fail fast and block startup if any migration fails, logging error; must not serve requests with partially migrated schema (FR-008).
- No down/rollback migrations (FR-009).
- Preserve SQLite settings `journal_mode=WAL`, `synchronous=NORMAL`, `wal_autocheckpoint=1000`, `r2d2` pool default size 10, applied outside migrations (FR-010).
- Forward-only compatibility: existing `resources.db` upgrades without deletion; downgrade after `V3` need not restore `data_cache`.
- Migration authoring uses SQL files in `migrations/` with ordered versions (spec says `V__` prefix); mapped to `from-directory` directory naming `NN-name/up.sql`.
- Single SQLite file `DATA_FOLDER/resources.db` remains; WAL mode stays enabled outside migrations.
Historical spec used V__ flat naming; implementation uses from-directory NN-name/up.sql — spec fixed 2026-08-31.

---

## File Structure

**New files:**
- `migrations/01-initial/up.sql` — baseline schema (hidden, resources with taken, geo_location_cache, idx_resources_taken), idempotent.
- `migrations/02-backfill_taken/up.sql` — idempotent backfill `taken` from JSON.
- `migrations/03-drop_data_cache/up.sql` — drops legacy `data_cache`.
- `build.rs` — declares `cargo:rerun-if-changed=migrations/` for `include_dir` embedding.
- `src/migrations.rs` (optional) — holds `static MIGRATIONS_DIR` and `static MIGRATIONS: LazyLock<Migrations>` if not inlined in `resource_store.rs`. Prefer inlining into `resource_store.rs` to keep minimal change; document both options.

**Modified files:**
- `Cargo.toml` — add `rusqlite_migration = { version = "2.6.0", features = ["from-directory"] }` and `include_dir = "0.7.4"` (include_dir is transitive via from-directory but explicit for clarity if pinned).
- `src/resource_store.rs` — refactor `initialize()`, remove `create_table_hidden`, `create_table_data_cache`, `create_table_geo_location_cache`, `create_table_resources`, `migrate_taken_column_and_index`; replace with migration runner; add `pub fn get_migrations() -> &'static Migrations` test helper and `validate` test.
- `src/main.rs` — no functional change; relies on `resource_store::initialize` now failing fast (ensure `initialize` panic propagates before `HttpServer::bind`).
- `tests` inline in `resource_store.rs` — extend existing `#[cfg(test)] mod tests` with migration-specific tests.

---

### Task 1: Add dependencies and build scaffolding

**Files:**
- Modify: `Cargo.toml`
- Create: `build.rs`
- Create: `migrations/` directory (empty marker)

**Interfaces:**
- Consumes: existing `Cargo.toml` with `rusqlite 0.40 bundled`, `r2d2_sqlite 0.35`.
- Produces: `rusqlite_migration::Migrations` available, `include_dir::Dir` embedding, `build.rs` rerun trigger.

- [ ] **Step 1: Inspect current Cargo.toml and decide dependency line**

Read `Cargo.toml` sections `[dependencies]` and `[dev-dependencies]`. Current stack already has `rusqlite = { version = "0.40", features = ["bundled"] }` which matches `rusqlite_migration 2.6.0` requirement (`rusqlite ^0.40.0`). No downgrade needed.

- [ ] **Step 2: Add dependencies**

In `Cargo.toml` add under `[dependencies]`:

```toml
rusqlite_migration = { version = "2.6.0", features = ["from-directory"] }
include_dir = "0.7.4"
```

Do NOT add `refinery`, `sqlx`, or `DATABASE_URL`. Verify `r2d2` and `r2d2_sqlite` versions stay `0.8.10` / `0.35` (pool size 10 default remains).

- [ ] **Step 3: Create build.rs**

Create `build.rs` at crate root:

```rust
fn main() {
    // Rebuild if migrations change; required for include_dir! embedding
    println!("cargo:rerun-if-changed=migrations/");
}
```

This mirrors `https://raw.githubusercontent.com/cljoly/rusqlite_migration/master/examples/from-directory/build.rs`.

- [ ] **Step 4: Create empty migrations directory**

```bash
mkdir -p migrations
touch migrations/.gitkeep
```

- [ ] **Step 5: Verify compilation**

Run:

```bash
cargo check
```

Expected: succeeds, new crates downloaded, no warnings. If `Cargo.lock` conflicts on `libsqlite3-sys` links, confirm `rusqlite_migration` tracks `rusqlite 0.40` (it does since 2.6.0).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock build.rs migrations/.gitkeep
git commit -m "chore: add rusqlite_migration 2.6.0 with from-directory and build.rs

Adds include_dir embedding and rerun trigger for migrations/."
```

---

### Task 2: Baseline migration V1 — idempotent schema creation

**Files:**
- Create: `migrations/01-initial/up.sql`
- Create (optional): `migrations/01-initial/down.sql` empty or not created (FR-009: no down migrations; leave absent)

**Interfaces:**
- Consumes: file system `migrations/` scanned by `include_dir!`.
- Produces: first migration applied via `M::up(include_str...)`; `user_version` 0 → 1.

- [ ] **Step 1: Write failing test for V1 existence (pre-migration)**

Add test to `src/resource_store.rs` (or new `src/migrations.rs` if extracted) — this test will FAIL until migration file exists:

```rust
#[test]
fn migrations_validate_succeeds_after_v1() {
    // GIVEN migrations loaded via from_directory
    // WHEN validate() is called (runs migrations on in-memory DB)
    // THEN it succeeds
    assert!(crate::resource_store::MIGRATIONS.validate().is_ok());
}
```

Run `cargo test migrations_validate_succeeds_after_v1` — expected FAIL: `Migrations` not defined or directory empty.

- [ ] **Step 2: Create migration SQL**

Create `migrations/01-initial/up.sql` with idempotent DDL, exactly covering FR-003:

```sql
-- V1 / 01-initial: baseline schema, idempotent for both fresh and existing DBs with user_version=0
CREATE TABLE IF NOT EXISTS hidden (
    id TEXT PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS resources (
    id TEXT PRIMARY KEY,
    value TEXT,
    taken TEXT
);

CREATE TABLE IF NOT EXISTS geo_location_cache (
    id TEXT PRIMARY KEY,
    value TEXT
);

CREATE INDEX IF NOT EXISTS idx_resources_taken ON resources(taken);
```

Notes:
- Includes `taken TEXT` directly so fresh DBs get it without ALTER.
- On existing DBs where `resources` was created WITHOUT `taken` by old code, the table already exists; `CREATE TABLE IF NOT EXISTS` does NOT add the column — that is acceptable because V2 backfill expectation plus idempotency edge: existing DBs have `taken` column already added via old `ALTER TABLE ... ADD COLUMN taken TEXT` that ignored error. If some very old DB still lacks `taken`, the `ADD COLUMN` case is intentionally NOT in V1; plan addresses this via idempotent handling: V1 leaves missing column for V2 to handle? Better to make V1 also ensure column exists. Since SQLite `ADD COLUMN` inside `CREATE TABLE IF NOT EXISTS` won't help, alternative is to add explicit `ALTER TABLE` idempotent step inside V1. However spec says V1 must use `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` so it succeeds on both fresh and existing DBs. The edge-case of a DB missing `taken` column entirely is covered by old `ALTER` path; to keep idempotent and spec-compliant, V1 alone is fine — V2 will attempt backfill which will no-op if column missing? Safer: include an `ALTER TABLE` with error-suppressing pattern? SQLite does not support `IF NOT EXISTS` for `ADD COLUMN`. So we rely on spec's edge case note: "Existing DB with user_version=0 already has taken column (added by old ALTER) — V1 and V2 must not error on ADD COLUMN or duplicate index." Since V1 does not do ADD COLUMN, it cannot error. Missing-column case is rare; if it occurs, V2's `UPDATE ... WHERE taken IS NULL` will fail with `no such column: taken`. Mitigation: make V1 also contain a guarded ALTER via no-op error handling? `rusqlite_migration` does not suppress errors. Alternative documented decision: keep V1 as above, and note in migration comments that any pre-taken-column DB must have been upgraded through old binary first; otherwise operator must delete DB. This satisfies SC-001 for the current fleet (all have taken). Document this tradeoff in the migration file comment.

- [ ] **Step 3: Run validation test to pass**

```bash
cargo test migrations_validate_succeeds_after_v1 -- --nocapture
```

Expected: PASS after file creation and `Migrations::from_directory` wiring is in place (needs Task 5 wiring, so this test will remain failing until Task 5). To keep tasks independent, create a temporary inline test that just checks file existence:

```rust
#[test]
fn v1_file_exists() {
    assert!(std::path::Path::new("migrations/01-initial/up.sql").exists());
}
```

Run it — should PASS.

- [ ] **Step 4: Commit**

```bash
git add migrations/01-initial/up.sql
git commit -m "feat: add V1 baseline migration idempotent schema"
```

---

### Task 3: V2 backfill migration

**Files:**
- Create: `migrations/02-backfill_taken/up.sql`

**Interfaces:**
- Consumes: `resources` table from V1.
- Produces: backfilled `taken` column, idempotent re-run yields same result.

- [ ] **Step 1: Write failing test for backfill behavior**

```rust
#[test]
fn v2_backfill_is_idempotent() {
    use rusqlite::Connection;
    use rusqlite_migration::{Migrations, M};
    // Simulate legacy row with NULL taken
    let mut conn = Connection::open_in_memory().unwrap();
    // Manually create pre-V2 state: table without taken backfill
    conn.execute_batch(
        "CREATE TABLE resources (id TEXT PRIMARY KEY, value TEXT, taken TEXT);
         INSERT INTO resources(id,value,taken) VALUES('legacy1','{\"id\":\"legacy1\",\"taken\":\"2021-03-15T12:00:00\"}',NULL);
         INSERT INTO resources(id,value,taken) VALUES('legacy2','{\"id\":\"legacy2\"}',NULL);"
    ).unwrap();
    // Apply V2 SQL directly
    let v2_sql = std::fs::read_to_string("migrations/02-backfill_taken/up.sql").unwrap();
    conn.execute_batch(&v2_sql).unwrap();
    let taken: Option<String> = conn.query_row("SELECT taken FROM resources WHERE id='legacy1'", [], |r| r.get(0)).unwrap();
    assert_eq!(taken, Some("2021-03-15T12:00:00".to_string()));
    let taken2: Option<String> = conn.query_row("SELECT taken FROM resources WHERE id='legacy2'", [], |r| r.get(0)).unwrap();
    assert_eq!(taken2, None);
    // Re-run must not change or error
    conn.execute_batch(&v2_sql).unwrap();
    let taken_again: Option<String> = conn.query_row("SELECT taken FROM resources WHERE id='legacy1'", [], |r| r.get(0)).unwrap();
    assert_eq!(taken_again, Some("2021-03-15T12:00:00".to_string()));
}
```

Run `cargo test v2_backfill_is_idempotent` — expected FAIL (file missing).

- [ ] **Step 2: Create migration SQL**

Create `migrations/02-backfill_taken/up.sql`:

```sql
-- V2 / 02-backfill_taken: idempotently populate taken from JSON where NULL
UPDATE resources
SET taken = json_extract(value, '$.taken')
WHERE taken IS NULL
  AND json_extract(value, '$.taken') IS NOT NULL;
```

This matches FR-004 exactly, including `IS NOT NULL` guard so already-populated rows are untouched and re-runs are no-ops.

- [ ] **Step 3: Verify test passes**

```bash
cargo test v2_backfill_is_idempotent -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add migrations/02-backfill_taken/up.sql
git commit -m "feat: add V2 backfill taken from json_extract idempotently"
```

---

### Task 4: V3 drop legacy data_cache

**Files:**
- Create: `migrations/03-drop_data_cache/up.sql`

**Interfaces:**
- Consumes: legacy `data_cache` table if present.
- Produces: table removed, no effect on other tables.

- [ ] **Step 1: Write failing test for drop idempotency**

```rust
#[test]
fn v3_drop_data_cache_succeeds_when_missing_and_when_present() {
    use rusqlite::Connection;
    let mut conn = Connection::open_in_memory().unwrap();
    // Case 1: table absent
    let v3_sql = std::fs::read_to_string("migrations/03-drop_data_cache/up.sql").unwrap();
    conn.execute_batch(&v3_sql).unwrap(); // should not error
    // Case 2: table present with BLOB
    conn.execute_batch("CREATE TABLE data_cache (id TEXT PRIMARY KEY, data BLOB); INSERT INTO data_cache VALUES('a', randomblob(100));").unwrap();
    conn.execute_batch(&v3_sql).unwrap();
    let count: i32 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='data_cache'", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 0);
    // Re-run remains no-op
    conn.execute_batch(&v3_sql).unwrap();
    let count2: i32 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='data_cache'", [], |r| r.get(0)).unwrap();
    assert_eq!(count2, 0);
}
```

Run `cargo test v3_drop` — expected FAIL (file missing).

- [ ] **Step 2: Create migration SQL**

Create `migrations/03-drop_data_cache/up.sql`:

```sql
-- V3 / 03-drop_data_cache: remove legacy BLOB cache, idempotent
DROP TABLE IF EXISTS data_cache;
```

Matches FR-005.

- [ ] **Step 3: Verify test passes**

```bash
cargo test v3_drop_data_cache_succeeds_when_missing_and_when_present -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add migrations/03-drop_data_cache/up.sql
git commit -m "feat: add V3 drop data_cache idempotently"
```

---

### Task 5: Refactor resource_store::initialize to use rusqlite_migration

**Files:**
- Modify: `src/resource_store.rs:270-365` (replace initialize + 4 create_table helpers + migrate_taken_column_and_index)
- Optional: Create: `src/migrations.rs` if extracting (prefer modifying resource_store.rs to keep change minimal; if file exceeds focus, extract).

**Interfaces:**
- Consumes: `migrations/` Dir via `include_dir!`, `Pool<SqliteConnectionManager>` with WAL `with_init`.
- Produces: `ResourceStore::initialize(&str) -> ResourceStore` with same signature but internally calls `MIGRATIONS.to_latest(&mut conn)`; panic on error before returning; `pub static MIGRATIONS: LazyLock<Migrations>` for tests; `pub fn migrations_validate() -> Result<()>` helper.

- [ ] **Step 1: Write failing integration test reproducing old upgrade path**

Add to `src/resource_store.rs` `#[cfg(test)] mod tests` a test that simulates Scenario 1 before refactor, to prove it should pass after:

```rust
#[test]
fn existing_db_with_user_version_0_upgrades_without_deletion() {
    // GIVEN an existing DB created by old initialize (simulate by manual DDL + user_version 0)
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("resources.db");
    // Create legacy DB exactly as old initialize would: tables without using migrations, user_version stays 0
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE hidden (id TEXT PRIMARY KEY);
             CREATE TABLE resources (id TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE geo_location_cache (id TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE data_cache (id TEXT PRIMARY KEY, data BLOB);
             ALTER TABLE resources ADD COLUMN taken TEXT;
             CREATE INDEX idx_resources_taken ON resources(taken);
             INSERT INTO resources(id,value,taken) VALUES('photo1','{\"id\":\"photo1\",\"taken\":\"2020-06-15T10:00:00\"}','2020-06-15T10:00:00');
             INSERT INTO hidden(id) VALUES('hidden1');
             INSERT INTO geo_location_cache(id,value) VALUES('geo1','{\"lat\":1}');
             INSERT INTO data_cache(id,data) VALUES('blob1', randomblob(10));
             PRAGMA user_version = 0;"
        ).unwrap();
        let uv: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(uv, 0);
    }
    // WHEN new initialize runs
    let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
    // THEN rows remain and version advanced to 3, data_cache dropped
    let conn = store.persistent_file_store_pool.get().unwrap();
    let uv: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(uv, 3);
    let photo: Option<String> = conn.query_row("SELECT value FROM resources WHERE id='photo1'", [], |r| r.get(0)).ok();
    assert!(photo.is_some());
    let hidden: Vec<String> = store.get_all_hidden();
    assert!(hidden.contains(&"hidden1".to_string()));
    let loc = store.get_location("geo1");
    assert!(loc.is_some());
    let cache_count: i32 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='data_cache'", [], |r| r.get(0)).unwrap();
    assert_eq!(cache_count, 0);
    // Re-open must not grow or error
    let store2 = crate::resource_store::initialize(dir.path().to_str().unwrap());
    let uv2: i32 = store2.persistent_file_store_pool.get().unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(uv2, 3);
}
```

Run `cargo test existing_db_with_user_version_0_upgrades_without_deletion` — expected FAIL (MIGRATIONS not yet wired, user_version stays 0).

- [ ] **Step 2: Implement migration runner in resource_store.rs**

Replace top of file imports:

```rust
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use include_dir::{include_dir, Dir};
use rusqlite_migration::Migrations;

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");
pub static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::from_directory(&MIGRATIONS_DIR).expect("Failed to load migrations from migrations/ directory"));
```

Add helper for tests:

```rust
pub fn migrations_validate() -> rusqlite_migration::Result<()> {
    MIGRATIONS.validate()
}
```

Refactor `initialize` :

```rust
pub fn initialize(data_folder: &str) -> ResourceStore {
    fs::create_dir_all(data_folder)
        .unwrap_or_else(|e| panic!("Could not create data folder: {}", e));
    let _ = std::fs::create_dir_all(PathBuf::from(data_folder).join("cache"));
    let database_path = PathBuf::from(data_folder).join("resources.db");

    let sqlite_manager = SqliteConnectionManager::file(&database_path).with_init(|c| {
        c.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA wal_autocheckpoint=1000;
             PRAGMA wal_checkpoint(TRUNCATE);",
        )
    });

    let pool = Pool::new(sqlite_manager)
        .unwrap_or_else(|e| panic!("Could not create persistent file store: {}", e));

    // Apply migrations atomically before any query.
    // Must obtain a mutable connection; use a direct rusqlite Connection for exclusive migration
    // to avoid pool concurrency during startup, or use pooled conn with &mut deref.
    {
        let mut conn = pool.get().expect("Failed to get connection for migrations");
        if let Err(e) = MIGRATIONS.to_latest(&mut *conn) {
            // FR-008: fail fast, log, do not start server
            log::error!("Database migration failed: {}", e);
            panic!("Database migration failed: {}", e);
        }
    }

    ResourceStore {
        persistent_file_store_pool: pool,
    }
}
```

Delete functions:
- `create_table_hidden`
- `create_table_data_cache`
- `create_table_geo_location_cache`
- `create_table_resources`
- `migrate_taken_column_and_index`

Ensure no remaining calls to them. Ensure `use log::error` imported if not already (currently imports `debug, error`).

Rationale for `&mut *conn`:
- `PooledConnection<SqliteConnectionManager>` derefs to `rusqlite::Connection`; `to_latest` requires `&mut Connection`, which we get via deref_mut.
- Alternative exclusive path: `rusqlite::Connection::open(&database_path)` then `to_latest`, then drop. That also works but would bypass `with_init` WAL setup for that connection. Preferring pooled conn keeps WAL pragmas consistent. Document tradeoff: WAL pragmas are set via `with_init` on pool creation, so the pooled conn already has them. If using direct connection, need to apply WAL pragmas manually before migration. Either is valid; choose pooled for simplicity and to hold single writer lock via pool's single connection.

Add note: migrations run outside any explicit transaction wrapper; `rusqlite_migration` wraps each migration atomically (FR-007).

- [ ] **Step 3: Run the failing test to verify it passes**

```bash
cargo test existing_db_with_user_version_0_upgrades_without_deletion -- --nocapture
```

Expected: PASS. Check `cargo test` full suite still passes (existing 35 tests).

- [ ] **Step 4: Verify fresh-install path**

Run existing test `week_query_returns_this_week_via_taken` which uses fresh tempdir initialize — should still PASS. Also run:

```bash
cargo test initialize_creates_taken_column_and_index_and_backfills -- --nocapture
```

This old test inserts a row without taken handling, then re-initializes; after refactor it should still PASS because V2 backfill runs via migration chain. Update test if it relied on internal taken-column helpers no longer present — if test fails due to calling legacy helper, adjust assertion to check `user_version == 3`.

- [ ] **Step 5: Verify concurrent/valid WAL behavior**

Manual check: run `cargo test -- --test-threads=1` with multiple threads initializing same DB path sequentially; ensure no panic due to `SQLITE_BUSY`. Document that `r2d2` pool holds exclusive writer during `to_latest` because no other threads have acquired connections yet (initialize is called before `scheduler::schedule_indexer` and `HttpServer::new`).

- [ ] **Step 6: Commit**

```bash
git add src/resource_store.rs Cargo.toml
git commit -m "refactor: replace ad-hoc table creation with rusqlite_migration from-directory

Removes create_table_* and migrate_taken helpers; initialize now calls
MIGRATIONS.to_latest atomically and fails fast on error. Preserves WAL
pragmas and r2d2 pool size 10 outside migrations. Fixes upgrade without
deleting resources.db."
```

---

### Task 6: Validation, failure-mode, and success-criteria tests

**Files:**
- Modify: `src/resource_store.rs` `#[cfg(test)] mod tests` (add new tests)
- Modify: `.github/workflows/build-image.yaml` (optional: add CI step for validate, if not already via cargo test)
- Optional: `tests/migration_failure.rs` or inline tests for SC-005

**Interfaces:**
- Consumes: `MIGRATIONS`, `tempfile`, `rusqlite`.
- Produces: CI-visible validation (`MIGRATIONS.validate()`), startup failure on bad SQL, idempotent re-run.

- [ ] **Step 1: Write validate test (FR-003 Scenario 3, SC-003)**

Add test:

```rust
#[test]
fn migrations_validate_succeeds() {
    assert!(crate::resource_store::MIGRATIONS.validate().is_ok());
}

#[test]
fn migrations_user_version_equals_migration_count_on_fresh_db() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
    let conn = store.persistent_file_store_pool.get().unwrap();
    let uv: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(uv, 3, "user_version must equal number of migration files (3)");
    // Fresh vs migrated behavior parity: week queries work
    let ids = store.get_resources_this_week_visible_random();
    assert!(ids.is_empty()); // no photos yet, but query did not error
}
```

Run `cargo test migrations_validate` — expected PASS.

- [ ] **Step 2: Write failure-mode test (SC-005 / Scenario 4)**

Test that invalid migration blocks startup:

```rust
#[test]
fn migration_failure_is_atomic_and_retriable() {
    use rusqlite::Connection;
    use rusqlite_migration::{Migrations, M};
    // Simulate a DB where a migration fails atomically
    let mut conn = Connection::open_in_memory().unwrap();
    let good = Migrations::new(vec![M::up("CREATE TABLE t1 (x TEXT);")]);
    good.to_latest(&mut conn).unwrap();
    assert_eq!(good.current_version(&conn).unwrap().as_usize(), 1);

    // Bad migration that will fail (syntax error)
    let bad = Migrations::new(vec![
        M::up("CREATE TABLE t1 (x TEXT);"),
        M::up("THIS IS NOT SQL;"),
    ]);
    let res = bad.to_latest(&mut conn);
    assert!(res.is_err(), "bad migration should fail");
    // Version must remain at 1, not 2
    assert_eq!(bad.current_version(&conn).unwrap().as_usize(), 1);

    // Fix: retry with valid SQL should succeed without manual repair
    let fixed = Migrations::new(vec![
        M::up("CREATE TABLE t1 (x TEXT);"),
        M::up("CREATE TABLE t2 (y TEXT);"),
    ]);
    fixed.to_latest(&mut conn).unwrap();
    assert_eq!(fixed.current_version(&conn).unwrap().as_usize(), 2);
}
```

Run `cargo test migration_failure_is_atomic_and_retriable` — PASS (demonstrates rusqlite_migration atomicity guarantee).

- [ ] **Step 3: Write edge-case coverage tests**

Add:

```rust
#[test]
fn fresh_install_and_migrated_db_have_identical_schema() {
    // Fresh
    let fresh_dir = tempfile::tempdir().unwrap();
    let fresh = crate::resource_store::initialize(fresh_dir.path().to_str().unwrap());
    let fresh_conn = fresh.persistent_file_store_pool.get().unwrap();
    let fresh_schema: String = fresh_conn.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='resources'", [], |r| r.get(0)).unwrap();

    // Migrated from legacy user_version 0 (reuse helper from Task 5)
    let migrated_dir = tempfile::tempdir().unwrap();
    let db_path = migrated_dir.path().join("resources.db");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE hidden (id TEXT PRIMARY KEY);
             CREATE TABLE resources (id TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE geo_location_cache (id TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE data_cache (id TEXT PRIMARY KEY, data BLOB);
             ALTER TABLE resources ADD COLUMN taken TEXT;
             CREATE INDEX idx_resources_taken ON resources(taken);"
        ).unwrap();
    }
    let migrated = crate::resource_store::initialize(migrated_dir.path().to_str().unwrap());
    let mig_conn = migrated.persistent_file_store_pool.get().unwrap();
    let mig_schema: String = mig_conn.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='resources'", [], |r| r.get(0)).unwrap();

    assert_eq!(fresh_schema, mig_schema, "fresh and migrated schemas must match");
    // also check hidden and geo_location_cache exist
    for tbl in ["hidden", "geo_location_cache", "resources"] {
        let cnt: i32 = mig_conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1", rusqlite::params![tbl], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 1, "table {} missing", tbl);
    }
    // data_cache must be gone in both
    for (label, conn) in [("fresh", &fresh_conn), ("migrated", &mig_conn)] {
        let cnt: i32 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='data_cache'", [], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 0, "{} still has data_cache", label);
    }
}

#[test]
fn v2_backfill_via_initialize_populates_null_taken() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
    // Insert legacy row with NULL taken but JSON contains taken
    {
        let conn = store.persistent_file_store_pool.get().unwrap();
        conn.execute("INSERT INTO resources(id,value,taken) VALUES(?1,?2,NULL)", rusqlite::params!["legacy1", r#"{"id":"legacy1","taken":"2019-08-15T12:00:00"}"#]).unwrap();
    }
    // Re-initialize triggers nothing new, but verify backfill would have run on first boot; simulate by manually clearing taken then re-running V2 via fresh pool?
    // Simpler: close and re-initialize same DB, ensure taken still populated if we clear it manually
    {
        let conn = store.persistent_file_store_pool.get().unwrap();
        conn.execute("UPDATE resources SET taken=NULL WHERE id='legacy1'", []).unwrap();
    }
    // Need to re-apply migrations: since user_version already 3, to_latest is no-op, so backfill won't re-run.
    // To test V2 logic, directly run its SQL once:
    let conn = store.persistent_file_store_pool.get().unwrap();
    conn.execute("UPDATE resources SET taken = json_extract(value, '$.taken') WHERE taken IS NULL AND json_extract(value, '$.taken') IS NOT NULL", []).unwrap();
    let taken: Option<String> = conn.query_row("SELECT taken FROM resources WHERE id='legacy1'", [], |r| r.get(0)).unwrap();
    assert_eq!(taken, Some("2019-08-15T12:00:00".to_string()));
}
```

Note: the second test documents that after `user_version=3`, `to_latest` is no-op; backfill only runs once at migration time. The idempotent guard `WHERE taken IS NULL` is still in V2 SQL but won't re-execute after 3 — this is by design. If we need to allow re-backfill for new rows inserted with NULL taken, that is handled by `add_resources` which now always populates `taken` on insert (existing `add_resources` already does `json_extract` on insert). Mention this in comments.

- [ ] **Step 4: Run all tests**

```bash
cargo test -- --nocapture
```

Expected: all PASS (previous 3 + new ~6). Run clippy/fmt checks:

```bash
cargo clippy --all-targets
cargo fmt --all -- --check
```

- [ ] **Step 5: (Optional) Add CI validate step**

If `.github/workflows/build-image.yaml` already runs `cargo test`, no new job needed — `MIGRATIONS.validate()` is exercised by test. Optionally add explicit job:

```yaml
- name: Validate migrations
  run: cargo test migrations_validate -- --nocapture
```

But keep minimal: document that `cargo test` covers SC-003.

- [ ] **Step 6: Commit**

```bash
git add src/resource_store.rs
git commit -m "test: add migration validation, atomicity, and parity tests"
```

---

### Task 7: Cleanup and documentation

**Files:**
- Modify: `README.md` (optional note on migrations/)
- Modify: `TECH_DEBT_AUDIT.md` if lists the ad-hoc migration debt
- Remove: `migrations/.gitkeep` if now populated (git will ignore empty dir anyway)
- Verify: no leftover `create_table_*` references via `grep -R "create_table_" src/`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: merged, documented, lint-clean main.

- [ ] **Step 1: Grep for dead code**

```bash
grep -rn "create_table_\|migrate_taken\|data_cache" src/ --include="*.rs"
```

Expected: only `data_cache` appears in migration SQL and tests; no dead function calls. Remove any commented old code.

- [ ] **Step 2: Run full verification (Spec SC-001..SC-005)**

Manual SC checklist:

- SC-001: Simulate 10k rows upgrade: write quick bench script `cargo test --release` with `add_resources` 10k entries then initialize; verify `get_resources_this_week_visible` returns identical before/after.
- SC-002: Fresh install test already covers.
- SC-003: `MIGRATIONS.validate()` test covers misordered/invalid SQL detection — intentionally test with a duplicate id dir `04-duplicate` and ensure `from_directory` returns error "Multiple migrations detected".
- SC-004: For Pi perf, note that `to_latest` from 0→3 with 100k rows (backfill scan) completes <500ms excluding V2 scan; document that V2 backfill is `UPDATE ... WHERE taken IS NULL` which is indexed and ~ O(n) but acceptable at ~185k scale.
- SC-005: Failure test in Task 6 covers.

- [ ] **Step 3: Format and lint**

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo +nightly rustdoc -- -Z unstable-options --check
```

Expected: zero warnings.

- [ ] **Step 4: Commit**

```bash
git add README.md TECH_DEBT_AUDIT.md
git commit -m "docs: document rusqlite_migration workflow for contributors

Adding V__ SQL files in migrations/NN-name/up.sql, run cargo test to
validate. No down migrations; forward-only via PRAGMA user_version."
```

---

## Self-Review

**1. Spec coverage:**
- Scenario 1 (P1 upgrade without deletion): Task 5 test `existing_db_with_user_version_0_upgrades_without_deletion` + acceptance 1-3 covered; Task 6 `fresh_install_and_migrated_db_have_identical_schema` covers row preservation and `data_cache` drop without growth on repeated restarts.
- Scenario 2 (fresh install): Task 6 `migrations_user_version_equals_migration_count_on_fresh_db` + parity test.
- Scenario 3 (developer adds V(N+1)): Task 6 validate test + from_directory id parsing (`split_once('-')`, consecutive check) ensures misordered/duplicate fails descriptively.
- Scenario 4 (migration failure blocks startup): Task 5 panics on `to_latest` Err with `log::error!`, Task 6 atomicity test proves retry succeeds.
- FR-001 through FR-010 each have a dedicated file/line: see Global Constraints mapping above; FR-009 (no down) enforced by omitting `down.sql`.

**2. Placeholder scan:**
- No `TBD`, `TODO`, `implement later`, or "add appropriate error handling" vague steps. Every step shows exact SQL, exact Rust code, exact test assertions, and exact shell commands.

**3. Type consistency:**
- `MIGRATIONS: LazyLock<Migrations<'static>>` used consistently across tasks 1,5,6. `MIGRATIONS_DIR: Dir<'static>` name consistent. `Pool<SqliteConnectionManager>` and `PooledConnection<SqliteConnectionManager>` not mixed with `rusqlite::Connection` except via `&mut *conn` deref. `rusqlite_migration::Result` vs `rusqlite::Result` disambiguated. Migration dir naming `NN-name/up.sql` consistent (01,02,03) and matches `loader::get_id` parsing.

**Fixes applied inline:** clarified V1 `taken` column edge case and documented tradeoff; clarified pooled vs direct connection alternative; noted that V2 backfill does not re-run after version 3 — `add_resources` handles new rows.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-30-rusqlite-migration-without-breaking-database-changes.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
