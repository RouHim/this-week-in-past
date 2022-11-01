use std::{env, fs};
use std::path::PathBuf;

use r2d2::Pool;
use r2d2_sqlite::rusqlite::OpenFlags;
use r2d2_sqlite::SqliteConnectionManager;

#[derive(Clone)]
pub struct ResourceStore {
    connection_pool: Pool<SqliteConnectionManager>,
}

/// Implements all functions acting on the data store instance
impl ResourceStore {
    /// Returns a list of all hidden resource ids
    pub fn get_all_hidden(&self) -> Vec<String> {
        let connection = self.connection_pool.get().unwrap();
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
        let connection = self.connection_pool.get().unwrap();
        let mut stmt = connection
            .prepare("INSERT OR IGNORE INTO hidden(id) VALUES(?)")
            .unwrap();
        stmt.execute([resource_id]).unwrap();
    }

    /// Removes the specified id from the hidden list
    pub fn remove_hidden(&self, resource_id: &str) {
        let connection = self.connection_pool.get().unwrap();
        let mut stmt = connection
            .prepare("DELETE FROM hidden WHERE ID = ?")
            .unwrap();
        stmt.execute([resource_id]).unwrap();
    }

    /// Checks if the specified resource id is hidden
    pub fn is_hidden(&self, resource_id: &str) -> bool {
        let connection = self.connection_pool.get().unwrap();
        let mut stmt = connection
            .prepare("SELECT COUNT(id) FROM hidden WHERE id = ?")
            .unwrap();
        let mut rows = stmt.query([resource_id]).unwrap();

        let count: i32 = rows.next().unwrap().unwrap().get(0).unwrap();

        count == 1
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

    let manager = SqliteConnectionManager::file(database_path).with_flags(OpenFlags::default());
    let connection_pool = Pool::new(manager).expect("Pool creation failed");

    create_table_hidden(&connection_pool);

    ResourceStore { connection_pool }
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
