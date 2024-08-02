use std::io::Cursor;

use image::imageops::FilterType;
use image::ImageReader;
use log::error;
use serde::{Deserialize, Serialize};

/// Represents the orientation of an image in two dimensions
/// rotation:               0, 90, 180 or 270
/// mirror_vertically:      true, if the image is mirrored vertically
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub struct ImageOrientation {
    pub rotation: u16,
    pub mirror_vertically: bool,
}

/// Adjusts the image to fit optimal to the browser resolution
/// Also fixes the orientation delivered by the exif image rotation
/// src: <https://sirv.com/help/articles/rotate-photos-to-be-upright/>
pub fn adjust_image(
    resource_path: String,
    resource_data: Vec<u8>,
    display_width: u32,
    display_height: u32,
    image_orientation: Option<ImageOrientation>,
) -> Option<Vec<u8>> {
    let read_result = ImageReader::new(Cursor::new(&resource_data))
        .with_guessed_format()
        .unwrap()
        .decode();

    if read_result.is_err() {
        error!("{resource_path} | Error: {}", read_result.unwrap_err());
        return None;
    }

    // Resize the image to the needed display size
    let mut image = read_result.unwrap();

    // Rotate or flip the image if needed
    image = if let Some(orientation) = image_orientation {
        let rotated = match orientation.rotation {
            90 => image.rotate90(),
            180 => image.rotate180(),
            270 => image.rotate270(),
            _ => image,
        };

        if orientation.mirror_vertically {
            rotated.flipv()
        } else {
            rotated
        }
    } else {
        image
    };

    image = if display_height > 0 && display_width > 0 {
        image.resize(display_width, display_height, FilterType::Triangle)
    } else {
        image
    };

    // Write the image to a buffer
    let mut bytes: Vec<u8> = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    Some(bytes)
}
