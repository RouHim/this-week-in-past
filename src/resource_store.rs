use std::fs;

use std::collections::HashMap;
use std::path::PathBuf;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

#[derive(Clone)]
pub struct ResourceStore {
    persistent_file_store_pool: Pool<SqliteConnectionManager>,
}

/// Implements all functions acting on the data store instance
impl ResourceStore {
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

    /// Gets a list of all visible resources this week in a random order
    /// Returns a list of resource IDs
    pub fn get_resource_this_week_visible_random(&self) -> Vec<String> {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare(
                r#"SELECT DISTINCT resources.id
                   FROM resources,
                        json_each(resources.value) json
                   WHERE json.key = 'taken'
                     AND json.value NOT NULL
                     AND strftime('%W', json.value) = strftime('%W', 'now')
                     AND resources.id NOT IN (SELECT id FROM hidden)
                   ORDER BY RANDOM();"#,
            )
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut ids: Vec<String> = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            ids.push(row.get(0).unwrap());
        }
        ids
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

    /// Adds a image cache entry, if an entry already exists it gets updated
    pub fn add_data_cache_entry(&self, id: String, data: &Vec<u8>) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("INSERT OR REPLACE INTO data_cache(id, data) VALUES(?, ?)")
            .unwrap();
        stmt.execute((&id, data))
            .unwrap_or_else(|_| panic!("Insertion of {id} failed"));
    }

    /// Get a image cache entry
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
            .unwrap_or_else(|_| panic!("Deletion of table 'data_cache' failed"));
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

    /// Returns a single random, non-hidden, resource id
    pub fn get_random_resource(&self) -> Option<String> {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("SELECT id FROM resources WHERE id NOT IN (SELECT id FROM hidden) ORDER BY RANDOM() LIMIT 1;")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();

        let first_entry = rows.next();

        if let Ok(first_entry) = first_entry {
            first_entry
                .map(|entry| entry.get(0))
                .and_then(|entry| entry.ok())
        } else {
            None
        }
    }

    /// Clears the complete resources cache
    pub fn clear_resources(&self) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection.prepare("DELETE FROM resources").unwrap();
        stmt.execute(())
            .unwrap_or_else(|_| panic!("Deletion of table 'resources' failed"));
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
            .unwrap_or_else(|_| panic!("Insertion of {id} failed"));
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
            .unwrap_or_else(|_| panic!("Insertion of {id} failed"));
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
}

/// Initializes a new datastore in the $DATA_FOLDER folder and returns the instance
/// If no $DATA_FOLDER env var is configured, ./data/ is used
/// Creates data folder if it does not exists
/// Also creates all tables if needed
pub fn initialize(data_folder: String) -> ResourceStore {
    fs::create_dir_all(&data_folder).unwrap_or_else(|_| panic!("Could not create {}", data_folder));
    let database_path = PathBuf::from(data_folder).join("resources.db");

    // Create persistent file store
    let persistent_file_store_pool = Pool::new(SqliteConnectionManager::file(database_path))
        .expect("persistent storage pool creation failed");

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
        .expect("table creation of 'hidden' failed");
}

/// Creates the "data_cache" database table
fn create_table_data_cache(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS data_cache (id TEXT PRIMARY KEY, data BLOB);",
            (),
        )
        .expect("table creation of 'data_cache' failed");
}

/// Creates the "geo_location_cache" database table
fn create_table_geo_location_cache(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS geo_location_cache (id TEXT PRIMARY KEY, value TEXT);",
            (),
        )
        .expect("table creation of 'geo_location_cache' failed");
}

/// Creates the "resources" database table
fn create_table_resources(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS resources (id TEXT PRIMARY KEY, value TEXT);",
            (),
        )
        .expect("table creation of 'resources' failed");
}
