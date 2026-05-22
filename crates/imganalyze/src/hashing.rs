//! Hashing exacto (BLAKE3 sobre los bytes del archivo) y perceptual
//! (dHash de 64 bits sobre la imagen reescalada).

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use image::{imageops::FilterType, DynamicImage};
use imgcore::ImgError;

/// BLAKE3 sobre el contenido del archivo. Detecta copias byte a byte.
pub fn content_hash(path: &Path) -> Result<[u8; 32], ImgError> {
    let file = File::open(path).map_err(|e| ImgError::io(path, e))?;
    let mut reader = BufReader::new(file);
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| ImgError::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(*hasher.finalize().as_bytes())
}

/// dHash de 64 bits.
///
/// 1. Reduce la imagen a 9×8 píxeles en escala de grises.
/// 2. Para cada fila, compara cada píxel con el de su derecha:
///    bit = 1 si `pixel > vecino_derecho`, 0 en caso contrario.
/// 3. Concatena 8×8 = 64 bits.
pub fn perceptual_hash(img: &DynamicImage) -> u64 {
    let small = img
        .resize_exact(9, 8, FilterType::Triangle)
        .to_luma8();
    let mut bits = 0u64;
    let mut bit_idx = 0u32;
    for y in 0..8u32 {
        for x in 0..8u32 {
            let left = small.get_pixel(x, y).0[0];
            let right = small.get_pixel(x + 1, y).0[0];
            if left > right {
                bits |= 1u64 << bit_idx;
            }
            bit_idx += 1;
        }
    }
    bits
}

/// Distancia de Hamming entre dos hashes perceptuales (XOR + popcount).
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::io::Write;

    fn dir_temp(nombre: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wehi_hashing_{}_{}",
            std::process::id(),
            nombre
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn imagen_gradiente(w: u32, h: u32) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let v = ((x * 255) / w.max(1)) as u8;
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    fn imagen_gradiente_decreciente(w: u32, h: u32) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let v = (255 - (x * 255) / w.max(1)) as u8;
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    fn imagen_uniforme(w: u32, h: u32, color: u8) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba([color, color, color, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn content_hash_de_dos_copias_identicas_coincide() {
        let dir = dir_temp("copias");
        let a = dir.join("a.bin");
        let b = dir.join("b.bin");
        let mut fa = File::create(&a).unwrap();
        let mut fb = File::create(&b).unwrap();
        let bytes = b"WeHi prueba de contenido identico";
        fa.write_all(bytes).unwrap();
        fb.write_all(bytes).unwrap();

        let ha = content_hash(&a).unwrap();
        let hb = content_hash(&b).unwrap();
        assert_eq!(ha, hb);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn content_hash_de_archivos_distintos_difiere() {
        let dir = dir_temp("distintos");
        let a = dir.join("a.bin");
        let b = dir.join("b.bin");
        std::fs::write(&a, b"hola").unwrap();
        std::fs::write(&b, b"chau").unwrap();
        assert_ne!(content_hash(&a).unwrap(), content_hash(&b).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn perceptual_hash_es_igual_para_imagen_identica() {
        let img = imagen_gradiente(200, 200);
        assert_eq!(perceptual_hash(&img), perceptual_hash(&img));
    }

    #[test]
    fn hamming_distance_imagen_similar_es_baja() {
        let a = imagen_gradiente(200, 200);
        // Variante muy ligeramente distinta: misma estructura, una franja
        // de píxeles diferente que no debe alterar el dHash más que en
        // unos pocos bits.
        let mut b_img = a.to_rgba8();
        for x in 0..200 {
            b_img.put_pixel(x, 0, Rgba([0, 0, 0, 255]));
        }
        let b = DynamicImage::ImageRgba8(b_img);
        let d = hamming_distance(perceptual_hash(&a), perceptual_hash(&b));
        assert!(d <= 6, "esperaba distancia baja, fue {d}");
    }

    #[test]
    fn hamming_distance_imagenes_distintas_es_alta() {
        // Uniforme → dHash 0 (left == right en toda fila).
        let a = imagen_uniforme(200, 200, 128);
        // Gradiente decreciente → left > right en toda fila → dHash ~ 1s.
        let b = imagen_gradiente_decreciente(200, 200);
        let d = hamming_distance(perceptual_hash(&a), perceptual_hash(&b));
        assert!(d >= 32, "esperaba distancia alta, fue {d}");
    }

    #[test]
    fn hamming_distance_es_correcto_en_casos_basicos() {
        assert_eq!(hamming_distance(0, 0), 0);
        assert_eq!(hamming_distance(0b1010, 0b0101), 4);
        assert_eq!(hamming_distance(u64::MAX, 0), 64);
    }
}
