use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::config;
use chrono::Datelike;
use include_dir::{include_dir, Dir};
use log::{debug, error};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rand::seq::SliceRandom;
use rusqlite_migration::Migrations;

static MIGRATIONS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");
pub static MIGRATIONS: LazyLock<Migrations<'_>> = LazyLock::new(|| {
    Migrations::from_directory(&MIGRATIONS_DIR)
        .expect("Failed to load migrations from migrations/ directory")
});

#[derive(Clone)]
pub struct ResourceStore {
    persistent_file_store_pool: Pool<SqliteConnectionManager>,
}

/// Implements all functions acting on the data store instance
impl ResourceStore {
    /// Cleanup database
    pub fn vacuum(&self) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection.prepare("VACUUM").unwrap();
        stmt.execute([]).unwrap_or_else(|error| {
            error!("VACUUM failed. Error:\n{}", error);
            0
        });
    }

    /// Returns a list of all hidden resource ids
    pub fn get_all_hidden(&self) -> Vec<String> {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection.prepare("SELECT id FROM hidden").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut ids: Vec<String> = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            ids.push(row.get(0).unwrap());
        }
        ids
    }

    /// Gets a list of all visible resources for the current week
    /// Returns a list of resource ids
    pub fn get_resources_this_week_visible_random(&self) -> Vec<String> {
        let connection = self.persistent_file_store_pool.get().unwrap();

        // Check if we are in the new year week
        // If yes, we need to query differently
        if range_hits_new_year() {
            debug!("🎊 New year week detected");
            let mut new_year_resources = [
                execute_query(&connection, get_last_year_query()),
                execute_query(&connection, get_next_year_query()),
            ]
            .concat();
            new_year_resources.shuffle(&mut rand::rng());
            return new_year_resources;
        }

        // Otherwise, we can query normally
        let regular_week_query = r#"
                   SELECT DISTINCT id FROM resources
                   WHERE taken IS NOT NULL
                     AND id NOT IN (SELECT id FROM hidden)
                     AND strftime('%m-%d', taken) BETWEEN strftime('%m-%d','now','localtime','-3 days')
                                                   AND strftime('%m-%d','now','localtime','+3 days')
                   ORDER BY RANDOM()
                   ;"#;
        execute_query(&connection, regular_week_query)
    }

    /// Returns the count of all visible resources for the current week
    pub fn get_resources_this_week_visible_count(&self) -> usize {
        let connection = self.persistent_file_store_pool.get().unwrap();

        // Check if we are in the new year week
        // If yes, we need to query differently
        if range_hits_new_year() {
            debug!("🎊 New year week detected");
            let new_year_resources_count = [
                execute_count_query(&connection, get_last_year_count_query()),
                execute_count_query(&connection, get_next_year_count_query()),
            ]
            .iter()
            .sum();
            return new_year_resources_count;
        }

        // Otherwise, we can query normally
        let regular_week_query = r#"
               SELECT COUNT(DISTINCT id) FROM resources
               WHERE taken IS NOT NULL
                 AND id NOT IN (SELECT id FROM hidden)
                 AND strftime('%m-%d', taken) BETWEEN strftime('%m-%d','now','localtime','-3 days')
                                                  AND strftime('%m-%d','now','localtime','+3 days')
               ;"#;
        execute_count_query(&connection, regular_week_query)
    }

    /// Sets the specified resource id as hidden
    pub fn add_hidden(&self, resource_id: &str) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("INSERT OR IGNORE INTO hidden(id) VALUES(?)")
            .unwrap();
        stmt.execute([resource_id]).unwrap();
    }

    /// Removes the specified id from the hidden list
    pub fn remove_hidden(&self, resource_id: &str) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("DELETE FROM hidden WHERE ID = ?")
            .unwrap();
        stmt.execute([resource_id]).unwrap();
    }

    /// Returns an id list of all resources, including hidden resources
    pub fn get_all_resource_ids(&self) -> Vec<String> {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection.prepare("SELECT id FROM resources").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut ids: Vec<String> = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            ids.push(row.get(0).unwrap());
        }
        ids
    }

    /// Get a resource value by id entry
    /// Returns a optional resource value
    pub fn get_resource(&self, id: &str) -> Option<String> {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("SELECT value FROM resources WHERE id = ?")
            .unwrap();
        let mut rows = stmt.query([id]).unwrap();

        let first_entry = rows.next();

        if let Ok(first_entry) = first_entry {
            first_entry
                .map(|entry| entry.get(0))
                .and_then(|entry| entry.ok())
        } else {
            None
        }
    }

    /// Returns random resources, non-hidden, resource id
    pub fn get_random_resources(&self) -> Vec<String> {
        let connection = self.persistent_file_store_pool.get().unwrap();
        // Request limit is calculated by: (60/SLIDESHOW_INTERVAL)*REFRESH_INTERVAL * 10% buffer
        let request_limit = (60. / config::get_slideshow_interval_value() as f32)
            * config::get_refresh_interval_value() as f32;
        let request_limit = (request_limit * 1.1) as usize;
        let mut stmt = connection
            .prepare(&format!(
                r#"
                SELECT id FROM resources 
                WHERE id NOT IN (SELECT id FROM hidden) 
                ORDER BY RANDOM() 
                LIMIT {};"#,
                request_limit
            ))
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut ids: Vec<String> = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            ids.push(row.get(0).unwrap());
        }
        ids
    }

    /// Clears the complete resources cache
    pub fn clear_resources(&self) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection.prepare("DELETE FROM resources").unwrap();
        stmt.execute(())
            .unwrap_or_else(|error| panic!("Deletion of table 'resources' failed.\n{}", error));
    }

    /// Batch inserts or updates resources
    pub fn add_resources(&self, resources: HashMap<String, String>) {
        let mut connection = self.persistent_file_store_pool.get().unwrap();
        let tx = connection
            .transaction()
            .expect("Failed to create transaction");

        resources.iter().for_each(|(id, value)| {
            let taken: Option<String> = serde_json::from_str::<serde_json::Value>(value.as_str())
                .ok()
                .and_then(|v| {
                    v.get("taken")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                });
            tx.execute(
                "INSERT OR REPLACE INTO resources(id, value, taken) VALUES(?1, ?2, ?3)",
                rusqlite::params![id.as_str(), value.as_str(), taken],
            )
            .unwrap_or_else(|error| panic!("Insertion of {id} failed.\n{}", error));
        });

        tx.commit().expect("Transaction commit failed");
    }

    /// Returns the current time of the database
    pub fn get_database_time(&self) -> String {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("SELECT datetime('now', 'localtime')")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();

        let first_entry = rows.next();

        if let Ok(first_entry) = first_entry {
            first_entry
                .map(|entry| entry.get(0))
                .and_then(|entry| entry.ok())
                .unwrap_or("N/A".to_string())
        } else {
            "N/A".to_string()
        }
    }
}

/// Initializes a new datastore in the $DATA_FOLDER folder and returns the instance
/// If no $DATA_FOLDER env var is configured, ./data/ is used
/// Creates data folder if it does not exists
/// Also creates all tables via versioned migrations (see `MIGRATIONS`)
pub fn initialize(data_folder: &str) -> ResourceStore {
    fs::create_dir_all(data_folder)
        .unwrap_or_else(|error| panic!("Could not create data folder: {}", error));
    let _ = std::fs::create_dir_all(PathBuf::from(data_folder).join("cache"));
    let database_path = PathBuf::from(data_folder).join("resources.db");

    // Create persistent file store and enable WAL mode
    let sqlite_manager = SqliteConnectionManager::file(&database_path).with_init(|c| {
        c.execute_batch(
            "
            PRAGMA journal_mode=WAL;            -- better write-concurrency
            PRAGMA synchronous=NORMAL;          -- fsync only in critical moments
            PRAGMA wal_autocheckpoint=1000;     -- write WAL changes back every 1000 pages
            PRAGMA wal_checkpoint(TRUNCATE);    -- free some space by truncating possibly massive WAL files from the last run
        ",
        )
    });

    let persistent_file_store_pool = Pool::new(sqlite_manager)
        .unwrap_or_else(|error| panic!("Could not create persistent file store: {}", error));

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
            // FR-008: fail fast, visible even if logger not yet init (tests, early init)
            // eprintln! ensures Pi operator sees error when log sink not yet configured; error! satisfies structured logging when available.
            eprintln!("Database migration failed: {}", e);
            error!("Database migration failed: {}", e);
            panic!("Database migration failed: {}", e);
        }
    }

    ResourceStore {
        persistent_file_store_pool,
    }
}

/// Checks if today +-3 hits new year
fn range_hits_new_year() -> bool {
    let today = chrono::Local::now();
    today.month() == 12 && today.day() >= 29 || today.month() == 1 && today.day() <= 3
}

/// Returns the week query for the next year
fn get_next_year_query() -> &'static str {
    r#"
       SELECT DISTINCT id
       FROM resources
       WHERE taken IS NOT NULL
         AND id NOT IN (SELECT id FROM hidden)
         AND strftime('%m-%d', taken) BETWEEN '01-01' AND strftime('%m-%d','now','localtime','+3 days')
   ;"#
}

/// Returns the week query for the last year
fn get_last_year_query() -> &'static str {
    r#"
       SELECT DISTINCT id
       FROM resources
       WHERE taken IS NOT NULL
         AND id NOT IN (SELECT id FROM hidden)
         AND strftime('%m-%d', taken) BETWEEN strftime('%m-%d','now','localtime','-3 days') AND '12-31'
   ;"#
}

/// Executes the specified query and returns a list of resource ids
fn execute_query(
    connection: &PooledConnection<SqliteConnectionManager>,
    week_query: &str,
) -> Vec<String> {
    let mut stmt = connection.prepare(week_query).unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut resources: Vec<String> = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let id = row.get(0).unwrap();
        resources.push(id);
    }
    resources
}

/// Executes the specified query and returns the count of resource ids
fn execute_count_query(
    connection: &PooledConnection<SqliteConnectionManager>,
    count_query: &str,
) -> usize {
    let mut stmt = connection.prepare(count_query).unwrap();
    let mut rows = stmt.query([]).unwrap();
    if let Ok(Some(row)) = rows.next() {
        row.get::<_, i64>(0).unwrap() as usize
    } else {
        0
    }
}

/// Returns the count query for the next year
fn get_next_year_count_query() -> &'static str {
    r#"
       SELECT COUNT(DISTINCT id)
       FROM resources
       WHERE taken IS NOT NULL
         AND id NOT IN (SELECT id FROM hidden)
         AND strftime('%m-%d', taken) BETWEEN '01-01' AND strftime('%m-%d','now','localtime','+3 days')
   ;"#
}

/// Returns the count query for the last year
fn get_last_year_count_query() -> &'static str {
    r#"
       SELECT COUNT(DISTINCT id)
       FROM resources
       WHERE taken IS NOT NULL
         AND id NOT IN (SELECT id FROM hidden)
         AND strftime('%m-%d', taken) BETWEEN strftime('%m-%d','now','localtime','-3 days') AND '12-31'
   ;"#
}

#[cfg(test)]
mod tests {
    #[test]
    fn week_query_returns_this_week_via_taken() {
        // GIVEN a store with one resource taken today and one old resource
        let dir = tempfile::tempdir().unwrap();
        let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
        let today_str = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let mut map = std::collections::HashMap::new();
        map.insert(
            "this_week_id".into(),
            format!(r#"{{"id":"this_week_id","taken":"{}"}}"#, today_str),
        );
        map.insert(
            "old_id".into(),
            r#"{"id":"old_id","taken":"2000-01-15T12:00:00"}"#.into(),
        );
        store.add_resources(map);

        // WHEN querying visible resources for the current week
        let ids = store.get_resources_this_week_visible_random();

        // THEN the resource taken today is returned
        assert!(ids.contains(&"this_week_id".to_string()));
    }

    #[test]
    fn week_query_uses_taken_index() {
        // GIVEN an initialized store with a taken index
        let dir = tempfile::tempdir().unwrap();
        let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
        let conn = store.persistent_file_store_pool.get().unwrap();

        // WHEN explaining the query plan for the taken-based week query
        let plan: String = conn.query_row(
            "EXPLAIN QUERY PLAN SELECT DISTINCT id FROM resources WHERE taken IS NOT NULL AND strftime('%m-%d', taken) BETWEEN '01-01' AND '12-31'",
            [], |r| r.get(3)
        ).unwrap();

        // THEN the index idx_resources_taken is used and no helper uses json_each
        assert!(
            plan.contains("idx_resources_taken") || plan.contains("USING INDEX"),
            "plan: {}",
            plan
        );
        assert!(
            !crate::resource_store::get_next_year_query().contains("json_each"),
            "get_next_year_query still uses json_each"
        );
        assert!(
            !crate::resource_store::get_last_year_query().contains("json_each"),
            "get_last_year_query still uses json_each"
        );
        assert!(
            !crate::resource_store::get_next_year_count_query().contains("json_each"),
            "get_next_year_count_query still uses json_each"
        );
        assert!(
            !crate::resource_store::get_last_year_count_query().contains("json_each"),
            "get_last_year_count_query still uses json_each"
        );
    }

    #[test]
    fn initialize_creates_taken_column_and_index_and_backfills() {
        // GIVEN a legacy DB with user_version=0 and a row where taken is NULL
        // (simulates DB created by old initialize before V2 backfill)
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("resources.db");
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE hidden (id TEXT PRIMARY KEY);
                 CREATE TABLE resources (id TEXT PRIMARY KEY, value TEXT, taken TEXT);
                 CREATE TABLE geo_location_cache (id TEXT PRIMARY KEY, value TEXT);
                 CREATE TABLE data_cache (id TEXT PRIMARY KEY, data BLOB);
                 CREATE INDEX IF NOT EXISTS idx_resources_taken ON resources(taken);
                 INSERT INTO resources(id,value,taken) VALUES('legacy1','{\"id\":\"legacy1\",\"taken\":\"2021-03-15T12:00:00\"}',NULL);
                 PRAGMA user_version = 0;",
            )
            .unwrap();
            let uv: i32 = conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(uv, 0);
        }

        // WHEN initializing (which runs migrations V1-V3)
        let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
        let conn = store.persistent_file_store_pool.get().unwrap();

        // THEN the taken column, index, and backfilled value exist
        let col: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('resources') WHERE name='taken'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(col, 1);
        let idx: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_resources_taken'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx, 1);
        let taken: Option<String> = conn
            .query_row("SELECT taken FROM resources WHERE id='legacy1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(taken, Some("2021-03-15T12:00:00".to_string()));
        // AND user_version advanced to latest (4) — V4 drops geo_location_cache
        let uv: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, 4);
        // AND data_cache dropped by V3 and geo_location_cache dropped by V4
        let cache: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='data_cache'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cache, 0);
        let geo_cache: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='geo_location_cache'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(geo_cache, 0);
    }

    #[test]
    fn migrations_validate_succeeds() {
        assert!(crate::resource_store::MIGRATIONS.validate().is_ok());
    }

    #[test]
    fn migrations_user_version_equals_migration_count_on_fresh_db() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
        let conn = store.persistent_file_store_pool.get().unwrap();
        let uv: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            uv, 4,
            "user_version must equal number of migration files (4)"
        );
    }

    #[test]
    fn migration_failure_is_atomic_and_retriable() {
        use rusqlite::Connection;
        use rusqlite_migration::{Migrations, M};
        let mut conn = Connection::open_in_memory().unwrap();
        let good = Migrations::new(vec![M::up("CREATE TABLE t1 (x TEXT);")]);
        good.to_latest(&mut conn).unwrap();
        assert_eq!(usize::from(good.current_version(&conn).unwrap()), 1);
        let bad = Migrations::new(vec![
            M::up("CREATE TABLE t1 (x TEXT);"),
            M::up("THIS IS NOT SQL;"),
        ]);
        let res = bad.to_latest(&mut conn);
        assert!(res.is_err(), "bad migration should fail");
        assert_eq!(usize::from(bad.current_version(&conn).unwrap()), 1);
        let fixed = Migrations::new(vec![
            M::up("CREATE TABLE t1 (x TEXT);"),
            M::up("CREATE TABLE t2 (y TEXT);"),
        ]);
        fixed.to_latest(&mut conn).unwrap();
        assert_eq!(usize::from(fixed.current_version(&conn).unwrap()), 2);
    }
    #[test]
    fn fresh_install_and_migrated_db_have_identical_schema() {
        let fresh_dir = tempfile::tempdir().unwrap();
        let fresh = crate::resource_store::initialize(fresh_dir.path().to_str().unwrap());
        let fresh_conn = fresh.persistent_file_store_pool.get().unwrap();
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
                 CREATE INDEX idx_resources_taken ON resources(taken);",
            )
            .unwrap();
            conn.execute("PRAGMA user_version = 0", []).unwrap();
        }
        let migrated = crate::resource_store::initialize(migrated_dir.path().to_str().unwrap());
        let mig_conn = migrated.persistent_file_store_pool.get().unwrap();
        // Compare schema via pragma_table_info for resources
        let fresh_cols: Vec<(String, String)> = {
            let mut stmt = fresh_conn
                .prepare("SELECT name, type FROM pragma_table_info('resources') ORDER BY cid")
                .unwrap();
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0).unwrap(),
                    r.get::<_, String>(1).unwrap(),
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        };
        let mig_cols: Vec<(String, String)> = {
            let mut stmt = mig_conn
                .prepare("SELECT name, type FROM pragma_table_info('resources') ORDER BY cid")
                .unwrap();
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0).unwrap(),
                    r.get::<_, String>(1).unwrap(),
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        };
        assert_eq!(
            fresh_cols, mig_cols,
            "fresh and migrated resources columns must match"
        );
        // Also check taken index exists in both
        for conn in [&fresh_conn, &mig_conn] {
            let idx: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_resources_taken'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(idx, 1, "idx_resources_taken missing");
        }
        for tbl in ["hidden", "resources"] {
            let cnt: i32 = mig_conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    rusqlite::params![tbl],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(cnt, 1, "table {} missing", tbl);
        }
        // geo_location_cache is dropped by V4 — must be absent in both fresh and migrated
        for (label, conn) in [("fresh", &fresh_conn), ("migrated", &mig_conn)] {
            let cnt: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='geo_location_cache'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(cnt, 0, "{} still has geo_location_cache", label);
        }
        for (label, conn) in [("fresh", &fresh_conn), ("migrated", &mig_conn)] {
            let cnt: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='data_cache'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(cnt, 0, "{} still has data_cache", label);
        }
    }

    #[test]
    fn existing_db_with_user_version_0_upgrades_without_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("resources.db");
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
                 PRAGMA user_version = 0;",
            )
            .unwrap();
            let uv: i32 = conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(uv, 0);
        }
        let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
        let conn = store.persistent_file_store_pool.get().unwrap();
        let uv: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, 4);
        let photo: Option<String> = conn
            .query_row("SELECT value FROM resources WHERE id='photo1'", [], |r| {
                r.get(0)
            })
            .ok();
        assert!(photo.is_some());
        let hidden: Vec<String> = store.get_all_hidden();
        assert!(hidden.contains(&"hidden1".to_string()));
        // geo_location_cache is dropped by V4
        let geo_cache: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='geo_location_cache'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(geo_cache, 0);
        let cache_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='data_cache'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cache_count, 0);
        let store2 = crate::resource_store::initialize(dir.path().to_str().unwrap());
        let uv2: i32 = store2
            .persistent_file_store_pool
            .get()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv2, 4);
    }

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
            let uv: i32 = conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(uv, 0);
            let has_taken: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('resources') WHERE name='taken'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(has_taken, 0);
        }
        // WHEN initializing
        let store = crate::resource_store::initialize(dir.path().to_str().unwrap());
        let conn = store.persistent_file_store_pool.get().unwrap();
        // THEN user_version 4, taken backfilled, hidden preserved, data_cache+geo_cache dropped
        let uv: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, 4);
        let taken: Option<String> = conn
            .query_row("SELECT taken FROM resources WHERE id='old1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(taken, Some("2020-03-15T10:00:00".to_string()));
        let idx: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_resources_taken'",
            [], |r| r.get(0)
        ).unwrap();
        assert_eq!(idx, 1);
        let hidden: Vec<String> = store.get_all_hidden();
        assert!(hidden.contains(&"h1".to_string()));
        let cache: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='data_cache'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cache, 0);
        let geo_cache: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='geo_location_cache'",
            [], |r| r.get(0)
        ).unwrap();
        assert_eq!(geo_cache, 0);
    }
}
