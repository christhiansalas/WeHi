//! Modelo de datos del análisis de un lote.
//!
//! Todos los tipos se serializan a JSON para ser cacheados (prompt
//! 11) y para cruzar el puente Tauri hacia el frontend.

use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Decisión del usuario sobre conservar o descartar una foto.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub enum CullStatus {
    Keep,
    Reject,
    #[default]
    Undecided,
}

/// Métricas objetivas de calidad calculadas sobre la luminancia.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub struct QualityMetrics {
    pub sharpness_raw: f32,
    pub clipped_highlights_pct: f32,
    pub clipped_shadows_pct: f32,
    pub mean_luma: f32,
    pub contrast: f32,
}

/// Resultado del modelo de estética (NIMA-style):
/// `mean` en 1..10 y `std_dev` como dispersión de la distribución.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub struct AestheticScore {
    pub mean: f32,
    pub std_dev: f32,
}

/// Resumen de la detección de caras.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub struct FaceSummary {
    pub count: u32,
    pub largest_face_frac: f32,
    pub main_face_centered: f32,
}

/// Sub-puntajes normalizados a 0..1 que componen el puntaje final.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub struct SubScores {
    pub sharpness: f32,
    pub exposure: f32,
    pub resolution: f32,
    pub aesthetic: f32,
    pub face_bonus: f32,
}

/// Representación TS de `SystemTime`. Se exporta como objeto con los
/// dos campos que serde genera al serializar `SystemTime` a JSON.
type TsSystemTime = ();

/// Registro completo del análisis de una foto del lote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub struct AnalysisRecord {
    #[ts(type = "string")]
    pub path: PathBuf,
    #[ts(type = "number")]
    pub file_size: u64,
    #[ts(type = "{ secs_since_epoch: number; nanos_since_epoch: number }")]
    pub modified: SystemTime,
    pub width: u32,
    pub height: u32,
    #[ts(type = "{ secs_since_epoch: number; nanos_since_epoch: number } | null")]
    pub capture_time: Option<SystemTime>,
    #[ts(type = "number[]")]
    pub content_hash: [u8; 32],
    #[ts(type = "number")]
    pub perceptual_hash: u64,
    pub quality: QualityMetrics,
    pub aesthetic: Option<AestheticScore>,
    pub faces: Option<FaceSummary>,
    pub sub_scores: SubScores,
    pub composite_score: f32,
    pub duplicate_group: Option<u32>,
    pub is_group_winner: bool,
    pub status: CullStatus,
    /// ID de persona asignado por el clustering ArcFace en la cara
    /// principal. `None` si no hay cara o no había modelo ArcFace.
    /// Los IDs son locales al lote: cambiar la composición puede
    /// renumerarlos.
    #[serde(default)]
    pub person_id: Option<u32>,
}

// Placeholder no-op para acallar al linter; `TsSystemTime` solo
// documenta cómo se representa SystemTime en los bindings TS.
#[allow(dead_code)]
fn _ts_systemtime_placeholder(_: TsSystemTime) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn registro_completo() -> AnalysisRecord {
        AnalysisRecord {
            path: PathBuf::from("/lote/IMG_0001.CR3"),
            file_size: 12_345_678,
            modified: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            width: 6000,
            height: 4000,
            capture_time: Some(UNIX_EPOCH + Duration::from_secs(1_699_999_900)),
            content_hash: [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
                0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C,
                0x1D, 0x1E, 0x1F, 0x20,
            ],
            perceptual_hash: 0xDEAD_BEEF_CAFE_BABE,
            quality: QualityMetrics {
                sharpness_raw: 123.5,
                clipped_highlights_pct: 0.4,
                clipped_shadows_pct: 0.1,
                mean_luma: 130.7,
                contrast: 45.2,
            },
            aesthetic: Some(AestheticScore {
                mean: 6.4,
                std_dev: 1.3,
            }),
            faces: Some(FaceSummary {
                count: 2,
                largest_face_frac: 0.18,
                main_face_centered: 0.72,
            }),
            sub_scores: SubScores {
                sharpness: 0.81,
                exposure: 0.72,
                resolution: 1.0,
                aesthetic: 0.60,
                face_bonus: 0.07,
            },
            composite_score: 0.78,
            duplicate_group: Some(3),
            is_group_winner: true,
            status: CullStatus::Keep,
            person_id: Some(0),
        }
    }

    #[test]
    fn round_trip_json_de_un_analysis_record_completo() {
        let original = registro_completo();
        let json = serde_json::to_string(&original).expect("serializa");
        let recuperado: AnalysisRecord = serde_json::from_str(&json).expect("deserializa");
        assert_eq!(original, recuperado);
    }

    #[test]
    fn round_trip_con_campos_opcionales_en_none() {
        let mut original = registro_completo();
        original.aesthetic = None;
        original.faces = None;
        original.capture_time = None;
        original.duplicate_group = None;
        original.is_group_winner = false;
        original.person_id = None;

        let json = serde_json::to_string(&original).expect("serializa");
        let recuperado: AnalysisRecord = serde_json::from_str(&json).expect("deserializa");
        assert_eq!(original, recuperado);
    }

    #[test]
    fn cull_status_default_es_undecided() {
        assert_eq!(CullStatus::default(), CullStatus::Undecided);
    }
}
