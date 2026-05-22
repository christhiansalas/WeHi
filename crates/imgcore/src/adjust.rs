//! Ajustes ligeros de imagen aplicados después del resize y antes
//! de las marcas de agua: exposición (brillo), contraste, saturación
//! y temperatura de color.
//!
//! Todos los valores se normalizan a `[-1.0, 1.0]` con `0` = neutro
//! (sin cambios). Esto permite que la UI use sliders consistentes y
//! que la operación sea no-op por defecto.

use image::DynamicImage;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct ImageAdjustments {
    /// Brillo. -1 = oscurece a negro, +1 = aclara a blanco.
    pub brightness: f32,
    /// Contraste. -1 = aplana a gris medio, +1 = duplica el contraste.
    pub contrast: f32,
    /// Saturación. -1 = blanco y negro, +1 = duplica la saturación.
    pub saturation: f32,
    /// Temperatura. -1 = más frío (azul), +1 = más cálido (naranja/rojo).
    pub temperature: f32,
}

impl ImageAdjustments {
    /// `true` si todos los valores son 0 (la transformación es no-op).
    pub fn is_identity(&self) -> bool {
        self.brightness == 0.0
            && self.contrast == 0.0
            && self.saturation == 0.0
            && self.temperature == 0.0
    }
}

/// Aplica los ajustes a la imagen píxel a píxel.
///
/// Si `adj.is_identity()` es `true`, devuelve la imagen sin tocar
/// (sin copiar buffers).
pub fn apply_adjustments(img: DynamicImage, adj: &ImageAdjustments) -> DynamicImage {
    if adj.is_identity() {
        return img;
    }

    let mut rgba = img.to_rgba8();
    // Mapeo de los -1..1 a multiplicadores efectivos:
    // - brightness: shift directo en [-1, 1] sobre [0,1].
    // - contrast: factor multiplicativo alrededor de 0.5. -1 → 0
    //   (plano), 0 → 1 (sin cambio), +1 → 2 (contraste duplicado).
    // - saturation: idem alrededor de la luminancia. -1 → BN, +1 → 2×.
    // - temperature: empuja R y B en sentidos opuestos.
    let b = adj.brightness.clamp(-1.0, 1.0);
    let c = (adj.contrast + 1.0).clamp(0.0, 3.0);
    let s = (adj.saturation + 1.0).clamp(0.0, 3.0);
    let t = adj.temperature.clamp(-1.0, 1.0);

    for px in rgba.pixels_mut() {
        let [r, g, b_chan, a] = px.0;
        let mut rf = r as f32 / 255.0;
        let mut gf = g as f32 / 255.0;
        let mut bf = b_chan as f32 / 255.0;

        // Brillo.
        rf += b;
        gf += b;
        bf += b;

        // Contraste alrededor del gris medio (0.5).
        rf = (rf - 0.5) * c + 0.5;
        gf = (gf - 0.5) * c + 0.5;
        bf = (bf - 0.5) * c + 0.5;

        // Saturación: interpola entre luminancia gris y el color.
        let lum = 0.299 * rf + 0.587 * gf + 0.114 * bf;
        rf = lum + (rf - lum) * s;
        gf = lum + (gf - lum) * s;
        bf = lum + (bf - lum) * s;

        // Temperatura: shift suave de canales (escala 0.1 = ±10%).
        rf += t * 0.1;
        bf -= t * 0.1;

        px.0 = [
            (rf.clamp(0.0, 1.0) * 255.0).round() as u8,
            (gf.clamp(0.0, 1.0) * 255.0).round() as u8,
            (bf.clamp(0.0, 1.0) * 255.0).round() as u8,
            a,
        ];
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Construye el filtro de ffmpeg equivalente. Devuelve `None` si
/// los ajustes son identidad (no se añade nada a la cadena).
pub fn build_ffmpeg_filter(adj: &ImageAdjustments) -> Option<String> {
    if adj.is_identity() {
        return None;
    }
    // `eq` cubre brightness/contrast/saturation.
    // brightness: [-1, 1] directo
    // contrast: 1 + adj (default 1)
    // saturation: 1 + adj (default 1)
    let brightness = adj.brightness.clamp(-1.0, 1.0);
    let contrast = (1.0 + adj.contrast).clamp(0.0, 3.0);
    let saturation = (1.0 + adj.saturation).clamp(0.0, 3.0);
    let mut filter = format!(
        "eq=brightness={brightness:.4}:contrast={contrast:.4}:saturation={saturation:.4}"
    );
    // Temperatura mediante `colorbalance` (rm/bm = midtones).
    if adj.temperature != 0.0 {
        let t = adj.temperature.clamp(-1.0, 1.0) * 0.3; // ±0.3 es suave
        filter.push_str(&format!(",colorbalance=rm={t:.4}:bm={negt:.4}", negt = -t));
    }
    Some(filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn imagen_uniforme(color: [u8; 4]) -> DynamicImage {
        let mut img = RgbaImage::new(10, 10);
        for px in img.pixels_mut() {
            *px = Rgba(color);
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn identity_no_modifica() {
        let original = imagen_uniforme([100, 150, 200, 255]);
        let bytes_antes = original.to_rgba8().into_raw();
        let out = apply_adjustments(original, &ImageAdjustments::default());
        assert_eq!(out.to_rgba8().into_raw(), bytes_antes);
    }

    #[test]
    fn brightness_positivo_aclara() {
        let img = imagen_uniforme([100, 100, 100, 255]);
        let adj = ImageAdjustments {
            brightness: 0.3,
            ..Default::default()
        };
        let out = apply_adjustments(img, &adj).to_rgba8();
        let px = out.get_pixel(0, 0).0;
        assert!(px[0] > 150, "esperaba aclarar: {}", px[0]);
    }

    #[test]
    fn saturation_negativa_max_es_blanco_y_negro() {
        let img = imagen_uniforme([255, 0, 0, 255]);
        let adj = ImageAdjustments {
            saturation: -1.0,
            ..Default::default()
        };
        let out = apply_adjustments(img, &adj).to_rgba8();
        let px = out.get_pixel(0, 0).0;
        // R, G, B deberían quedar muy cercanos (gris).
        let diff = (px[0] as i32 - px[1] as i32).abs() + (px[1] as i32 - px[2] as i32).abs();
        assert!(diff < 5, "esperaba gris, fue {:?}", px);
    }

    #[test]
    fn temperatura_positiva_aumenta_rojo() {
        let img = imagen_uniforme([128, 128, 128, 255]);
        let adj_calida = ImageAdjustments {
            temperature: 0.8,
            ..Default::default()
        };
        let out = apply_adjustments(img, &adj_calida).to_rgba8();
        let px = out.get_pixel(0, 0).0;
        assert!(px[0] > 128, "R esperado > 128, fue {}", px[0]);
        assert!(px[2] < 128, "B esperado < 128, fue {}", px[2]);
    }

    #[test]
    fn build_ffmpeg_filter_skip_si_identity() {
        assert!(build_ffmpeg_filter(&ImageAdjustments::default()).is_none());
    }

    #[test]
    fn build_ffmpeg_filter_incluye_eq_y_colorbalance() {
        let adj = ImageAdjustments {
            brightness: 0.1,
            contrast: 0.2,
            saturation: 0.3,
            temperature: 0.5,
        };
        let f = build_ffmpeg_filter(&adj).unwrap();
        assert!(f.contains("eq=brightness"));
        assert!(f.contains("colorbalance="));
    }
}
