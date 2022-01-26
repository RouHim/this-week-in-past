use std::io::Cursor;

use assertor::*;
use image::GenericImageView;
use image::io::Reader as ImageReader;

use crate::image_processor;

#[test]
fn scale_image() {
    // GIVEN is a jpeg image
    let image_data = reqwest::blocking::get(
        "https://raw.githubusercontent.com/ianare/exif-samples/master/jpg/Samsung_Digimax_i50_MP3.jpg"
    ).unwrap().bytes().unwrap().to_vec();

    // WHEN resolving the city name
    let scaled_image_buf = image_processor::optimize_image(
        image_data,
        1024,
        786,
        None
    );

    // THEN the resolved city name should be Koblenz
    let scaled_image = ImageReader::new(Cursor::new(scaled_image_buf))
        .with_guessed_format().unwrap()
        .decode().unwrap();
    assert_that!(scaled_image.width()).is_equal_to(1024);
    assert_that!(scaled_image.height()).is_equal_to(768);
}