use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config;
use chrono::Datelike;
use log::{debug, error};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rand::seq::SliceRandom;

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
            new_year_resources.shuffle(&mut rand::thread_rng());
            return new_year_resources;
        }

        // Otherwise, we can query normally
        let regular_week_query = r#"
                   SELECT DISTINCT resources.id
                   FROM resources,
                        json_each(resources.value) json
                   WHERE json.key = 'taken'
                     AND json.value NOT NULL
                     AND resources.id NOT IN (SELECT id FROM hidden)
                     AND strftime('%m-%d', json.value) BETWEEN strftime('%m-%d', 'now', 'localtime', '-3 days') AND strftime('%m-%d', 'now', 'localtime', '+3 days')
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
               SELECT COUNT(DISTINCT resources.id)
               FROM resources,
                    json_each(resources.value) json
               WHERE json.key = 'taken'
                 AND json.value NOT NULL
                 AND resources.id NOT IN (SELECT id FROM hidden)
                 AND strftime('%m-%d', json.value) BETWEEN strftime('%m-%d', 'now', 'localtime', '-3 days') AND strftime('%m-%d', 'now', 'localtime', '+3 days')
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

    /// Adds an image cache entry, if an entry already exists it gets updated
    pub fn add_data_cache_entry(&self, id: String, data: &Vec<u8>) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("INSERT OR REPLACE INTO data_cache(id, data) VALUES(?, ?)")
            .unwrap();
        stmt.execute((&id, data))
            .unwrap_or_else(|error| panic!("Insertion of {id} failed:n{}", error));
    }

    /// Get an image cache entry
    pub fn get_data_cache_entry(&self, id: String) -> Option<Vec<u8>> {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("SELECT data FROM data_cache WHERE id = ?")
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

    /// Clears the complete image cache
    pub fn clear_data_cache(&self) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection.prepare("DELETE FROM data_cache").unwrap();
        stmt.execute(())
            .unwrap_or_else(|error| panic!("Deletion of table 'data_cache' failed.\n{}", error));
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
        let request_limit =
            (60 / config::get_refresh_interval_value()) * config::get_refresh_interval_value();
        let request_limit = (request_limit as f32 * 1.1) as usize;
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
            tx.execute(
                "INSERT OR REPLACE INTO resources(id, value) VALUES(?, ?)",
                (id.as_str(), value.as_str()),
            )
            .unwrap_or_else(|error| panic!("Insertion of {id} failed.\n{}", error));
        });

        tx.commit().expect("Transaction commit failed");
    }

    /// Adds a geo location cache entry, if an entry already exists it gets updated
    pub fn add_location(&self, id: String, value: String) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("INSERT OR REPLACE INTO geo_location_cache(id, value) VALUES(?, ?)")
            .unwrap();
        stmt.execute((&id, value))
            .unwrap_or_else(|error| panic!("Insertion of {id} failed:n{}", error));
    }

    /// Get a geo location entry by id entry
    pub fn get_location(&self, id: &str) -> Option<String> {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("SELECT value FROM geo_location_cache WHERE id = ?")
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

    /// Checks if the specified geo location entry exists
    pub fn location_exists(&self, id: &str) -> bool {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("SELECT COUNT(id) FROM geo_location_cache WHERE id = ?")
            .unwrap();
        let mut rows = stmt.query([id]).unwrap();

        let count: i32 = rows.next().unwrap().unwrap().get(0).unwrap();

        count == 1
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
/// Also creates all tables if needed
pub fn initialize(data_folder: &str) -> ResourceStore {
    fs::create_dir_all(data_folder)
        .unwrap_or_else(|error| panic!("Could not create data folder: {}", error));
    let database_path = PathBuf::from(data_folder).join("resources.db");

    // Create persistent file store and enable WAL mode
    let sqlite_manager = SqliteConnectionManager::file(database_path).with_init(|c| {
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

    create_table_hidden(&persistent_file_store_pool);
    create_table_data_cache(&persistent_file_store_pool);
    create_table_geo_location_cache(&persistent_file_store_pool);
    create_table_resources(&persistent_file_store_pool);

    ResourceStore {
        persistent_file_store_pool,
    }
}

/// Creates the "hidden" database table
fn create_table_hidden(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS hidden (id TEXT PRIMARY KEY);",
            (),
        )
        .unwrap_or_else(|error| panic!("table creation of 'hidden' failed.\n{}", error));
}

/// Creates the "data_cache" database table
fn create_table_data_cache(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS data_cache (id TEXT PRIMARY KEY, data BLOB);",
            (),
        )
        .unwrap_or_else(|error| panic!("table creation of 'data_cache' failed.\n{}", error));
}

/// Creates the "geo_location_cache" database table
fn create_table_geo_location_cache(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS geo_location_cache (id TEXT PRIMARY KEY, value TEXT);",
            (),
        )
        .unwrap_or_else(|error| {
            panic!("table creation of 'geo_location_cache' failed.\n{}", error)
        });
}

/// Creates the "resources" database table
fn create_table_resources(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS resources (id TEXT PRIMARY KEY, value TEXT);",
            (),
        )
        .unwrap_or_else(|error| panic!("table creation of 'resources' failed.\n{}", error));
}

/// Checks if today +-3 hits new year
fn range_hits_new_year() -> bool {
    let today = chrono::Local::now();
    today.month() == 12 && today.day() >= 29 || today.month() == 1 && today.day() <= 3
}

/// Returns the week query for the next year
fn get_next_year_query() -> &'static str {
    r#"
       SELECT DISTINCT resources.id
       FROM resources,
            json_each(resources.value) json
       WHERE json.key = 'taken'
         AND json.value NOT NULL
         AND resources.id NOT IN (SELECT id FROM hidden)
         AND strftime('%m-%d', json.value) BETWEEN '01-01' AND strftime('%m-%d', 'now', 'localtime', '+3 days')
   ;"#
}

/// Returns the week query for the last year
fn get_last_year_query() -> &'static str {
    r#"
       SELECT DISTINCT resources.id
       FROM resources,
            json_each(resources.value) json
       WHERE json.key = 'taken'
         AND json.value NOT NULL
         AND resources.id NOT IN (SELECT id FROM hidden)
         AND strftime('%m-%d', json.value) BETWEEN strftime('%m-%d', 'now', 'localtime', '-3 days') AND '12-31'
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
        row.get(0).unwrap()
    } else {
        0
    }
}

/// Returns the count query for the next year
fn get_next_year_count_query() -> &'static str {
    r#"
       SELECT COUNT(DISTINCT resources.id)
       FROM resources,
            json_each(resources.value) json
       WHERE json.key = 'taken'
         AND json.value NOT NULL
         AND resources.id NOT IN (SELECT id FROM hidden)
         AND strftime('%m-%d', json.value) BETWEEN '01-01' AND strftime('%m-%d', 'now', 'localtime', '+3 days')
   ;"#
}

/// Returns the count query for the last year
fn get_last_year_count_query() -> &'static str {
    r#"
       SELECT COUNT(DISTINCT resources.id)
       FROM resources,
            json_each(resources.value) json
       WHERE json.key = 'taken'
         AND json.value NOT NULL
         AND resources.id NOT IN (SELECT id FROM hidden)
         AND strftime('%m-%d', json.value) BETWEEN strftime('%m-%d', 'now', 'localtime', '-3 days') AND '12-31'
   ;"#
}
