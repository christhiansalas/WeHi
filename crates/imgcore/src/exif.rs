//! Lectura y aplicación de la orientación EXIF.
//!
//! Cubre los 8 valores de orientación definidos por el estándar.
//! Si el archivo no tiene EXIF o no contiene la etiqueta de
//! orientación, la imagen se devuelve sin cambios.

use std::io::Cursor;

use exif::{In, Reader, Tag};
use image::{imageops, DynamicImage};

/// Lee la etiqueta de orientación EXIF de un buffer de bytes
/// (típicamente JPEG o TIFF). Devuelve 1 si no hay EXIF o la
/// etiqueta está ausente.
pub fn read_orientation(bytes: &[u8]) -> u32 {
    let reader = Reader::new();
    let mut cursor = Cursor::new(bytes);
    let Ok(exif) = reader.read_from_container(&mut cursor) else {
        return 1;
    };
    exif.get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1)
}

/// Aplica una orientación EXIF (1–8) a una imagen. Valores fuera de
/// rango se tratan como 1 (sin cambios).
pub fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        1 => img,
        2 => DynamicImage::ImageRgba8(imageops::flip_horizontal(&img.to_rgba8())),
        3 => DynamicImage::ImageRgba8(imageops::rotate180(&img.to_rgba8())),
        4 => DynamicImage::ImageRgba8(imageops::flip_vertical(&img.to_rgba8())),
        5 => {
            let rotated = imageops::rotate90(&img.to_rgba8());
            DynamicImage::ImageRgba8(imageops::flip_horizontal(&rotated))
        }
        6 => DynamicImage::ImageRgba8(imageops::rotate90(&img.to_rgba8())),
        7 => {
            let rotated = imageops::rotate270(&img.to_rgba8());
            DynamicImage::ImageRgba8(imageops::flip_horizontal(&rotated))
        }
        8 => DynamicImage::ImageRgba8(imageops::rotate270(&img.to_rgba8())),
        _ => img,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn imagen_de_prueba() -> DynamicImage {
        let mut buf = RgbaImage::new(4, 2);
        // Distinguir esquinas: pinta solo el píxel (0,0) en rojo opaco.
        buf.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        DynamicImage::ImageRgba8(buf)
    }

    #[test]
    fn orientacion_1_no_modifica() {
        let img = imagen_de_prueba();
        let original = img.to_rgba8();
        let girada = apply_orientation(img, 1).to_rgba8();
        assert_eq!(original.as_raw(), girada.as_raw());
    }

    #[test]
    fn orientacion_6_intercambia_dimensiones() {
        let img = imagen_de_prueba();
        let (w, h) = (img.width(), img.height());
        let girada = apply_orientation(img, 6);
        assert_eq!(girada.width(), h);
        assert_eq!(girada.height(), w);
    }

    #[test]
    fn orientacion_fuera_de_rango_devuelve_original() {
        let img = imagen_de_prueba();
        let (w, h) = (img.width(), img.height());
        let resultado = apply_orientation(img, 99);
        assert_eq!(resultado.width(), w);
        assert_eq!(resultado.height(), h);
    }

    #[test]
    fn read_orientation_devuelve_1_si_no_hay_exif() {
        let basura = [0u8; 16];
        assert_eq!(read_orientation(&basura), 1);
    }
}
