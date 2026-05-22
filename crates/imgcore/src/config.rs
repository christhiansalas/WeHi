//! Modelo de datos de los presets de procesamiento.
//!
//! La posición del logo se guarda en coordenadas normalizadas
//! 0.0–1.0 respecto al CENTRO del logo, para que el mismo preset
//! funcione igual en fotos de distintas dimensiones dentro del lote.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;


#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub enum ResizeMode {
    #[default]
    MaxDimension,
    Exact,
    Percentage,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub enum OutputFormat {
    #[default]
    Jpeg,
    Png,
    WebP,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct ResizeConfig {
    pub mode: ResizeMode,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub percentage: Option<f32>,
    pub keep_aspect_ratio: bool,
    pub allow_upscale: bool,
}

/// Posición del CENTRO del logo en coordenadas normalizadas 0.0–1.0
/// respecto a la imagen base.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct LogoPosition {
    pub x: f32,
    pub y: f32,
}

/// Recuadro opcional dibujado detrás del logo.
///
/// El canal alfa de `color_rgba` controla la translucidez del fondo;
/// `padding_pct` es el relleno alrededor del logo, expresado como %
/// del ancho del logo ya reescalado.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct LogoBackground {
    pub color_rgba: [u8; 4],
    pub padding_pct: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct LogoConfig {
    /// Referencia al logo dentro de la biblioteca de la app.
    pub logo_id: String,
    pub position: LogoPosition,
    /// Ancho del logo como % del ancho de la imagen base.
    pub scale_pct: f32,
    /// Opacidad global del logo (0.0–1.0).
    pub opacity: f32,
    pub background: Option<LogoBackground>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct OutputConfig {
    pub format: OutputFormat,
    pub quality: u8,
    #[ts(type = "string")]
    pub folder: PathBuf,
    pub filename_pattern: String,
    pub preserve_structure: bool,
}

/// Marca de agua por TEXTO (firma del fotógrafo, fecha, etc.).
///
/// El tamaño es relativo al ancho de la imagen base
/// (`font_size_pct`), igual que el logo, para que el mismo preset
/// se vea proporcional en fotos de distintas resoluciones. Soporta
/// los tokens `{filename}` y `{date}` en el campo `text`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct TextWatermark {
    /// Contenido. Acepta tokens `{filename}` y `{date}`.
    pub text: String,
    /// Tamaño de fuente como fracción del ancho de la imagen base
    /// (típicamente 0.02–0.08).
    pub font_size_pct: f32,
    pub color_rgba: [u8; 4],
    pub position: LogoPosition,
    pub opacity: f32,
    pub background: Option<LogoBackground>,
}

/// Recorte adicional aplicado al output principal. Cada `NetworkCrop`
/// produce un archivo extra `<nombre>__<id>.<ext>` con la imagen
/// centrada-y-recortada al aspect ratio (width/height) y reescalada a
/// esas dimensiones exactas.
///
/// Útil para entregar una foto en varios formatos de redes sociales
/// en una sola pasada de procesamiento.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct NetworkCrop {
    /// Sufijo que aparece en el nombre del archivo.
    pub id: String,
    /// Etiqueta legible para la UI.
    pub label: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct Preset {
    pub name: String,
    pub resize: ResizeConfig,
    /// **Legacy.** Antes de v0.2 había un solo logo por preset.
    /// Se conserva por compatibilidad de lectura: los presets viejos
    /// guardados como `{ "logo": {...} }` siguen cargando. Al guardar
    /// se vuelca al primer slot de `logos` y este campo queda `None`.
    /// Para presets nuevos usar `logos` directamente.
    #[serde(default)]
    pub logo: Option<LogoConfig>,
    /// Lista de marcas de agua compuestas en orden (firma + sello +
    /// frame, etc.). Si `logo` está set por compatibilidad, se trata
    /// como si fuera el primer elemento de esta lista.
    #[serde(default)]
    pub logos: Vec<LogoConfig>,
    pub output: OutputConfig,
    /// Recortes adicionales (uno por red/formato). Se generan después
    /// del output principal usando la imagen ya redimensionada + con
    /// marca de agua. Vacío = solo se produce el output principal.
    #[serde(default)]
    pub network_crops: Vec<NetworkCrop>,
    /// Marcas de agua por TEXTO. Se componen DESPUÉS de los logos
    /// para que el texto quede por encima de las marcas gráficas.
    #[serde(default)]
    pub text_watermarks: Vec<TextWatermark>,
    /// Ajustes de exposición / contraste / saturación / temperatura
    /// aplicados ANTES de las marcas de agua. Por defecto es
    /// identidad (no-op).
    #[serde(default)]
    pub adjustments: crate::adjust::ImageAdjustments,
}

impl Preset {
    /// Devuelve todas las marcas configuradas en orden de composición
    /// (el legacy `logo` primero, luego `logos`).
    pub fn all_logos(&self) -> impl Iterator<Item = &LogoConfig> {
        self.logo.iter().chain(self.logos.iter())
    }

    /// `true` si el preset no tiene ninguna marca de agua.
    pub fn has_no_logos(&self) -> bool {
        self.logo.is_none() && self.logos.is_empty()
    }
}

impl Default for ResizeConfig {
    fn default() -> Self {
        Self {
            mode: ResizeMode::MaxDimension,
            width: Some(2048),
            height: Some(2048),
            percentage: None,
            keep_aspect_ratio: true,
            allow_upscale: false,
        }
    }
}

impl Default for LogoPosition {
    fn default() -> Self {
        Self { x: 0.9, y: 0.9 }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: OutputFormat::Jpeg,
            quality: 85,
            folder: PathBuf::from("./salida"),
            filename_pattern: "{name}{ext}".to_string(),
            preserve_structure: false,
        }
    }
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "Por defecto".to_string(),
            resize: ResizeConfig::default(),
            logo: None,
            logos: Vec::new(),
            output: OutputConfig::default(),
            network_crops: Vec::new(),
            text_watermarks: Vec::new(),
            adjustments: crate::adjust::ImageAdjustments::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset_con_logo(con_fondo: bool) -> Preset {
        let background = if con_fondo {
            Some(LogoBackground {
                color_rgba: [0, 0, 0, 180],
                padding_pct: 0.05,
            })
        } else {
            None
        };

        Preset {
            name: "Marca esquina".to_string(),
            resize: ResizeConfig {
                mode: ResizeMode::Exact,
                width: Some(1080),
                height: Some(1350),
                percentage: None,
                keep_aspect_ratio: true,
                allow_upscale: false,
            },
            logo: Some(LogoConfig {
                logo_id: "logo-principal".to_string(),
                position: LogoPosition { x: 0.85, y: 0.92 },
                scale_pct: 0.18,
                opacity: 0.9,
                background,
            }),
            output: OutputConfig {
                format: OutputFormat::WebP,
                quality: 82,
                folder: PathBuf::from("/tmp/out"),
                filename_pattern: "{name}_wehi{ext}".to_string(),
                preserve_structure: true,
            },
            logos: Vec::new(),
            network_crops: Vec::new(),
            text_watermarks: Vec::new(),
            adjustments: crate::adjust::ImageAdjustments::default(),
        }
    }

    fn round_trip(preset: &Preset) -> Preset {
        let json = serde_json::to_string(preset).expect("serializar");
        serde_json::from_str(&json).expect("deserializar")
    }

    #[test]
    fn round_trip_preset_sin_logo() {
        let original = Preset::default();
        let recuperado = round_trip(&original);
        assert_eq!(original.name, recuperado.name);
        assert!(recuperado.logo.is_none());
        assert_eq!(recuperado.output.quality, 85);
        assert_eq!(recuperado.output.format, OutputFormat::Jpeg);
        assert!(!recuperado.resize.allow_upscale);
    }

    #[test]
    fn round_trip_preset_con_logo_sin_fondo() {
        let original = preset_con_logo(false);
        let recuperado = round_trip(&original);
        let logo = recuperado.logo.expect("logo presente");
        assert_eq!(logo.logo_id, "logo-principal");
        assert!(logo.background.is_none());
        assert!((logo.position.x - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn round_trip_preset_con_logo_y_fondo() {
        let original = preset_con_logo(true);
        let recuperado = round_trip(&original);
        let logo = recuperado.logo.expect("logo presente");
        let fondo = logo.background.expect("fondo presente");
        assert_eq!(fondo.color_rgba, [0, 0, 0, 180]);
        assert!((fondo.padding_pct - 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn preset_default_no_tiene_logo_y_calidad_es_85() {
        let p = Preset::default();
        assert!(p.logo.is_none());
        assert_eq!(p.output.quality, 85);
        assert_eq!(p.output.format, OutputFormat::Jpeg);
        assert!(!p.resize.allow_upscale);
    }
}
