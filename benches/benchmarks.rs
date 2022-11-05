use std::path::PathBuf;
use std::{env, fs};

use criterion::{criterion_group, criterion_main, Criterion};

use rand::Rng;

const TEST_JPEG_URL: &str = "https://www.w3.org/People/mimasa/test/imgformat/img/w3c_home.jpg";

fn criterion_benchmark(_crit: &mut Criterion) {
    let base_dir = create_base_dir();
    let _base_dir_str = base_dir.as_path().to_str().unwrap();
    let _image_data = download_image();

    // TODO

    fs::remove_dir_all(base_dir).expect("Cleanup failed");
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
