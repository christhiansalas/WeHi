//! Recomendación por red social.
//!
//! Tabla data-driven de redes y, para cada foto, cálculo del encaje
//! de formato (retención del recorte × sujeto dentro × resolución
//! suficiente). La recomendación final es `encaje × composite_score`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;


#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub enum NetworkOrientation {
    Portrait,
    #[default]
    Landscape,
    Square,
}

/// Especificación de una red social. `aspect_ratio` es ancho/alto.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub struct NetworkSpec {
    pub id: String,
    pub name: String,
    pub aspect_ratio: f32,
    pub orientation: NetworkOrientation,
    pub suggested_width: u32,
    pub suggested_height: u32,
}

/// Lista por defecto de redes; editable desde la app.
pub fn default_networks() -> Vec<NetworkSpec> {
    vec![
        NetworkSpec {
            id: "ig_feed_4_5".into(),
            name: "Instagram feed 4:5".into(),
            aspect_ratio: 4.0 / 5.0,
            orientation: NetworkOrientation::Portrait,
            suggested_width: 1080,
            suggested_height: 1350,
        },
        NetworkSpec {
            id: "ig_feed_square".into(),
            name: "Instagram feed 1:1".into(),
            aspect_ratio: 1.0,
            orientation: NetworkOrientation::Square,
            suggested_width: 1080,
            suggested_height: 1080,
        },
        NetworkSpec {
            id: "stories_reels_tiktok".into(),
            name: "Stories / Reels / TikTok 9:16".into(),
            aspect_ratio: 9.0 / 16.0,
            orientation: NetworkOrientation::Portrait,
            suggested_width: 1080,
            suggested_height: 1920,
        },
        NetworkSpec {
            id: "fb_feed".into(),
            name: "Facebook feed 1.91:1".into(),
            aspect_ratio: 1.91,
            orientation: NetworkOrientation::Landscape,
            suggested_width: 1200,
            suggested_height: 628,
        },
        NetworkSpec {
            id: "yt_thumb".into(),
            name: "Miniatura YouTube 16:9".into(),
            aspect_ratio: 16.0 / 9.0,
            orientation: NetworkOrientation::Landscape,
            suggested_width: 1280,
            suggested_height: 720,
        },
    ]
}

/// Calcula el encaje de una foto a una red. El sujeto se da en
/// coordenadas normalizadas 0..1 (centro de la cara principal o
/// el centro de la imagen si no hay caras).
///
/// Resultado en 0..1: `retención × sujeto_dentro × resolución_suficiente`.
pub fn format_fit(
    width: u32,
    height: u32,
    subject_x_norm: f32,
    subject_y_norm: f32,
    net: &NetworkSpec,
) -> f32 {
    if width == 0 || height == 0 || net.aspect_ratio <= 0.0 {
        return 0.0;
    }
    let w = width as f32;
    let h = height as f32;
    let img_ratio = w / h;
    let net_ratio = net.aspect_ratio;

    let (crop_w, crop_h) = if img_ratio > net_ratio {
        let cw = h * net_ratio;
        (cw, h)
    } else {
        let ch = w / net_ratio;
        (w, ch)
    };

    let retention = ((crop_w * crop_h) / (w * h)).clamp(0.0, 1.0);

    let sx = subject_x_norm.clamp(0.0, 1.0) * w;
    let sy = subject_y_norm.clamp(0.0, 1.0) * h;
    let crop_x = (sx - crop_w / 2.0).clamp(0.0, w - crop_w);
    let crop_y = (sy - crop_h / 2.0).clamp(0.0, h - crop_h);
    let subject_inside = if sx >= crop_x
        && sx <= crop_x + crop_w
        && sy >= crop_y
        && sy <= crop_y + crop_h
    {
        1.0
    } else {
        0.0
    };

    let r_w = crop_w / net.suggested_width as f32;
    let r_h = crop_h / net.suggested_height as f32;
    let resolution_sufficient = r_w.min(r_h).clamp(0.0, 1.0);

    (retention * subject_inside * resolution_sufficient).clamp(0.0, 1.0)
}

/// Recomendación = encaje × composite_score, recortado a 0..1.
pub fn recommendation(fit: f32, composite_score: f32) -> f32 {
    (fit * composite_score).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red(id: &str) -> NetworkSpec {
        default_networks()
            .into_iter()
            .find(|n| n.id == id)
            .expect("red en la tabla por defecto")
    }

    #[test]
    fn foto_16_9_full_hd_encaja_perfecto_en_youtube() {
        let fit = format_fit(1920, 1080, 0.5, 0.5, &red("yt_thumb"));
        assert!(fit > 0.99, "fit = {fit}");
    }

    #[test]
    fn foto_16_9_horizontal_pierde_recorte_en_reels() {
        let fit = format_fit(1920, 1080, 0.5, 0.5, &red("stories_reels_tiktok"));
        // Recorte vertical fuerte → retención baja.
        assert!(fit < 0.5, "fit = {fit}");
    }

    #[test]
    fn baja_resolucion_baja_el_fit() {
        let fit_alto = format_fit(1920, 1080, 0.5, 0.5, &red("yt_thumb"));
        let fit_bajo = format_fit(640, 360, 0.5, 0.5, &red("yt_thumb"));
        assert!(fit_alto > fit_bajo);
    }

    #[test]
    fn recommendation_combina_fit_y_score() {
        let r = recommendation(0.8, 0.9);
        assert!((r - 0.72).abs() < 1e-5);
    }

    #[test]
    fn tabla_por_defecto_incluye_redes_obligatorias() {
        let redes = default_networks();
        for esperado in [
            "ig_feed_4_5",
            "ig_feed_square",
            "stories_reels_tiktok",
            "fb_feed",
            "yt_thumb",
        ] {
            assert!(
                redes.iter().any(|n| n.id == esperado),
                "falta {esperado}"
            );
        }
    }
}
