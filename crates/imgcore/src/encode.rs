//! Codificación final a JPEG, PNG o WebP a partir de una
//! `image::DynamicImage` y una `OutputConfig`.

use std::io::Cursor;

use image::{codecs::jpeg::JpegEncoder, DynamicImage, ImageEncoder, ImageFormat};

use crate::config::{OutputConfig, OutputFormat};
use crate::error::ImgError;

/// Codifica una imagen a bytes según `cfg.format` y `cfg.quality`.
///
/// - **Jpeg**: lossy con la calidad indicada (0–100).
/// - **Png**: sin pérdida, el campo `quality` se ignora.
/// - **WebP**: lossy con calidad, vía el crate `webp` (libwebp).
pub fn encode_image(img: &DynamicImage, cfg: &OutputConfig) -> Result<Vec<u8>, ImgError> {
    match cfg.format {
        OutputFormat::Jpeg => encode_jpeg(img, cfg.quality),
        OutputFormat::Png => encode_png(img),
        OutputFormat::WebP => encode_webp(img, cfg.quality),
    }
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, ImgError> {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut bytes = Vec::with_capacity((w as usize) * (h as usize));
    let encoder = JpegEncoder::new_with_quality(&mut bytes, quality);
    encoder
        .write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| ImgError::processing(format!("encode jpeg: {e}")))?;
    Ok(bytes)
}

fn encode_png(img: &DynamicImage) -> Result<Vec<u8>, ImgError> {
    let mut bytes = Vec::new();
    img.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|e| ImgError::processing(format!("encode png: {e}")))?;
    Ok(bytes)
}

fn encode_webp(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, ImgError> {
    let encoder = webp::Encoder::from_image(img)
        .map_err(|e| ImgError::processing(format!("encode webp: {e}")))?;
    let q = (quality as f32).clamp(0.0, 100.0);
    let memory = encoder.encode(q);
    Ok(memory.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OutputFormat;
    use image::{Rgba, RgbaImage};
    use std::path::PathBuf;

    fn imagen(w: u32, h: u32) -> DynamicImage {
        let mut buf = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                buf.put_pixel(
                    x,
                    y,
                    Rgba([
                        (x as u8).wrapping_mul(3),
                        (y as u8).wrapping_mul(7),
                        100,
                        255,
                    ]),
                );
            }
        }
        DynamicImage::ImageRgba8(buf)
    }

    fn config(format: OutputFormat, quality: u8) -> OutputConfig {
        OutputConfig {
            format,
            quality,
            folder: PathBuf::from("."),
            filename_pattern: "{name}{ext}".into(),
            preserve_structure: false,
        }
    }

    #[test]
    fn jpeg_produce_bytes_decodificables() {
        let bytes = encode_image(&imagen(32, 32), &config(OutputFormat::Jpeg, 80)).unwrap();
        assert!(!bytes.is_empty());
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), 32);
        assert_eq!(decoded.height(), 32);
    }

    #[test]
    fn png_produce_bytes_decodificables() {
        let bytes = encode_image(&imagen(32, 32), &config(OutputFormat::Png, 100)).unwrap();
        assert!(!bytes.is_empty());
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), 32);
        assert_eq!(decoded.height(), 32);
    }

    #[test]
    fn webp_produce_bytes_decodificables() {
        let bytes = encode_image(&imagen(32, 32), &config(OutputFormat::WebP, 75)).unwrap();
        assert!(!bytes.is_empty());
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), 32);
        assert_eq!(decoded.height(), 32);
    }
}
