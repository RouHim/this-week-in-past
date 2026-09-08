use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::ImageReader;
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
    let reader = match ImageReader::new(Cursor::new(&resource_data)).with_guessed_format() {
        Ok(reader) => reader,
        Err(error) => {
            log::error!("{resource_path} | Error: {}", error);
            return None;
        }
    };

    let mut image = match reader.decode() {
        Ok(image) => image,
        Err(error) => {
            log::error!("{resource_path} | Error: {}", error);
            return None;
        }
    };
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
        image.thumbnail(display_width, display_height)
    } else {
        image
    };

    // Encode as JPEG quality 90
    let mut bytes: Vec<u8> = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut bytes, 90);
    if enc.encode_image(&image).is_err() {
        return None;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjust_image_returns_jpeg_not_png() {
        // GIVEN a small PNG image buffer
        let img = image::RgbImage::new(10, 10);
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();

        // WHEN adjusting the image to 5x5
        let out = adjust_image("test.png".into(), buf, 5, 5, None).unwrap();

        // THEN the output is JPEG magic bytes (FF D8), not PNG
        assert_eq!(&out[0..2], &[0xFF, 0xD8], "must be JPEG magic");
    }

    #[test]
    fn adjust_image_accepts_panorama_without_limits() {
        // GIVEN a wide 11_000x100 panorama exceeding the former 10_000 dimension cap
        let pano = image::RgbImage::new(11_000, 100);
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(pano)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();

        // WHEN adjusting the panorama
        let out = adjust_image("pano.png".into(), buf, 100, 100, None);

        // THEN the image is processed (no dimension limits)
        let out = out.expect("panorama must not be rejected");
        assert_eq!(&out[0..2], &[0xFF, 0xD8], "must be JPEG magic");
    }

    #[test]
    fn adjust_image_always_decodes_even_for_zero_dims() {
        // GIVEN a valid PNG image buffer and zero display dimensions
        let img = image::RgbImage::new(10, 10);
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();

        // WHEN adjusting with 0x0 dimensions (no resize)
        let out = adjust_image("test.png".into(), buf.clone(), 0, 0, None).unwrap();

        // THEN the image is still decoded and re-encoded as JPEG
        assert_eq!(&out[0..2], &[0xFF, 0xD8]);
        assert_ne!(out, buf);
    }
}
