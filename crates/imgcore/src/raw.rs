//! Decodificación de archivos RAW: solo extrae la previsualización
//! JPEG embebida. NO desarrolla el sensor (sin demosaicing).
//!
//! Se mantiene tras una interfaz mínima
//! (`is_raw`, `extract_raw_preview`) para que el motor RAW pueda
//! sustituirse en el futuro sin tocar el resto del crate.

use std::fs;
use std::path::Path;

use crate::error::ImgError;

const RAW_EXTENSIONS: &[&str] = &["cr2", "cr3", "nef", "nrw"];

/// Devuelve `true` si la extensión del path coincide con un formato
/// RAW soportado, sin distinción entre mayúsculas y minúsculas.
pub fn is_raw(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| RAW_EXTENSIONS.iter().any(|r| r.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Extrae el JPEG de previsualización embebido en un archivo RAW.
///
/// Estrategia: localiza el flujo JPEG más grande dentro del archivo
/// recorriendo los marcadores SOI (`FF D8`) y EOI (`FF D9`). Todos
/// los formatos RAW soportados (CR2, CR3, NEF, NRW) llevan al menos
/// un JPEG embebido del tamaño del sensor.
///
/// El crate `rawler` se mantiene como dependencia para futuras
/// extracciones que requieran metadatos específicos del fabricante;
/// la firma de esta función no cambia si se sustituye el motor.
pub fn extract_raw_preview(path: &Path) -> Result<Vec<u8>, ImgError> {
    let bytes = fs::read(path).map_err(|e| ImgError::io(path, e))?;
    largest_embedded_jpeg(&bytes)
        .ok_or_else(|| ImgError::raw_preview(path, "no se encontró JPEG embebido"))
}

fn largest_embedded_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let mut best: Option<(usize, usize)> = None;
    let mut i = 0usize;
    while i + 1 < data.len() {
        if data[i] == 0xFF && data[i + 1] == 0xD8 {
            let start = i;
            let mut j = i + 2;
            while j + 1 < data.len() {
                if data[j] == 0xFF && data[j + 1] == 0xD9 {
                    let end = j + 2;
                    let len = end - start;
                    if best.is_none_or(|(_, l)| len > l) {
                        best = Some((start, len));
                    }
                    i = end;
                    break;
                }
                j += 1;
            }
            if j + 1 >= data.len() {
                break;
            }
        } else {
            i += 1;
        }
    }
    best.map(|(start, len)| data[start..start + len].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detecta_extensiones_raw_sin_case() {
        assert!(is_raw(&PathBuf::from("foto.CR2")));
        assert!(is_raw(&PathBuf::from("foto.cr3")));
        assert!(is_raw(&PathBuf::from("a/b/c.NEF")));
        assert!(is_raw(&PathBuf::from("d.nrw")));
        assert!(!is_raw(&PathBuf::from("foto.jpg")));
        assert!(!is_raw(&PathBuf::from("foto")));
    }

    #[test]
    fn extrae_el_jpeg_mas_grande_de_un_buffer_sintetico() {
        let mut buf: Vec<u8> = Vec::new();
        // Pequeño JPEG: FFD8 ... 0x55 0x55 ... FFD9
        buf.extend_from_slice(&[0xFF, 0xD8, 0x55, 0x55, 0xFF, 0xD9]);
        // Grande: FFD8 + 32 bytes 0xAA + FFD9
        buf.extend_from_slice(&[0xFF, 0xD8]);
        buf.extend(std::iter::repeat_n(0xAAu8, 32));
        buf.extend_from_slice(&[0xFF, 0xD9]);

        let preview = largest_embedded_jpeg(&buf).expect("debe encontrar al menos uno");
        // El segundo (más grande) mide 2 + 32 + 2 = 36 bytes.
        assert_eq!(preview.len(), 36);
        assert_eq!(&preview[0..2], &[0xFF, 0xD8]);
        assert_eq!(&preview[preview.len() - 2..], &[0xFF, 0xD9]);
    }

    #[test]
    fn devuelve_none_si_no_hay_jpeg() {
        let basura = [0u8; 64];
        assert!(largest_embedded_jpeg(&basura).is_none());
    }
}
