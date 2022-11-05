use std::path::PathBuf;
use std::{env, fs};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

#[derive(Clone)]
pub struct ResourceStore {
    persistent_file_store_pool: Pool<SqliteConnectionManager>,
    in_memory_pool: Pool<SqliteConnectionManager>,
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

    /// Checks if the specified resource id is hidden
    pub fn is_hidden(&self, resource_id: &str) -> bool {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("SELECT COUNT(id) FROM hidden WHERE id = ?")
            .unwrap();
        let mut rows = stmt.query([resource_id]).unwrap();

        let count: i32 = rows.next().unwrap().unwrap().get(0).unwrap();

        count == 1
    }

    /// Adds a image cache entry
    pub fn add_image_cache_entry(&self, id: String, data: &Vec<u8>) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("INSERT OR IGNORE INTO image_cache(id, data) VALUES(?, ?)")
            .unwrap();
        stmt.execute((&id, data))
            .unwrap_or_else(|_| panic!("Insertion of {id} failed"));
    }

    /// Get a image cache entry
    pub fn get_image_cache_entry(&self, id: String) -> Option<Vec<u8>> {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection
            .prepare("SELECT data FROM image_cache WHERE id = ?")
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
    pub fn clear_image_cache(&self) {
        let connection = self.persistent_file_store_pool.get().unwrap();
        let mut stmt = connection.prepare("DELETE FROM image_cache").unwrap();
        stmt.execute(())
            .unwrap_or_else(|_| panic!("Deletion of table 'image_cache' failed"));
    }
}

/// Initializes a new datastore in the $DATA_FOLDER folder and returns the instance
/// If no $DATA_FOLDER env var is configured, ./data/ is used
/// Creates data folder if it does not exists
/// Also creates all tables if needed
pub fn initialize() -> ResourceStore {
    let data_folder = env::var("DATA_FOLDER").unwrap_or_else(|_| "./data".to_string());
    fs::create_dir_all(&data_folder).unwrap_or_else(|_| panic!("Could not create {}", data_folder));
    let database_path = PathBuf::from(data_folder).join("resources.db");

    // Create persistent file store
    let file_store_pool = Pool::new(SqliteConnectionManager::file(database_path))
        .expect("persistent storage pool creation failed");

    // Create in memory store
    let in_memory_pool =
        Pool::new(SqliteConnectionManager::memory()).expect("In memory pool creation failed");

    create_table_hidden(&file_store_pool);
    create_table_image_cache(&file_store_pool);

    ResourceStore {
        persistent_file_store_pool: file_store_pool,
        in_memory_pool,
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

/// Creates the "image_cache" database table
fn create_table_image_cache(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS image_cache (id TEXT PRIMARY KEY, data BLOB);",
            (),
        )
        .expect("table creation of 'image_cache' failed");
}
