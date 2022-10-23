use std::io::Cursor;

use image::imageops::FilterType;
use image::io::Reader;
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
/// src: https://sirv.com/help/articles/rotate-photos-to-be-upright/
pub fn optimize_image(
    resource_data: Vec<u8>,
    display_width: u32,
    display_height: u32,
    image_orientation: Option<ImageOrientation>,
) -> Vec<u8> {
    let original_image = Reader::new(Cursor::new(resource_data))
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();

    // Resize the image to the needed display size
    let resized = original_image.resize(display_width, display_height, FilterType::Triangle);

    // Rotate or flip the image if needed
    let fixed_orientation = if let Some(orientation) = image_orientation {
        let rotated = match orientation.rotation {
            90 => resized.rotate90(),
            180 => resized.rotate180(),
            270 => resized.rotate270(),
            _ => resized,
        };

        if orientation.mirror_vertically {
            rotated.flipv()
        } else {
            rotated
        }
    } else {
        resized
    };

    // Write the image to a buffer
    let mut bytes: Vec<u8> = Vec::new();
    fixed_orientation
        .write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::Png)
        .unwrap();
    bytes
}
