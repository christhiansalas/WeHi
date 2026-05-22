//! Decodificación unificada: estándar (JPEG/PNG/WebP) y RAW
//! (CR2/CR3/NEF/NRW). En ambos casos se aplica la orientación EXIF
//! al final.

use std::fs;
use std::io::Cursor;
use std::path::Path;

use image::DynamicImage;

use crate::error::ImgError;
use crate::exif::{apply_orientation, read_orientation};
use crate::raw::{extract_raw_preview, is_raw};

const STANDARD_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// Decodifica el archivo apuntado por `path` a una imagen RGBA en
/// memoria, despachando según la extensión y aplicando la orientación
/// EXIF al final.
pub fn decode(path: &Path) -> Result<DynamicImage, ImgError> {
    if is_raw(path) {
        let preview = extract_raw_preview(path)?;
        return decode_bytes(path, &preview);
    }

    if !is_standard(path) {
        return Err(ImgError::unsupported(path));
    }

    let bytes = fs::read(path).map_err(|e| ImgError::io(path, e))?;
    decode_bytes(path, &bytes)
}

fn is_standard(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| STANDARD_EXTENSIONS.iter().any(|s| s.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

fn decode_bytes(path: &Path, bytes: &[u8]) -> Result<DynamicImage, ImgError> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| ImgError::decode(path, e.to_string()))?;
    let img = reader
        .decode()
        .map_err(|e| ImgError::decode(path, e.to_string()))?;
    let orientation = read_orientation(bytes);
    Ok(apply_orientation(img, orientation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Write;

    fn imagen_jpeg_en_disco(dir: &std::path::Path, nombre: &str) -> std::path::PathBuf {
        let mut buf = RgbaImage::new(8, 8);
        for y in 0..8 {
            for x in 0..8 {
                buf.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        let img = DynamicImage::ImageRgba8(buf).to_rgb8();
        let mut bytes: Vec<u8> = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .unwrap();
        let path = dir.join(nombre);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&bytes).unwrap();
        path
    }

    #[test]
    fn decodifica_un_jpeg_estandar() {
        let dir = std::env::temp_dir().join(format!("wehi_decode_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = imagen_jpeg_en_disco(&dir, "p.jpg");
        let img = decode(&path).unwrap();
        assert_eq!(img.width(), 8);
        assert_eq!(img.height(), 8);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn rechaza_extension_desconocida() {
        let dir = std::env::temp_dir().join(format!("wehi_decode_unk_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("foo.xyz");
        std::fs::write(&path, b"basura").unwrap();
        let err = decode(&path).unwrap_err();
        assert!(matches!(err, ImgError::UnsupportedFormat { .. }));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
