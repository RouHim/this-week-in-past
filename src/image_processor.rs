use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::ImageReader;
use serde::{Deserialize, Serialize};

const MAX_DIM: u32 = 10_000;
const MAX_PIXELS: u64 = 50_000_000;

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
    // Guard before full decode: peek dimensions without allocating pixels
    if let Ok(reader) = ImageReader::new(Cursor::new(&resource_data)).with_guessed_format() {
        if let Ok((w, h)) = reader.into_dimensions() {
            if w > MAX_DIM || h > MAX_DIM || (w as u64 * h as u64) > MAX_PIXELS {
                log::warn!("{resource_path} | Rejected: {w}x{h} exceeds limit");
                return None;
            }
        }
    }

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
    // Post-decode guard: peek may have failed or been bypassed, enforce limits after allocation
    if image.width() > MAX_DIM
        || image.height() > MAX_DIM
        || (image.width() as u64 * image.height() as u64) > MAX_PIXELS
    {
        log::warn!(
            "{resource_path} | Rejected post-decode: {}x{} exceeds limit",
            image.width(),
            image.height()
        );
        return None;
    }

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
    fn adjust_image_rejects_huge_image() {
        // GIVEN a huge 11_000x11_000 image exceeding 10_000 limit
        let huge = image::RgbImage::new(11_000, 11_000);
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(huge)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();

        // WHEN attempting to adjust the huge image
        let out = adjust_image("huge.png".into(), buf, 100, 100, None);

        // THEN the image is rejected (None) to prevent OOM
        assert!(out.is_none(), "should reject >10000");
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
