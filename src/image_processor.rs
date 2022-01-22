use std::io::Cursor;
use std::time::Instant;

use image::imageops::FilterType;
use image::io::Reader as ImageReader;

pub fn scale(resource_data: Vec<u8>, display_width: u32, display_height: u32) -> Vec<u8> {
    let s = Instant::now();

    let img = ImageReader::new(Cursor::new(resource_data))
        .with_guessed_format().unwrap()
        .decode().unwrap();
    println!("Decoding took {}s!", s.elapsed().as_secs());

    let resize = img.resize(display_width, display_height, FilterType::Nearest);
    println!("Scaling took {}s!", s.elapsed().as_secs());

    let mut bytes: Vec<u8> = Vec::new();
    resize.write_to(&mut bytes, image::ImageOutputFormat::Png).unwrap();

    println!("Buffer writing took {}s!", s.elapsed().as_secs());

    bytes
}
