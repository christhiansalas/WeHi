//! Agrupación de fotos casi-duplicadas.
//!
//! Construye un grafo donde dos fotos están conectadas si su
//! distancia de Hamming (sobre `perceptual_hash`) es ≤ `hamming_threshold`
//! Y, si AMBAS tienen `capture_time`, su separación temporal es
//! ≤ `time_window_seconds`. Si una de las dos no tiene `capture_time`,
//! solo se aplica el filtro de Hamming.
//!
//! Los componentes conexos con más de un miembro son grupos de
//! duplicados; a cada miembro se le asigna el `duplicate_group`.
//!
//! La elección de la ganadora del grupo (`is_group_winner`) se hace
//! en el módulo `scoring` (prompt 09), porque depende del puntaje
//! compuesto del lote.

use std::collections::HashMap;
use std::time::Duration;

use crate::hashing::hamming_distance;
use crate::record::AnalysisRecord;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../src/bindings/")]
pub struct DedupConfig {
    /// Distancia de Hamming máxima entre `perceptual_hash` para
    /// considerar dos fotos como repetidas.
    pub hamming_threshold: u32,
    /// Separación temporal máxima en segundos. Solo se aplica si
    /// ambas fotos tienen `capture_time`.
    #[ts(type = "number")]
    pub time_window_seconds: u64,
}

impl Default for DedupConfig {
    fn default() -> Self {
        Self {
            hamming_threshold: 10,
            time_window_seconds: 30,
        }
    }
}

/// Asigna `duplicate_group` a los registros que forman parte de un
/// grupo de duplicados. Los registros aislados quedan con
/// `duplicate_group = None`. Reinicia `is_group_winner` a `false`
/// (lo decidirá el scoring).
#[allow(clippy::needless_range_loop)]
pub fn assign_duplicate_groups(records: &mut [AnalysisRecord], cfg: &DedupConfig) {
    let n = records.len();
    let mut uf = UnionFind::new(n);

    for i in 0..n {
        for j in (i + 1)..n {
            if son_duplicados(&records[i], &records[j], cfg) {
                uf.union(i, j);
            }
        }
    }

    let roots: Vec<usize> = (0..n).map(|i| uf.find(i)).collect();

    let mut counts: HashMap<usize, u32> = HashMap::new();
    for &root in &roots {
        *counts.entry(root).or_insert(0) += 1;
    }

    let mut component_to_group: HashMap<usize, u32> = HashMap::new();
    let mut next_group: u32 = 0;
    for (record, &root) in records.iter_mut().zip(roots.iter()) {
        let group = if counts[&root] > 1 {
            let id = *component_to_group.entry(root).or_insert_with(|| {
                let id = next_group;
                next_group += 1;
                id
            });
            Some(id)
        } else {
            None
        };
        record.duplicate_group = group;
        record.is_group_winner = false;
    }
}

fn son_duplicados(a: &AnalysisRecord, b: &AnalysisRecord, cfg: &DedupConfig) -> bool {
    if hamming_distance(a.perceptual_hash, b.perceptual_hash) > cfg.hamming_threshold {
        return false;
    }
    match (a.capture_time, b.capture_time) {
        (Some(ta), Some(tb)) => {
            let diff = if ta >= tb {
                ta.duration_since(tb).unwrap_or(Duration::ZERO)
            } else {
                tb.duration_since(ta).unwrap_or(Duration::ZERO)
            };
            diff.as_secs() <= cfg.time_window_seconds
        }
        _ => true,
    }
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let r = self.find(self.parent[x]);
            self.parent[x] = r;
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => self.parent[ra] = rb,
            std::cmp::Ordering::Greater => self.parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{AnalysisRecord, CullStatus, QualityMetrics, SubScores};
    use std::path::PathBuf;
    use std::time::{Duration, UNIX_EPOCH};

    fn r(ph: u64, capture_secs: Option<u64>) -> AnalysisRecord {
        AnalysisRecord {
            path: PathBuf::from(format!("/x/{ph:x}.jpg")),
            file_size: 0,
            modified: UNIX_EPOCH,
            width: 100,
            height: 100,
            capture_time: capture_secs.map(|s| UNIX_EPOCH + Duration::from_secs(s)),
            content_hash: [0u8; 32],
            perceptual_hash: ph,
            quality: QualityMetrics::default(),
            aesthetic: None,
            faces: None,
            sub_scores: SubScores::default(),
            composite_score: 0.0,
            duplicate_group: None,
            is_group_winner: false,
            status: CullStatus::Undecided,
        }
    }

    #[test]
    fn dos_hashes_identicos_quedan_en_el_mismo_grupo() {
        let mut lote = vec![r(0xAA, None), r(0xAA, None), r(0xFFFF_FFFF_FFFF_FFFF, None)];
        assign_duplicate_groups(&mut lote, &DedupConfig::default());
        assert!(lote[0].duplicate_group.is_some());
        assert_eq!(lote[0].duplicate_group, lote[1].duplicate_group);
        assert!(lote[2].duplicate_group.is_none());
    }

    #[test]
    fn diferencias_dentro_del_umbral_se_agrupan() {
        // 5 bits de diferencia.
        let a = 0u64;
        let b = 0b11111u64;
        let cfg = DedupConfig {
            hamming_threshold: 10,
            time_window_seconds: 30,
        };
        let mut lote = vec![r(a, None), r(b, None)];
        assign_duplicate_groups(&mut lote, &cfg);
        assert!(lote[0].duplicate_group.is_some());
        assert_eq!(lote[0].duplicate_group, lote[1].duplicate_group);
    }

    #[test]
    fn diferencias_fuera_del_umbral_no_se_agrupan() {
        let cfg = DedupConfig {
            hamming_threshold: 3,
            time_window_seconds: 30,
        };
        let mut lote = vec![r(0u64, None), r(0xFFu64, None)];
        assign_duplicate_groups(&mut lote, &cfg);
        assert!(lote[0].duplicate_group.is_none());
        assert!(lote[1].duplicate_group.is_none());
    }

    #[test]
    fn ventana_temporal_separa_aunque_hash_coincida() {
        let cfg = DedupConfig {
            hamming_threshold: 10,
            time_window_seconds: 30,
        };
        let mut lote = vec![
            r(0u64, Some(1_700_000_000)),
            r(0u64, Some(1_700_001_000)), // 1000 s después
        ];
        assign_duplicate_groups(&mut lote, &cfg);
        assert!(lote[0].duplicate_group.is_none());
        assert!(lote[1].duplicate_group.is_none());
    }

    #[test]
    fn sin_capture_time_solo_se_usa_hamming() {
        let cfg = DedupConfig {
            hamming_threshold: 10,
            time_window_seconds: 30,
        };
        let mut lote = vec![
            r(0u64, None),
            r(0u64, Some(1_700_000_000)), // uno tiene tiempo, el otro no
        ];
        assign_duplicate_groups(&mut lote, &cfg);
        assert_eq!(lote[0].duplicate_group, lote[1].duplicate_group);
        assert!(lote[0].duplicate_group.is_some());
    }

    #[test]
    fn varios_grupos_reciben_ids_distintos() {
        let mut lote = vec![
            r(0u64, None),
            r(0u64, None),
            r(0xFFFFu64, None),
            r(0xFFFFu64, None),
        ];
        assign_duplicate_groups(&mut lote, &DedupConfig::default());
        assert_eq!(lote[0].duplicate_group, lote[1].duplicate_group);
        assert_eq!(lote[2].duplicate_group, lote[3].duplicate_group);
        assert_ne!(lote[0].duplicate_group, lote[2].duplicate_group);
    }
}
