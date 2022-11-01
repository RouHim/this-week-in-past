use std::{env, fs};
use std::path::PathBuf;

use criterion::{black_box, Criterion, criterion_group, criterion_main};
use r2d2::Pool;
use r2d2_sqlite::rusqlite::OpenFlags;
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


    crit.bench_function("cache_with_cacache", |bencher| {
        bencher.iter(|| cache_with_cacache(black_box(base_dir_str), black_box(image_data.clone())))
    });

    crit.bench_function("cache_with_sqlite", |bencher| {
        bencher.iter(|| cache_with_sqlite(black_box(image_data.clone()), connection_pool.clone()))
    });

    fs::remove_dir_all(base_dir).expect("Cleanup failed");
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

fn cache_with_cacache(base_dir: &str, image_data: Vec<u8>) {
    let key = rand::thread_rng().gen::<u32>().to_string();

    cacache::write_sync(base_dir, key, image_data).expect("write failed");
}

fn cache_with_sqlite(image_data: Vec<u8>, pool: Pool<SqliteConnectionManager>) {
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
