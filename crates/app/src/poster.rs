//! Decode and resize remote artwork for the software canvas.
//!
//! Decoding runs on the poster worker, never the render loop. The result is a
//! small poster-sized RGBA bitmap; the detail, hero, and card layouts all draw
//! it with the same cover transform.

use archiveplayer_tv_gfx::Image;
use image::imageops::FilterType;

/// Cards are about 272×408 on the 1920×1080 canvas and the detail poster is
/// 384×576. 360×540 keeps that crisp without holding full-size posters in RAM.
pub const POSTER_WIDTH: u32 = 360;
pub const POSTER_HEIGHT: u32 = 540;

pub fn decode_poster(bytes: &[u8]) -> Result<Image, String> {
    let decoded = image::load_from_memory(bytes).map_err(|error| error.to_string())?;
    // Archive.org's item image service returns a 180×101 frame, and TMDB can
    // return a wide backdrop. Neither is a poster; upscaling them into the
    // 2:3 slot looks worse than the deterministic artwork, so keep the
    // fallback for those sources.
    let ratio = decoded.width() as f32 / decoded.height().max(1) as f32;
    if decoded.width() < 240 || ratio > 1.2 {
        return Err("artwork is not a portrait poster".into());
    }
    let scaled = decoded.resize_to_fill(POSTER_WIDTH, POSTER_HEIGHT, FilterType::Triangle);
    let rgba = scaled.to_rgba8();
    Image::from_rgba(POSTER_WIDTH, POSTER_HEIGHT, rgba.into_raw())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn decodes_and_crops_to_poster_size() {
        let source = image::RgbaImage::from_pixel(400, 600, image::Rgba([180, 40, 20, 255]));
        let mut bytes = Vec::new();
        source
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        let poster = decode_poster(&bytes).unwrap();
        assert_eq!(
            (poster.width(), poster.height()),
            (POSTER_WIDTH, POSTER_HEIGHT)
        );
    }

    #[test]
    fn rejects_undecodable_bytes() {
        assert!(decode_poster(b"not an image").is_err());
    }

    #[test]
    fn rejects_low_resolution_or_wide_artwork() {
        for (width, height) in [(180, 101), (320, 180)] {
            let source =
                image::RgbaImage::from_pixel(width, height, image::Rgba([20, 40, 60, 255]));
            let mut bytes = Vec::new();
            source
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .unwrap();
            assert!(decode_poster(&bytes).is_err());
        }
    }
}
