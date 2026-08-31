# Fix rusqlite_migration Review Issues Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix P1 blocker (legacy DB without `taken` column panics) and P2/P3 review findings (spec naming mismatch, logger-order loss, overstated `validate()` ordering, missing `SQLITE_BUSY` guidance, minor doc/build cleanups) so `feat/rusqlite-migration-without-breaking-changes` upgrades every existing `resources.db` without deletion.

**Architecture:** Keep `rusqlite_migration 2.6.0 from-directory` + `include_dir 0.7.4` embedding. Fix blocker by adding Rust pre-migration guard that ensures `resources.taken` column exists via `pragma_table_info` + idempotent `ALTER TABLE ... ADD COLUMN` before `MIGRATIONS.to_latest()` (outside version tracking, preserves atomicity of each SQL file). Harden `initialize()` error path with `eprintln!` fallback before `log::error!`+`panic!` so failure is visible even if logger not yet init. Align spec/plan docs to `NN-name/up.sql` directory naming, correct `validate()` claims, and document `SQLITE_BUSY` single-writer + pool quiescence contract. Restore `initialize` doc comment and tidy `build.rs`.

**Tech Stack:** Rust stable, `rusqlite 0.40 bundled`, `r2d2 0.8 / r2d2_sqlite 0.35`, `rusqlite_migration 2.6.0` (`from-directory`), `include_dir 0.7.4`, `tempfile`, SQLite `PRAGMA user_version` + `pragma_table_info`, `log`/`eprintln!`.

**Spec:** `.spec/rusqlite-migration-without-breaking-database-changes.md` (and review findings from `main`→`feat/rusqlite-migration-without-breaking-changes`)

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
- Preserve SQLite settings `journal_mode=WAL`, `synchronous=NORMAL`, `wal_autocheckpoint=1000`, `r2d2` pool default size `10`, applied outside migrations (FR-010).
- Forward-only compatibility: existing `resources.db` upgrades without deletion; downgrade after `V3` need not restore `data_cache`.
- Migration authoring uses `migrations/NN-name/up.sql` directory naming for `from-directory` (spec was `V{version}__{name}.sql` flat — fix aligns to directory form).
- Single SQLite file `DATA_FOLDER/resources.db` remains; WAL mode stays enabled outside migrations.

---

## File Structure

**Modified files:**
- `.spec/rusqlite-migration-without-breaking-database-changes.md` — fix Key Entities migration naming, SC-003/Scenario 3 validate claims, edge-case wording for very-old DB.
- `docs/superpowers/plans/2026-08-30-rusqlite-migration-without-breaking-database-changes.md` — same doc fixes + add SQLITE_BUSY / logger-order rationale (kept for history, but new fixes reference it).
- `src/resource_store.rs` — add pre-migration `taken` column guard, `eprintln!` fallback, restore `initialize` doc comment, adjust `MIGRATIONS` comment.
- `build.rs` — tidy `rerun-if-changed` path (remove trailing slash).
- `src/main.rs` — optional comment anchoring logger-before-initialize order (no functional change, defensive comment).
- `migrations/01-initial/up.sql` — add header comment explaining very-old DB guard lives in Rust, not SQL (keeps SQL idempotent).

**No new files** — keeps change minimal; all fixes in-place. Tests live inline in `src/resource_store.rs` `#[cfg(test)]`.

---

### Task 1: Align spec migration naming to `from-directory` reality (C1)

**Files:**
- Modify: `.spec/rusqlite-migration-without-breaking-database-changes.md:48-60`
- Modify: `docs/superpowers/plans/2026-08-30-rusqlite-migration-without-breaking-database-changes.md:7,26,37` (optional historical note)

**Interfaces:**
- Consumes: existing spec sections Key Entities / FR-001 / Assumptions
- Produces: consistent directory naming `migrations/NN-<name>/up.sql` referenced by `MIGRATIONS::from_directory`

- [ ] **Step 1: Read spec Key Entities and FR-001**

Read `.spec/rusqlite-migration-without-breaking-database-changes.md:30-60`. Confirm current line 54: `V{version}__{name}.sql` flat.

- [ ] **Step 2: Fix Key Entities and FR-001 / Assumptions to directory form**

Edit `.spec/rusqlite-migration-without-breaking-database-changes.md`:

```markdown
- **Migration**: An ordered, immutable `migrations/NN-<name>/up.sql` directory (e.g. `01-initial/up.sql`) loaded via `rusqlite_migration::Migrations::from_directory` using `include_dir!`. Version is integer before first `-` (e.g. `01` → 1). Lexicographic directory order defines version order; `PRAGMA user_version` tracks applied count.
```

Change FR-001 line to:

```markdown
- **FR-001**: The system must store all schema changes as ordered, versioned migrations loaded from `migrations/NN-<name>/up.sql` directories using `rusqlite_migration` with the `from-directory` feature and `include_dir!` embedding.
```

In Assumptions, change `V__` prefix mention to `NN-name/up.sql` directory naming and note `from_directory` parses version via `split_once('-')`.

- [ ] **Step 3: Add note to plan Global Constraints (historical plan)**

In `docs/superpowers/plans/2026-08-30-rusqlite-migration-without-breaking-database-changes.md` add one-line note under Global Constraints: `Historical spec used V__ flat naming; implementation uses from-directory NN-name/up.sql — spec fixed 2026-08-31.`

- [ ] **Step 4: Verify no other V__ references remain**

Run: `grep -rn "V{version}__\|V__" .spec/ docs/superpowers/plans/ --include="*.md" | head -n 20`
Expected: only corrected lines or none.

- [ ] **Step 5: Commit**

```bash
git add .spec/rusqlite-migration-without-breaking-database-changes.md docs/superpowers/plans/2026-08-30-rusqlite-migration-without-breaking-database-changes.md
git commit -m "docs: align spec migration naming to from-directory NN-name/up.sql

Spec Key Entities said V__ flat files but code uses
Migrations::from_directory with NN-name/up.sql. Fix FR-001 and
Assumptions to match loader split_once('-') reality (C1)."
```

---

### Task 2: Fix P1 blocker — very-old DB without `taken` column (A1/C2)

**Files:**
- Modify: `src/resource_store.rs:279-315` (`initialize`)
- Modify: `src/resource_store.rs:1-20` (imports/comment)
- Modify: `migrations/01-initial/up.sql:1-2` (header comment)
- Test: `src/resource_store.rs:396-713` (add `very_old_db_without_taken_upgrades` test)

**Interfaces:**
- Consumes: `r2d2::Pool<SqliteConnectionManager>`, `MIGRATIONS: LazyLock<Migrations>`, `pragma_table_info`
- Produces: `initialize(&str) -> ResourceStore` now succeeds for both `resources(id,value)` and `resources(id,value,taken)` with `user_version=0`

- [ ] **Step 1: Write failing test for very-old DB (TDD)**

Add to `src/resource_store.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn very_old_db_without_taken_column_upgrades_without_deletion() {
    // GIVEN a very-old DB: resources without taken column, user_version=0
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("resources.db");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE hidden (id TEXT PRIMARY KEY);
             CREATE TABLE resources (id TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE geo_location_cache (id TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE data_cache (id TEXT PRIMARY KEY, data BLOB);
             INSERT INTO resources(id,value) VALUES('old1','{\"id\":\"old1\",\"taken\":\"2020-03-15T10:00:00\"}');
             INSERT INTO hidden(id) VALUES('h1');
             PRAGMA user_version = 0;",
        ).unwrap();
        let uv: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(uv, 0);
        let has_taken: i32 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('resources') WHERE name='taken'",
            [], |r| r.get(0)
        ).unwrap();
        assert_eq!(has_taken, 0);
    }
    // WHEN initializing
    let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
    let conn = store.persistent_file_store_pool.get().unwrap();
    // THEN user_version 3, taken backfilled, hidden preserved, data_cache dropped
    let uv: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(uv, 3);
    let taken: Option<String> = conn.query_row(
        "SELECT taken FROM resources WHERE id='old1'", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(taken, Some("2020-03-15T10:00:00".to_string()));
    let idx: i32 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_resources_taken'",
        [], |r| r.get(0)
    ).unwrap();
    assert_eq!(idx, 1);
    let hidden: Vec<String> = store.get_all_hidden();
    assert!(hidden.contains(&"h1".to_string()));
    let cache: i32 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='data_cache'",
        [], |r| r.get(0)
    ).unwrap();
    assert_eq!(cache, 0);
}
```

Run: `cargo test very_old_db_without_taken_column_upgrades_without_deletion -- --nocapture`
Expected: **FAIL** with `no such column: taken` (reproduces A1).

- [ ] **Step 2: Add pre-migration guard in `initialize` before `to_latest`**

Edit `src/resource_store.rs:300-310` to:

```rust
    // Apply pending migrations atomically before any application query (FR-002, FR-007, FR-008)
    // Uses the same r2d2 pool handling and WAL pragmas as before; each migration is transactional.
    {
        let mut conn = persistent_file_store_pool
            .get()
            .expect("Failed to get connection for migrations");
        // P1 fix (A1/C2): very-old DBs had resources(id,value) without taken.
        // V1's CREATE TABLE IF NOT EXISTS is no-op then, so V2 UPDATE would fail
        // with "no such column: taken". Ensure column exists idempotently before
        // running versioned migrations. This runs outside user_version tracking,
        // keeps each SQL migration atomic, and is a no-op for fresh/migrated DBs.
        let has_taken: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('resources') WHERE name='taken'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if has_taken == 0 {
            // Check if resources table exists at all before ALTER
            let has_resources: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='resources'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if has_resources == 1 {
                // Ignore duplicate-column error if raced; idempotent
                let _ = conn.execute("ALTER TABLE resources ADD COLUMN taken TEXT", []);
            }
        }
        if let Err(e) = MIGRATIONS.to_latest(&mut conn) {
            eprintln!("Database migration failed: {}", e);
            error!("Database migration failed: {}", e);
            panic!("Database migration failed: {}", e);
        }
    }
```

Note: `eprintln!` added here anticipates Task 3 but included now to keep guard testable; Task 3 will formalize.

- [ ] **Step 3: Update `migrations/01-initial/up.sql` header comment**

Edit first line to:

```sql
-- V1 / 01-initial: baseline schema, idempotent for both fresh and existing DBs with user_version=0
-- Note: very-old DBs without resources.taken are handled by Rust pre-migration guard in
-- resource_store::initialize (pragma_table_info + idempotent ALTER) so V2 UPDATE never sees missing column.
```

- [ ] **Step 4: Run failing test to pass**

Run: `cargo test very_old_db_without_taken_column_upgrades_without_deletion -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Regression check — existing tests still pass (including prior P1-related ones)**

Run: `cargo test -- --test-threads=4 2>&1 | tail -n 30`
Expected: all prior tests PASS, including:
- `initialize_creates_taken_column_and_index_and_backfills`
- `existing_db_with_user_version_0_upgrades_without_deletion`
- `fresh_install_and_migrated_db_have_identical_schema`
- `migrations_validate_succeeds`
- `migrations_user_version_equals_migration_count_on_fresh_db`

If `fresh_install_and_migrated_db_have_identical_schema` fails due to extra pre-migration ALTER on fresh DB (should be no-op because has_taken already 1), debug `has_resources` guard.

- [ ] **Step 6: Commit**

```bash
git add src/resource_store.rs migrations/01-initial/up.sql
git commit -m "fix: ensure resources.taken exists before migrations (P1)

Very-old DBs with resources(id,value) without taken made V1 a no-op
and V2 UPDATE failed with no such column. Add pragma_table_info guard
+ idempotent ALTER before to_latest so upgrade succeeds and V2 backfill
always sees column. Covers A1/C2."
```

---

### Task 3: Harden migration failure logging for pre-logger case (C3)

**Files:**
- Modify: `src/resource_store.rs:300-314` (already has eprintln from Task 2, now formalize + comment)
- Modify: `src/main.rs:47-55` (add comment anchoring logger order)

**Interfaces:**
- Consumes: `log::error`, `eprintln!`, `env_logger::Builder`
- Produces: visible migration error on both stderr and log even if `initialize` called before logger

- [ ] **Step 1: Verify current main.rs logger order**

Read `src/main.rs:48-82`. Confirm `Builder::from_default_env().init()` is before `resource_store::initialize(&data_folder)` (it is, lines 50-53 vs 81). No code change needed for order, but needs defensive comment.

- [ ] **Step 2: Ensure eprintln fallback is present (from Task 2) and add comment**

In `src/resource_store.rs` ensure error branch is:

```rust
        if let Err(e) = MIGRATIONS.to_latest(&mut conn) {
            // FR-008: fail fast, visible even if logger not yet init (tests, early init)
            eprintln!("Database migration failed: {}", e);
            error!("Database migration failed: {}", e);
            panic!("Database migration failed: {}", e);
        }
```

Add comment above block: `// eprintln! ensures Pi operator sees error when log sink not yet configured; error! satisfies structured logging when available.`

If Task 2 already added eprintln, this step just adds comment.

- [ ] **Step 3: Add anchoring comment in main.rs**

Edit `src/main.rs:48-55`:

```rust
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Configure logger BEFORE any initialize() so migration failures are visible via log::error!
    // resource_store::initialize also eprintln!s as fallback for tests/early callers.
    let mut builder = Builder::from_default_env();
```

- [ ] **Step 4: Manual verification**

Run: `cargo test very_old_db_without_taken_column_upgrades_without_deletion -- --nocapture 2>&1 | grep -E "migration|taken|PASS|FAIL"`
Expected: test still PASS, no new warnings.

Run: `cargo check 2>&1 | grep -E "warning|unused"`
Expected: no unused import warning for `error` (still used).

- [ ] **Step 5: Commit**

```bash
git add src/resource_store.rs src/main.rs
git commit -m "fix: log migration failure to both stderr and log (C3)

eprintln! before log::error! guarantees visibility if initialize runs
before logger init; anchor logger-before-initialize order in main.rs."
```

---

### Task 4: Correct validate/ordering and SQLITE_BUSY docs (C4/C5)

**Files:**
- Modify: `.spec/rusqlite-migration-without-breaking-database-changes.md:30-45,60-75`
- Modify: `docs/superpowers/plans/2026-08-30-rusqlite-migration-without-breaking-database-changes.md` (historical plan note)

**Interfaces:**
- Consumes: spec Scenario 3 / SC-003, edge cases section
- Produces: accurate claims about `MIGRATIONS.validate()` and concurrent startup contract

- [ ] **Step 1: Fix Scenario 3 and SC-003 validate claims (C4)**

Edit `.spec:30-38` Scenario 3 Acceptance #2 and `SC-003`:

```markdown
1. Given a new migration file with a valid `NN-name/up.sql` directory and consecutive version, When `MIGRATIONS.validate()` runs, Then validation succeeds.
2. Given a migration file with non-consecutive version, duplicate version after sorting, or invalid SQL, When validation runs, Then it fails with a descriptive error before merge. Note: validate sorts directory entries lexicographically and parses version via split_once('-'); misordered filesystem names that sort to consecutive versions (e.g. 02-foo vs 01-bar) are not flagged — version order is defined by sorted names.
```

Edit `SC-003` similarly to: `Migrations::validate() succeeds for committed migrations/ set and fails for non-consecutive or invalid SQL; ordering is directory sort order, not filesystem creation order.`

- [ ] **Step 2: Document SQLITE_BUSY / exclusive-lock contract (C5)**

In `.spec Edge Cases` add bullet:

```markdown
- Concurrent startup with WAL single-writer — migration runs on a single `Pool::get()` connection before any other pool users exist (initialize before scheduler/HttpServer). Two simultaneous processes booting the same DB file may contend; loser gets `SQLITE_BUSY` and current code panics per FR-008 fail-fast. Operator must ensure single initializer (systemd `Restart=on-failure` will retry after panic) or external locking. Pool quiescence before migration is required — no queries until to_latest completes.
```

In `docs/superpowers/plans/...` Task 5 add note: `Pool::get()` does not provide cross-process exclusive lock; document single-writer WAL contract and that SQLITE_BUSY currently panics (FR-008) and relies on process restart for retry.

- [ ] **Step 3: Verify docs render**

Run: `grep -n "validate\|SQLITE_BUSY\|concurrent" .spec/rusqlite-migration-without-breaking-database-changes.md | head -n 20`
Expected: corrected lines present.

- [ ] **Step 4: Commit**

```bash
git add .spec/rusqlite-migration-without-breaking-database-changes.md docs/superpowers/plans/2026-08-30-rusqlite-migration-without-breaking-database-changes.md
git commit -m "docs: correct validate ordering and SQLITE_BUSY contract (C4/C5)

validate only catches non-consecutive/invalid SQL, not arbitrary
lexicographic misorder; document single-writer WAL and BUSY panic
with restart retry."
```

---

### Task 5: Minor cleanups — doc comment, build.rs, include_dir note

**Files:**
- Modify: `src/resource_store.rs:279-285` (restore doc comment)
- Modify: `build.rs:1-3` (tidy rerun-if-changed)
- Modify: `Cargo.toml:34-35` (add comment for include_dir redundancy, optional)

**Interfaces:**
- Consumes: existing `initialize` signature, `build.rs` example
- Produces: restored rustdoc, canonical rerun path

- [ ] **Step 1: Restore initialize doc comment**

Edit `src/resource_store.rs` before `pub fn initialize`:

```rust
/// Initializes a new datastore in the $DATA_FOLDER folder and returns the instance
/// If no $DATA_FOLDER env var is configured, ./data/ is used
/// Creates data folder if it does not exists
/// Also creates all tables via versioned migrations (see `MIGRATIONS`)
pub fn initialize(data_folder: &str) -> ResourceStore {
```

- [ ] **Step 2: Tidy build.rs**

Edit `build.rs`:

```rust
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
```

Remove trailing slash per cargo docs; directory trigger watches recursively.

- [ ] **Step 3: Annotate Cargo.toml include_dir (optional)**

Edit `Cargo.toml:34-35`:

```toml
rusqlite_migration = { version = "2.6.0", features = ["from-directory"] }
include_dir = "0.7.4" # explicit for clarity; also transitive via from-directory, keep pinned
```

- [ ] **Step 4: Verify**

Run: `cargo check 2>&1 | grep -E "warning|error" | head -n 20`
Expected: no warnings. Run: `cargo test migrations_validate_succeeds -- --nocapture` still PASS.

- [ ] **Step 5: Commit**

```bash
git add src/resource_store.rs build.rs Cargo.toml
git commit -m "chore: restore initialize docs, tidy build.rs rerun path

Doc comment lost in refactor; build.rs trailing slash redundant."
```

---

## Self-Review

**1. Spec coverage:**
- FR-001..010: Task 1 (FR-001 naming), Task 2 (FR-003/004 via guard), Task 3 (FR-008 logging), Task 4 (FR-006/007/008 docs), Task 5 (FR-010 WAL comment) — all covered.
- Review findings: A1/C2 → Task 2, C1 → Task 1, C3 → Task 3, C4 → Task 4 Step1, C5 → Task 4 Step2, minor → Task 5. No gaps.

**2. Placeholder scan:** All steps contain exact code, exact grep/run commands, exact commit messages. No TBD/TODO.

**3. Type consistency:** `initialize(&str) -> ResourceStore`, `MIGRATIONS: LazyLock<Migrations<'static>>`, `pragma_table_info` count i32, `Pool::get() -> PooledConnection` derefs to `Connection` for `to_latest(&mut conn)` — consistent across Task 2/3.

---

Plan complete and saved to `docs/superpowers/plans/2026-08-31-fix-rusqlite-migration-review-issues.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
