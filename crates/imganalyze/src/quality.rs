//! Métricas objetivas de calidad calculadas sobre el canal de
//! LUMINANCIA de una copia de trabajo reducida (lado mayor ~1024 px).
//!
//! Reducir antes de medir hace que los valores sean comparables
//! entre fotos de distintas resoluciones dentro del mismo lote.

use image::{imageops::FilterType, DynamicImage, GenericImageView, GrayImage};

use crate::record::QualityMetrics;

const TARGET_LONGEST_SIDE: u32 = 1024;

/// Calcula las cinco métricas objetivas de calidad de una imagen.
pub fn compute_quality(img: &DynamicImage) -> QualityMetrics {
    let luma = reduced_luma(img);
    let total = (luma.width() as usize) * (luma.height() as usize);
    if total == 0 {
        return QualityMetrics::default();
    }

    let (mean, contrast) = mean_and_contrast(&luma);
    QualityMetrics {
        sharpness_raw: laplacian_variance(&luma),
        clipped_highlights_pct: pct_at_or_above(&luma, 250),
        clipped_shadows_pct: pct_at_or_below(&luma, 5),
        mean_luma: mean,
        contrast,
    }
}

fn reduced_luma(img: &DynamicImage) -> GrayImage {
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    let small = if longest > TARGET_LONGEST_SIDE {
        let scale = TARGET_LONGEST_SIDE as f32 / longest as f32;
        let nw = ((w as f32 * scale).round() as u32).max(1);
        let nh = ((h as f32 * scale).round() as u32).max(1);
        img.resize_exact(nw, nh, FilterType::Triangle)
    } else {
        img.clone()
    };
    small.to_luma8()
}

/// Varianza del Laplaciano 3×3. Más varianza = más bordes nítidos.
fn laplacian_variance(luma: &GrayImage) -> f32 {
    let (w, h) = luma.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }
    let inner = ((w - 2) as usize) * ((h - 2) as usize);
    let mut sum: f64 = 0.0;
    let mut sum_sq: f64 = 0.0;
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let c = luma.get_pixel(x, y).0[0] as i32 * 4;
            let up = luma.get_pixel(x, y - 1).0[0] as i32;
            let dn = luma.get_pixel(x, y + 1).0[0] as i32;
            let lf = luma.get_pixel(x - 1, y).0[0] as i32;
            let rt = luma.get_pixel(x + 1, y).0[0] as i32;
            let lap = (c - up - dn - lf - rt) as f64;
            sum += lap;
            sum_sq += lap * lap;
        }
    }
    let n = inner as f64;
    let mean = sum / n;
    let var = (sum_sq / n) - mean * mean;
    var.max(0.0) as f32
}

fn pct_at_or_above(luma: &GrayImage, threshold: u8) -> f32 {
    let total = luma.pixels().count();
    if total == 0 {
        return 0.0;
    }
    let n = luma.pixels().filter(|p| p.0[0] >= threshold).count();
    100.0 * n as f32 / total as f32
}

fn pct_at_or_below(luma: &GrayImage, threshold: u8) -> f32 {
    let total = luma.pixels().count();
    if total == 0 {
        return 0.0;
    }
    let n = luma.pixels().filter(|p| p.0[0] <= threshold).count();
    100.0 * n as f32 / total as f32
}

fn mean_and_contrast(luma: &GrayImage) -> (f32, f32) {
    let total = (luma.width() as usize) * (luma.height() as usize);
    if total == 0 {
        return (0.0, 0.0);
    }
    let mut sum: u64 = 0;
    let mut sum_sq: u64 = 0;
    for p in luma.pixels() {
        let v = p.0[0] as u64;
        sum += v;
        sum_sq += v * v;
    }
    let n = total as f64;
    let mean = sum as f64 / n;
    let var = (sum_sq as f64 / n) - mean * mean;
    (mean as f32, var.max(0.0).sqrt() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn uniforme(w: u32, h: u32, color: u8) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba([color, color, color, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    fn tablero(w: u32, h: u32) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let v: u8 = if (x + y) % 2 == 0 { 0 } else { 255 };
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn imagen_nitida_da_mayor_sharpness_que_borrosa() {
        // El tablero es de altísima frecuencia; el uniforme es plano.
        let nitida = tablero(200, 200);
        let borrosa = uniforme(200, 200, 128);
        let q_nitida = compute_quality(&nitida);
        let q_borrosa = compute_quality(&borrosa);
        assert!(
            q_nitida.sharpness_raw > q_borrosa.sharpness_raw,
            "esperaba nítida > borrosa: {} vs {}",
            q_nitida.sharpness_raw,
            q_borrosa.sharpness_raw
        );
        assert!(
            q_nitida.sharpness_raw > 100.0,
            "tablero debería dar varianza alta, fue {}",
            q_nitida.sharpness_raw
        );
    }

    #[test]
    fn imagen_blanca_tiene_alto_porcentaje_de_altas_recortadas() {
        let q = compute_quality(&uniforme(200, 200, 255));
        assert!(q.clipped_highlights_pct >= 99.0);
        assert!(q.clipped_shadows_pct <= 1.0);
        assert!(q.mean_luma >= 250.0);
    }

    #[test]
    fn imagen_negra_tiene_alto_porcentaje_de_sombras_recortadas() {
        let q = compute_quality(&uniforme(200, 200, 0));
        assert!(q.clipped_shadows_pct >= 99.0);
        assert!(q.clipped_highlights_pct <= 1.0);
        assert!(q.mean_luma <= 5.0);
    }

    #[test]
    fn imagen_gris_medio_tiene_contraste_bajo() {
        let q = compute_quality(&uniforme(200, 200, 128));
        assert!(q.contrast < 1.0);
        assert!((q.mean_luma - 128.0).abs() < 1.0);
    }

    #[test]
    fn tablero_tiene_contraste_muy_alto() {
        let q = compute_quality(&tablero(200, 200));
        assert!(
            q.contrast > 50.0,
            "tablero debería tener contraste alto, fue {}",
            q.contrast
        );
    }

    #[test]
    fn imagen_muy_grande_se_reduce_a_lado_1024() {
        let big = uniforme(2048, 1024, 100);
        let _ = compute_quality(&big);
        // Si llegamos aquí sin pánico y rápido, la reducción funcionó.
        let red = reduced_luma(&big);
        assert!(red.width() <= TARGET_LONGEST_SIDE);
        assert!(red.height() <= TARGET_LONGEST_SIDE);
        assert!(red.width().max(red.height()) == TARGET_LONGEST_SIDE);
    }
}
