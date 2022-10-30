use std::env;
use std::path::PathBuf;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

#[derive(Clone)]
pub struct ResourceStore {
    pool: Pool<SqliteConnectionManager>,
}

impl ResourceStore {
    pub fn get_all_hidden(&self) -> Vec<String> {
        let connection = self.pool.get().unwrap();
        let mut stmt = connection.prepare("SELECT id FROM hidden").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut ids: Vec<String> = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            ids.push(row.get(0).unwrap());
        }
        ids
    }
}

pub fn initialize() -> ResourceStore {
    let database_path = env::var("DATA_FOLDER").unwrap_or_else(|_| "./data".to_string());
    let resources_path = PathBuf::from(database_path).join("resources.db");
    let manager = SqliteConnectionManager::file(resources_path);
    let pool = r2d2::Pool::new(manager).unwrap();

    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS hidden (
            id    VARCHAR(128) PRIMARY KEY,
        )",
            (), // empty list of parameters.
        )
        .expect("table creation of 'hidden' failed");

    ResourceStore { pool }
}
