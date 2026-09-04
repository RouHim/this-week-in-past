# Feature Specification: rusqlite_migration without breaking database changes

**Created**: 2026-08-30
**Status**: Approved
**Input**: lets plan the feature rusqlite_migration without breaking database changes

## Goal
Replace the ad-hoc `CREATE TABLE IF NOT EXISTS` + `ALTER TABLE ... (ignore error)` logic in `initialize()` with a versioned migration system using `rusqlite_migration` backed by SQL files, so existing installations upgrade in place without deleting `resources.db`. The feature provides forward-only, auditable, atomic schema evolution while preserving the current single-file SQLite, WAL, and `r2d2` pooling behavior.

## User Scenarios
### Scenario 1 - Existing Pi upgrades without deleting database (P1)
A user with an existing `DATA_FOLDER/resources.db` containing photos, hidden flags, and geo cache upgrades the binary. On next startup the app detects pending migrations and applies them automatically without manual deletion or export/import.

**Acceptance**
1. Given an existing `resources.db` with `user_version = 0` and schema created by the old `initialize()` (including `taken` column already added via `ALTER`), When the new binary starts, Then all pending migrations are applied exactly once and `user_version` advances to the latest version.
2. Given the same existing DB, When migrations complete, Then all existing rows in `resources`, `hidden`, and `geo_location_cache` remain present and queryable via the week/random/hidden APIs.
3. Given an existing DB where `data_cache` contains legacy BLOB rows, When `V3` runs, Then the table is dropped without error and `resources.db` size does not grow on repeated restarts.

### Scenario 2 - Fresh install creates database via same path (P1)
A fresh installation with empty `DATA_FOLDER` (no `resources.db`) starts the app. The database is created solely through the migration chain, resulting in the same schema as an upgraded installation.

**Acceptance**
1. Given no `resources.db` exists, When the app starts, Then `resources.db` is created with all tables and indexes defined by migrations `V1` through latest.
2. Given a fresh DB created via migrations, When week queries and hidden/resource APIs are used, Then behavior matches a migrated existing DB.

### Scenario 3 - Developer adds a new schema change (P2)
A developer needs to add a new table, column, or index. They add a new `NN-name/up.sql` directory (e.g. `05-new_feature/up.sql`) and CI validates it before merge.

**Acceptance**
1. Given a new migration file with a valid `NN-name/up.sql` directory and consecutive version, When `MIGRATIONS.validate()` runs, Then validation succeeds.
2. Given a migration file with non-consecutive version, duplicate version after sorting, or invalid SQL, When validation runs, Then it fails with a descriptive error before merge. Note: validate sorts directory entries lexicographically and parses version via `split_once('-')`; misordered filesystem names that sort to consecutive versions (e.g. `02-foo` vs `01-bar`) are not flagged — version order is defined by sorted names.
### Scenario 4 - Migration failure blocks startup (P1)
A migration fails due to disk full, permission error, or invalid SQL on a Pi. The app does not serve stale or partially migrated data.

**Acceptance**
1. Given a migration step fails, When `initialize()` attempts to run migrations, Then startup aborts, an error is logged, and the HTTP server does not start.
2. Given a migration fails atomically, When the app is restarted after fixing the cause (e.g., freeing disk), Then the same migration retries and succeeds without manual DB repair.

## Functional Requirements
- **FR-001**: The system must store all schema changes as ordered, versioned migrations loaded from `migrations/NN-<name>/up.sql` directories using `rusqlite_migration` with the `from-directory` feature and `include_dir!` embedding.
- **FR-002**: The system must apply pending migrations automatically at startup via `to_latest()` before any application query, using the same `r2d2` connection handling and WAL pragmas as before.
- **FR-003**: The baseline migration `V1` must recreate the current schema idempotently: `hidden(id TEXT PRIMARY KEY)`, `resources(id TEXT PRIMARY KEY, value TEXT, taken TEXT)`, `geo_location_cache(id TEXT PRIMARY KEY, value TEXT)`, and `idx_resources_taken ON resources(taken)`, using `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` so it succeeds on both fresh and existing DBs with `user_version = 0`.
- **FR-004**: The migration `V2` must idempotently backfill `resources.taken` from `json_extract(value, '$.taken')` where `taken IS NULL`, so existing rows gain the indexed column without data loss or duplicate updates on re-run.
- **FR-005**: The migration `V3` must drop the legacy `data_cache` table via `DROP TABLE IF EXISTS data_cache` without requiring manual intervention and without affecting other tables.
- **FR-006**: The system must track the current schema version solely via SQLite `PRAGMA user_version` (as provided by `rusqlite_migration`), not via a separate history table, and must not require external tools or `DATABASE_URL`.
- **FR-007**: The system must run each migration atomically; a partially applied migration must leave the database at the previous version so a subsequent retry can succeed.
- **FR-008**: The system must fail fast and block startup if any migration fails, logging the error; it must not serve requests with a partially migrated schema.
- **FR-009**: The system must not provide down/rollback migrations; rollback is out of scope and requires restoring a backup.
- **FR-010**: The system must preserve existing SQLite settings: `journal_mode=WAL`, `synchronous=NORMAL`, `wal_autocheckpoint=1000`, and `r2d2` pool default size `10`, applied outside migrations.

## Key Entities
- **resources.db**: Single SQLite file at `DATA_FOLDER/resources.db` containing all persistent tables. Source of truth for photos metadata, hidden state, and geo cache.
- **Migration**: An ordered, immutable `migrations/NN-<name>/up.sql` directory (e.g. `01-initial/up.sql`) loaded via `rusqlite_migration::Migrations::from_directory` using `include_dir!`. Version is integer before first `-` (e.g. `01` → 1). Lexicographic directory order defines version order; `PRAGMA user_version` tracks applied count.
- **Schema Version**: Integer stored in `PRAGMA user_version` representing the number of successfully applied migrations. `0` means no migrations applied (legacy DB).

## Edge Cases
- Existing DB with `user_version = 0` already has `taken` column (added by old `ALTER`) — `V1` and `V2` must not error on `ADD COLUMN` or duplicate index.
- Existing DB where `taken` is `NULL` for old rows — `V2` backfill must populate it from `value` JSON where present.
- Fresh install with no DB file — `V1-V3` must create a complete schema from scratch.
- `data_cache` absent, empty, or large (legacy BLOB) — `V3 DROP TABLE IF EXISTS` must succeed in all cases.
- Concurrent startup with WAL single-writer — migration runs on a single `Pool::get()` connection before any other pool users exist (initialize before scheduler/HttpServer). Two simultaneous processes booting the same DB file may contend; loser gets `SQLITE_BUSY` and current code panics per FR-008 fail-fast. Operator must ensure single initializer (systemd `Restart=on-failure` will retry after panic) or external locking. Pool quiescence before migration is required — no queries until `to_latest` completes.
- Disk full, permission denied, or power loss mid-migration — atomicity must ensure DB remains at previous version and startup fails visibly.
- Future `ALTER TABLE DROP COLUMN` or similar SQLite limitation — migrations must use SQLite-supported statements only.
## Research Notes
- https://docs.rs/rusqlite_migration/latest/rusqlite_migration/ — `rusqlite_migration` uses `PRAGMA user_version` at a fixed file offset, not a history table, for fast open and lightweight tracking.
- https://github.com/cljoly/rusqlite_migration/tree/master/examples/from-directory — `from-directory` feature loads `migrations/**/*.sql` via `build.rs` and provides `MIGRATIONS.validate()` for CI and snapshot testing.
- https://crates.io/crates/rusqlite_migration — Latest `2.6.0` (2026-05-28), ~3.9M total downloads, tracks `rusqlite` MSRV, supports `rusqlite 0.40 bundled`, no `sqlx` or `DATABASE_URL` required.
- https://www.sqlite.org/pragma.html#pragma_user_version — `user_version` is a 32-bit integer persisted in the database file, suitable for versioning up to ~2B migrations.

## Assumptions
- "Without breaking" means forward-only compatibility: existing `resources.db` files must upgrade without deletion; downgrade to the old binary after `V3` is not required to restore `data_cache`.
- Migration authoring uses `migrations/NN-<name>/up.sql` directories with ordered versions parsed via `split_once('-')` on the directory name, not inline `Migrations::from_slice`.
- Failure handling is fail-fast and blocks startup; no automatic `resources.db.bak` copy is created by the app (operator handles backups).
- No down migrations are provided; forward-only is sufficient.
- The SQLite file remains at `DATA_FOLDER/resources.db` and WAL mode remains enabled outside migrations.

- **SC-001**: An existing `resources.db` with 10k+ rows and `taken` column already present upgrades on next boot without manual deletion and all `get_resources_this_week_visible` / `get_all_hidden` queries return identical results before and after.
- **SC-002**: A fresh installation creates `resources.db` via migrations alone and passes the same integration tests as a migrated existing DB.
- **SC-003**: `MIGRATIONS.validate()` succeeds for the committed `migrations/` set and fails for a migration file with non-consecutive version, duplicate version after sorting, or invalid SQL; validate sorts lexicographically via `split_once('-')` — misordered filesystem names that sort to consecutive versions are not flagged, version order is defined by sorted names and not filesystem creation order.
- **SC-004**: Migration from `user_version = 0` to latest completes in under 500 ms on a Pi-class device for a DB with 100k rows (excluding `V2` backfill scan time) and leaves `user_version` equal to the number of migration files.
- **SC-005**: If a migration is forced to fail (e.g., injected invalid SQL), the process exits with non-zero status, logs the migration error, and does not serve HTTP; a subsequent restart with valid SQL succeeds without manual DB repair.
