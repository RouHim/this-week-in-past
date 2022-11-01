use std::path::PathBuf;
use std::{env, fs};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rand::Rng;

const TEST_JPEG_URL: &str = "https://www.w3.org/People/mimasa/test/imgformat/img/w3c_home.jpg";

fn criterion_benchmark(crit: &mut Criterion) {
    let base_dir = create_base_dir();
    let base_dir_str = base_dir.as_path().to_str().unwrap();
    let image_data = download_image();

    // Prepare sqlite store
    let manager = SqliteConnectionManager::file(base_dir.join("test.db"));
    let connection_pool = Pool::new(manager).expect("Pool creation failed");
    create_test_table(&connection_pool);

    //
    //  WRITE BENCHES
    //
    crit.bench_function("cache_write_with_cacache", |bencher| {
        bencher.iter(|| {
            cache_write_with_cacache(black_box(base_dir_str), black_box(image_data.clone()))
        })
    });

    crit.bench_function("cache_write_with_sqlite", |bencher| {
        bencher.iter(|| {
            cache_write_with_sqlite(black_box(image_data.clone()), connection_pool.clone())
        })
    });

    //
    //  READ BENCHES
    //

    let cacache_keys: Vec<String> = cacache::list_sync(base_dir_str)
        .map(|key| key.unwrap().key)
        .collect();

    let sqlite_keys = get_all_sqlite_keys(connection_pool.clone());

    crit.bench_function("cache_read_with_cacache", |bencher| {
        bencher.iter(|| {
            cache_read_with_cacache(black_box(base_dir_str), black_box(cacache_keys.clone()))
        })
    });

    crit.bench_function("cache_read_with_sqlite", |bencher| {
        bencher.iter(|| {
            cache_read_with_sqlite(connection_pool.clone(), black_box(sqlite_keys.clone()))
        })
    });

    fs::remove_dir_all(base_dir).expect("Cleanup failed");
}

fn get_all_sqlite_keys(pool: Pool<SqliteConnectionManager>) -> Vec<String> {
    let connection = pool.get().unwrap();
    let mut stmt = connection.prepare("SELECT id FROM test").unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut ids: Vec<String> = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        ids.push(row.get(0).unwrap());
    }
    ids
}

fn cache_read_with_cacache(base_dir: &str, keys: Vec<String>) {
    keys.iter().for_each(|key| {
        let _: Vec<u8> = cacache::read_sync(base_dir, key).expect("could not read key");
    });
}

fn cache_read_with_sqlite(pool: Pool<SqliteConnectionManager>, keys: Vec<String>) {
    let connection = pool.get().unwrap();
    keys.iter().for_each(|key| {
        let mut stmt = connection
            .prepare("SELECT data FROM test WHERE id = ?")
            .unwrap();
        let mut rows = stmt.query([key]).unwrap();

        let _: Vec<u8> = rows.next().unwrap().unwrap().get(0).unwrap();
    });
}

fn create_test_table(pool: &Pool<SqliteConnectionManager>) {
    pool.get()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS test (id TEXT PRIMARY KEY, data BLOB);",
            (),
        )
        .expect("table creation failed");
}

fn cache_write_with_cacache(base_dir: &str, image_data: Vec<u8>) {
    let key = rand::thread_rng().gen::<u32>().to_string();

    cacache::write_sync(base_dir, key, image_data).expect("write failed");
}

fn cache_write_with_sqlite(image_data: Vec<u8>, pool: Pool<SqliteConnectionManager>) {
    let key = rand::thread_rng().gen::<u32>().to_string();

    let connection = pool.get().unwrap();
    let mut stmt = connection
        .prepare("INSERT OR IGNORE INTO test(id, data) VALUES(?, ?)")
        .unwrap();
    stmt.execute((key, image_data)).unwrap();
}

fn create_base_dir() -> PathBuf {
    let random_string = rand::thread_rng().gen::<u32>().to_string();
    let test_dir: PathBuf = env::temp_dir().join(&random_string);
    if !test_dir.exists() {
        fs::create_dir_all(&test_dir).unwrap();
    }
    test_dir
}

fn download_image() -> Vec<u8> {
    let response = ureq::get(TEST_JPEG_URL).call().unwrap();

    let len: usize = response.header("Content-Length").unwrap().parse().unwrap();

    let mut data: Vec<u8> = Vec::with_capacity(len);
    response
        .into_reader()
        .read_to_end(&mut data)
        .expect("write fail");

    data
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
