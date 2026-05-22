//! Puntaje compuesto, ranking y elección de ganadora por grupo.
//!
//! El puntaje compuesto es una combinación lineal de cuatro
//! sub-puntajes objetivos (nitidez, exposición, estética,
//! resolución) más un **bono de caras** que NUNCA penaliza —si no
//! hay caras, vale 0.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::record::{AnalysisRecord, QualityMetrics, SubScores};

/// Pesos editables del puntaje compuesto. Los pesos de la base
/// (sharpness, exposure, aesthetic, resolution) deberían sumar 1.0,
/// pero el código tolera otros valores y siempre recorta el
/// resultado a [0, 1].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub struct ScoringWeights {
    pub w_sharpness: f32,
    pub w_exposure: f32,
    pub w_aesthetic: f32,
    pub w_resolution: f32,
    pub face_bonus_max: f32,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            w_sharpness: 0.30,
            w_exposure: 0.22,
            w_aesthetic: 0.38,
            w_resolution: 0.10,
            face_bonus_max: 0.10,
        }
    }
}

const REFERENCE_PIXELS: u64 = 1920 * 1080;

/// Calcula `sub_scores` y `composite_score` para cada registro y
/// asigna `is_group_winner = true` a la foto con mejor puntaje
/// dentro de cada `duplicate_group`. Pasa por el lote completo dos
/// veces: una para los percentiles, otra para los ganadores.
pub fn score_batch(records: &mut [AnalysisRecord], weights: &ScoringWeights) {
    if records.is_empty() {
        return;
    }

    let mut sharpness_sorted: Vec<f32> = records.iter().map(|r| r.quality.sharpness_raw).collect();
    sharpness_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    for rec in records.iter_mut() {
        let sub = compute_sub_scores(rec, &sharpness_sorted, weights);
        let weighted = weights.w_sharpness * sub.sharpness
            + weights.w_exposure * sub.exposure
            + weights.w_aesthetic * sub.aesthetic
            + weights.w_resolution * sub.resolution;
        let composite = (weighted + sub.face_bonus).clamp(0.0, 1.0);
        rec.sub_scores = sub;
        rec.composite_score = composite;
    }

    assign_group_winners(records);
}

fn compute_sub_scores(
    rec: &AnalysisRecord,
    sharpness_sorted: &[f32],
    w: &ScoringWeights,
) -> SubScores {
    let sharpness = percentile_rank(sharpness_sorted, rec.quality.sharpness_raw);
    let exposure = exposure_score(&rec.quality);
    let resolution = resolution_score(rec.width, rec.height);
    let aesthetic = rec
        .aesthetic
        .map(|s| ((s.mean - 1.0) / 9.0).clamp(0.0, 1.0))
        .unwrap_or(0.5);
    let face_bonus = match rec.faces {
        Some(f) if f.count > 0 => {
            let size_norm = (f.largest_face_frac / 0.2).clamp(0.0, 1.0);
            let centered = f.main_face_centered.clamp(0.0, 1.0);
            (w.face_bonus_max * (0.5 * centered + 0.5 * size_norm)).clamp(0.0, w.face_bonus_max)
        }
        _ => 0.0,
    };
    SubScores {
        sharpness,
        exposure,
        resolution,
        aesthetic,
        face_bonus,
    }
}

/// Rango percentil de `v` dentro de `sorted` (ya ordenado ascendente).
pub fn percentile_rank(sorted: &[f32], v: f32) -> f32 {
    if sorted.is_empty() {
        return 0.5;
    }
    let count_leq = sorted.partition_point(|x| *x <= v);
    let rank = (count_leq as f32 - 0.5) / sorted.len() as f32;
    rank.clamp(0.0, 1.0)
}

fn exposure_score(q: &QualityMetrics) -> f32 {
    // Las altas luces recortadas pesan más (irrecuperables).
    let p_high = ((q.clipped_highlights_pct / 100.0) * 5.0).min(1.0);
    let p_low = ((q.clipped_shadows_pct / 100.0) * 2.5).min(1.0);

    // Banda ideal de luminancia media: 90..160.
    let luma_dev = if q.mean_luma < 90.0 {
        90.0 - q.mean_luma
    } else if q.mean_luma > 160.0 {
        q.mean_luma - 160.0
    } else {
        0.0
    };
    let p_luma = (luma_dev / 80.0).clamp(0.0, 1.0);

    let total = (p_high * 0.5 + p_low * 0.2 + p_luma * 0.3).clamp(0.0, 1.0);
    (1.0 - total).clamp(0.0, 1.0)
}

fn resolution_score(width: u32, height: u32) -> f32 {
    let total = width as u64 * height as u64;
    (total as f32 / REFERENCE_PIXELS as f32).clamp(0.0, 1.0)
}

fn assign_group_winners(records: &mut [AnalysisRecord]) {
    let mut best_per_group: HashMap<u32, (usize, f32)> = HashMap::new();
    for (i, rec) in records.iter().enumerate() {
        if let Some(group) = rec.duplicate_group {
            best_per_group
                .entry(group)
                .and_modify(|e| {
                    if rec.composite_score > e.1 {
                        *e = (i, rec.composite_score);
                    }
                })
                .or_insert((i, rec.composite_score));
        }
    }
    for rec in records.iter_mut() {
        rec.is_group_winner = false;
    }
    for (_, (idx, _)) in best_per_group {
        records[idx].is_group_winner = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{AestheticScore, CullStatus, FaceSummary, QualityMetrics};
    use std::path::PathBuf;
    use std::time::UNIX_EPOCH;

    fn record(width: u32, height: u32, q: QualityMetrics) -> AnalysisRecord {
        AnalysisRecord {
            path: PathBuf::from(format!("/x/{width}x{height}.jpg")),
            file_size: 0,
            modified: UNIX_EPOCH,
            width,
            height,
            capture_time: None,
            content_hash: [0u8; 32],
            perceptual_hash: 0,
            quality: q,
            aesthetic: None,
            faces: None,
            sub_scores: SubScores::default(),
            composite_score: 0.0,
            duplicate_group: None,
            is_group_winner: false,
            status: CullStatus::Undecided,
            person_id: None,
        }
    }

    fn quality(sharp: f32, hi_pct: f32, lo_pct: f32, mean: f32, contrast: f32) -> QualityMetrics {
        QualityMetrics {
            sharpness_raw: sharp,
            clipped_highlights_pct: hi_pct,
            clipped_shadows_pct: lo_pct,
            mean_luma: mean,
            contrast,
        }
    }

    #[test]
    fn exposure_ideal_da_puntaje_alto() {
        let q = quality(100.0, 0.0, 0.0, 130.0, 50.0);
        let s = exposure_score(&q);
        assert!(s > 0.95, "exposición ideal debería ser ~1.0, fue {s}");
    }

    #[test]
    fn exposure_blanco_quemado_da_puntaje_bajo() {
        let q = quality(0.0, 50.0, 0.0, 250.0, 10.0);
        let s = exposure_score(&q);
        assert!(s < 0.4, "blanco quemado debería penalizar, fue {s}");
    }

    #[test]
    fn resolucion_full_hd_es_1_0() {
        assert!((resolution_score(1920, 1080) - 1.0).abs() < 1e-3);
        assert!(resolution_score(800, 600) < 0.5);
        assert!((resolution_score(6000, 4000) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn percentile_rank_basico() {
        let sorted = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let p_min = percentile_rank(&sorted, 1.0);
        let p_max = percentile_rank(&sorted, 5.0);
        let p_mid = percentile_rank(&sorted, 3.0);
        assert!(p_min < p_mid);
        assert!(p_mid < p_max);
    }

    #[test]
    fn composite_score_mas_nitido_y_mejor_expuesto_gana() {
        let mut lote = vec![
            record(1920, 1080, quality(50.0, 0.0, 0.0, 130.0, 50.0)),
            record(1920, 1080, quality(500.0, 0.0, 0.0, 130.0, 60.0)),
        ];
        score_batch(&mut lote, &ScoringWeights::default());
        assert!(lote[1].composite_score > lote[0].composite_score);
    }

    #[test]
    fn ganadora_de_grupo_es_la_de_mayor_composite() {
        let q_ok = quality(200.0, 0.0, 0.0, 130.0, 50.0);
        let q_malo = quality(10.0, 0.0, 0.0, 130.0, 30.0);
        let mut lote = vec![
            {
                let mut r = record(1920, 1080, q_malo);
                r.duplicate_group = Some(0);
                r
            },
            {
                let mut r = record(1920, 1080, q_ok);
                r.duplicate_group = Some(0);
                r
            },
            {
                let mut r = record(1920, 1080, q_ok);
                r.duplicate_group = Some(1);
                r
            },
        ];
        score_batch(&mut lote, &ScoringWeights::default());
        assert!(!lote[0].is_group_winner);
        assert!(lote[1].is_group_winner);
        assert!(lote[2].is_group_winner);
    }

    #[test]
    fn aesthetic_neutro_si_no_hay_modelo() {
        let q = quality(100.0, 0.0, 0.0, 130.0, 50.0);
        let mut lote = vec![record(1920, 1080, q)];
        score_batch(&mut lote, &ScoringWeights::default());
        assert!((lote[0].sub_scores.aesthetic - 0.5).abs() < 1e-5);
    }

    #[test]
    fn face_bonus_nunca_penaliza_y_aumenta_composite() {
        let q = quality(100.0, 0.0, 0.0, 130.0, 50.0);
        let mut sin_cara = record(1920, 1080, q);
        let mut con_cara = record(1920, 1080, q);
        con_cara.faces = Some(FaceSummary {
            count: 1,
            largest_face_frac: 0.2,
            main_face_centered: 1.0,
        });
        let mut lote = vec![sin_cara.clone(), con_cara.clone()];
        score_batch(&mut lote, &ScoringWeights::default());
        assert!(lote[1].composite_score > lote[0].composite_score);
        assert!(lote[1].sub_scores.face_bonus > 0.0);
        assert!(lote[0].sub_scores.face_bonus == 0.0);
        // silence unused warnings
        sin_cara.composite_score = 0.0;
        con_cara.composite_score = 0.0;
    }

    #[test]
    fn aesthetic_alta_aumenta_composite() {
        let q = quality(100.0, 0.0, 0.0, 130.0, 50.0);
        let mut baja = record(1920, 1080, q);
        baja.aesthetic = Some(AestheticScore {
            mean: 2.0,
            std_dev: 1.0,
        });
        let mut alta = record(1920, 1080, q);
        alta.aesthetic = Some(AestheticScore {
            mean: 9.0,
            std_dev: 0.5,
        });
        let mut lote = vec![baja, alta];
        score_batch(&mut lote, &ScoringWeights::default());
        assert!(lote[1].composite_score > lote[0].composite_score);
    }
}
