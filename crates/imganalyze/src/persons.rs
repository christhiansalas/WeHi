//! Clustering de embeddings ArcFace para asignar `person_id` a cada
//! foto del lote.
//!
//! Algoritmo: single-link greedy con similitud coseno. Cada embedding
//! se compara contra los centroides existentes; si supera el umbral
//! con alguno se incorpora a ese cluster (centroide actualizado como
//! media móvil L2-normalizada), de lo contrario forma un cluster nuevo.
//!
//! Complejidad O(n × k) donde k = número de personas únicas. Para un
//! lote típico de boda (1000 fotos, 30 invitados) son 30 000 comparaciones.

/// Similitud coseno entre dos vectores. Asume ambos L2-normalizados
/// (resultado real está en `[-1, 1]`; para ArcFace siempre ≥ 0 en la
/// práctica porque los embeddings viven en el hemisferio positivo).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn l2_normalize(v: &mut [f32]) {
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 1e-12 {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}

/// Para un lote ya scored, devuelve `(person_id, path_index)` con el
/// índice de la mejor foto por persona según `composite_score`. Ignora
/// fotos sin `person_id` (no había cara o no había modelo ArcFace).
pub fn best_photo_per_person(
    records: &[crate::record::AnalysisRecord],
) -> std::collections::HashMap<u32, usize> {
    use std::collections::HashMap;
    let mut best: HashMap<u32, (usize, f32)> = HashMap::new();
    for (i, r) in records.iter().enumerate() {
        let Some(pid) = r.person_id else { continue };
        match best.get(&pid) {
            Some((_, s)) if *s >= r.composite_score => {}
            _ => {
                best.insert(pid, (i, r.composite_score));
            }
        }
    }
    best.into_iter().map(|(k, (i, _))| (k, i)).collect()
}

/// Asigna `person_id` a cada embedding. `None` entra → `None` sale
/// (la foto no tenía cara detectada o no había ArcFace cargado).
///
/// `threshold` es la similitud coseno mínima para considerar misma
/// persona. Valor típico para ArcFace: 0.5 (Insightface usa 0.55–0.65
/// para verificación 1:1, pero clustering tolera valores algo más bajos).
pub fn cluster_persons(embeddings: &[Option<Vec<f32>>], threshold: f32) -> Vec<Option<u32>> {
    let mut centroids: Vec<Vec<f32>> = Vec::new();
    let mut counts: Vec<u32> = Vec::new();
    let mut assignments = Vec::with_capacity(embeddings.len());

    for emb in embeddings {
        let Some(v) = emb else {
            assignments.push(None);
            continue;
        };
        if v.is_empty() {
            assignments.push(None);
            continue;
        }

        let mut best_idx: Option<usize> = None;
        let mut best_sim = threshold;
        for (i, c) in centroids.iter().enumerate() {
            let s = cosine_similarity(v, c);
            if s > best_sim {
                best_sim = s;
                best_idx = Some(i);
            }
        }

        let id = match best_idx {
            Some(i) => {
                // Actualiza el centroide como media móvil renormalizada.
                let n = counts[i] as f32;
                for k in 0..centroids[i].len() {
                    centroids[i][k] = (centroids[i][k] * n + v[k]) / (n + 1.0);
                }
                l2_normalize(&mut centroids[i]);
                counts[i] += 1;
                i as u32
            }
            None => {
                let id = centroids.len() as u32;
                let mut new_centroid = v.clone();
                l2_normalize(&mut new_centroid);
                centroids.push(new_centroid);
                counts.push(1);
                id
            }
        };
        assignments.push(Some(id));
    }
    assignments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_norm(v: &[f32]) -> Vec<f32> {
        let mut o = v.to_vec();
        l2_normalize(&mut o);
        o
    }

    #[test]
    fn cosine_de_identicos_es_uno() {
        let a = vec_norm(&[1.0, 1.0, 1.0]);
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_de_ortogonales_es_cero() {
        let a = vec_norm(&[1.0, 0.0, 0.0]);
        let b = vec_norm(&[0.0, 1.0, 0.0]);
        assert!(cosine_similarity(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn cluster_dos_personas_distintas_da_ids_diferentes() {
        // Persona A: vectores con primera coordenada dominante.
        let a1 = Some(vec_norm(&[1.0, 0.0, 0.0]));
        let a2 = Some(vec_norm(&[0.95, 0.1, 0.0]));
        // Persona B: tercera coordenada dominante.
        let b1 = Some(vec_norm(&[0.0, 0.0, 1.0]));
        let b2 = Some(vec_norm(&[0.0, 0.1, 0.95]));
        let assigns = cluster_persons(&[a1, b1, a2, b2], 0.5);
        // a1 y a2 deben coincidir; b1 y b2 también; A != B.
        assert_eq!(assigns[0], assigns[2]);
        assert_eq!(assigns[1], assigns[3]);
        assert_ne!(assigns[0], assigns[1]);
    }

    #[test]
    fn cluster_none_se_propaga() {
        let a = Some(vec_norm(&[1.0, 0.0]));
        let assigns = cluster_persons(&[a, None, None], 0.5);
        assert!(assigns[0].is_some());
        assert_eq!(assigns[1], None);
        assert_eq!(assigns[2], None);
    }

    #[test]
    fn cluster_vacio_no_panickea() {
        let assigns = cluster_persons(&[], 0.5);
        assert!(assigns.is_empty());
    }

    #[test]
    fn cluster_un_solo_embedding_es_persona_cero() {
        let a = Some(vec_norm(&[1.0, 0.0, 0.0]));
        let assigns = cluster_persons(&[a], 0.5);
        assert_eq!(assigns, vec![Some(0)]);
    }

    #[test]
    fn cluster_centroide_se_actualiza_y_admite_drift() {
        // Pequeño drift secuencial dentro del mismo cluster.
        let v1 = Some(vec_norm(&[1.0, 0.0, 0.0]));
        let v2 = Some(vec_norm(&[0.9, 0.3, 0.0]));
        let v3 = Some(vec_norm(&[0.8, 0.5, 0.0]));
        let v4 = Some(vec_norm(&[0.7, 0.6, 0.1]));
        let assigns = cluster_persons(&[v1, v2, v3, v4], 0.5);
        // Los 4 deberían acabar en el mismo cluster gracias al drift del centroide.
        assert_eq!(assigns[0], assigns[1]);
        assert_eq!(assigns[1], assigns[2]);
        assert_eq!(assigns[2], assigns[3]);
    }
}
