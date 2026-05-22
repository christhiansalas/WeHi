//! Orquestación del análisis completo: por foto y por lote.
//!
//! - [`analyze_photo`] hace todo lo que no necesita ver al resto del
//!   lote: decodifica, hashing, métricas de calidad y, si se pasa un
//!   `AiEngine`, estética y caras.
//! - [`analyze_batch`] paraleliza con `rayon`, luego ejecuta `dedup`
//!   y `scoring` (que SÍ necesitan ver el lote completo para los
//!   percentiles y los grupos).
//!
//! Un archivo con error no detiene el lote: se registra en
//! [`BatchError`] y el resto continúa.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rayon::prelude::*;

use imgcore::ImgError;

use crate::ai::{crop_face_with_margin, AiEngine, FaceBox};
use crate::dedup::{assign_duplicate_groups, DedupConfig};
use crate::hashing::{content_hash, perceptual_hash};
use crate::persons::cluster_persons;
use crate::quality::compute_quality;
use crate::record::{AnalysisRecord, CullStatus, SubScores};
use crate::scoring::{score_batch, ScoringWeights};

/// Similitud coseno mínima para considerar misma persona. Calibrado
/// para los embeddings 512D de ArcFace/InsightFace.
const ARCFACE_CLUSTER_THRESHOLD: f32 = 0.5;

/// Error por archivo dentro de un lote.
#[derive(Debug, Clone)]
pub struct BatchError {
    pub path: PathBuf,
    pub error: String,
}

/// Analiza una foto y devuelve un `AnalysisRecord` SIN puntaje
/// compuesto (eso lo hace `score_batch` cuando se conoce el lote).
pub fn analyze_photo(path: &Path, ai: Option<&AiEngine>) -> Result<AnalysisRecord, ImgError> {
    let (record, _) = analyze_photo_with_embedding(path, ai)?;
    Ok(record)
}

/// Igual que [`analyze_photo`] pero también devuelve el embedding
/// ArcFace de la cara principal (si los modelos están cargados y se
/// detectó al menos una cara). El orquestador del lote usa esta
/// función para luego clusterizar las personas.
pub fn analyze_photo_with_embedding(
    path: &Path,
    ai: Option<&AiEngine>,
) -> Result<(AnalysisRecord, Option<Vec<f32>>), ImgError> {
    let content_hash_val = content_hash(path)?;
    let metadata = std::fs::metadata(path).map_err(|e| ImgError::io(path, e))?;
    let file_size = metadata.len();
    let modified = metadata.modified().unwrap_or(UNIX_EPOCH);

    let img = imgcore::decode(path)?;
    let width = img.width();
    let height = img.height();

    let perceptual_hash_val = perceptual_hash(&img);
    let quality = compute_quality(&img);
    let capture_time = read_capture_time(path);

    let aesthetic = ai.and_then(|e| e.aesthetic(&img)).and_then(|r| r.ok());

    // Detección de caras + cajas para alimentar ArcFace.
    let (faces, main_box) = match ai.and_then(|e| e.faces_with_boxes(&img)) {
        Some(Ok((summary, boxes))) => {
            let main = boxes
                .iter()
                .max_by(|a, b| {
                    a.area()
                        .partial_cmp(&b.area())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied();
            (Some(summary), main)
        }
        _ => (None, None),
    };

    let embedding = compute_face_embedding(ai, &img, main_box.as_ref());

    Ok((
        AnalysisRecord {
            path: path.to_path_buf(),
            file_size,
            modified,
            width,
            height,
            capture_time,
            content_hash: content_hash_val,
            perceptual_hash: perceptual_hash_val,
            quality,
            aesthetic,
            faces,
            sub_scores: SubScores::default(),
            composite_score: 0.0,
            duplicate_group: None,
            is_group_winner: false,
            status: CullStatus::Undecided,
            person_id: None,
        },
        embedding,
    ))
}

fn compute_face_embedding(
    ai: Option<&AiEngine>,
    img: &image::DynamicImage,
    main_box: Option<&FaceBox>,
) -> Option<Vec<f32>> {
    let ai = ai?;
    let face_box = main_box?;
    if !ai.has_arcface() {
        return None;
    }
    let crop = crop_face_with_margin(img, face_box);
    match ai.arcface_embed(&crop) {
        Some(Ok(emb)) => Some(emb),
        _ => None,
    }
}

/// Analiza un lote en paralelo. Devuelve los registros calculados y
/// la lista de errores por archivo. Un archivo con error NO detiene
/// el lote.
pub fn analyze_batch(
    paths: &[PathBuf],
    ai: Option<&AiEngine>,
    weights: &ScoringWeights,
) -> (Vec<AnalysisRecord>, Vec<BatchError>) {
    type PhotoResult = Result<(AnalysisRecord, Option<Vec<f32>>), ImgError>;
    let collected: Vec<(PathBuf, PhotoResult)> = paths
        .par_iter()
        .map(|p| (p.clone(), analyze_photo_with_embedding(p, ai)))
        .collect();

    let mut records = Vec::with_capacity(collected.len());
    let mut embeddings: Vec<Option<Vec<f32>>> = Vec::with_capacity(collected.len());
    let mut errors = Vec::new();
    for (path, result) in collected {
        match result {
            Ok((rec, emb)) => {
                records.push(rec);
                embeddings.push(emb);
            }
            Err(e) => errors.push(BatchError {
                path,
                error: e.to_string(),
            }),
        }
    }

    // Clustering de personas: solo tiene efecto si ArcFace estaba
    // cargado y al menos una foto produjo embedding.
    if embeddings.iter().any(|e| e.is_some()) {
        let assignments = cluster_persons(&embeddings, ARCFACE_CLUSTER_THRESHOLD);
        for (rec, pid) in records.iter_mut().zip(assignments) {
            rec.person_id = pid;
        }
    }

    assign_duplicate_groups(&mut records, &DedupConfig::default());
    score_batch(&mut records, weights);

    (records, errors)
}

fn read_capture_time(path: &Path) -> Option<SystemTime> {
    let bytes = if imgcore::raw::is_raw(path) {
        imgcore::raw::extract_raw_preview(path).ok()?
    } else {
        std::fs::read(path).ok()?
    };
    parse_exif_capture_time(&bytes)
}

fn parse_exif_capture_time(bytes: &[u8]) -> Option<SystemTime> {
    let reader = exif::Reader::new();
    let mut cursor = std::io::Cursor::new(bytes);
    let exif = reader.read_from_container(&mut cursor).ok()?;

    let field = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))?;

    let raw_text = match &field.value {
        exif::Value::Ascii(vec) => {
            let first = vec.first()?;
            std::str::from_utf8(first).ok()?
        }
        _ => return None,
    };
    let trimmed = raw_text.trim_end_matches('\0').trim();

    let naive = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y:%m:%d %H:%M:%S").ok()?;
    let secs = naive.and_utc().timestamp();
    if secs >= 0 {
        Some(UNIX_EPOCH + Duration::from_secs(secs as u64))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::path::PathBuf;

    fn dir_temp(nombre: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wehi_analyze_{}_{}", std::process::id(), nombre));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn guardar_png(path: &Path, w: u32, h: u32, color: [u8; 4]) {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba(color));
            }
        }
        image::DynamicImage::ImageRgba8(img).save(path).unwrap();
    }

    fn guardar_gradiente_decreciente(path: &Path, w: u32, h: u32) {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let v = (255 - (x * 255) / w.max(1)) as u8;
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        image::DynamicImage::ImageRgba8(img).save(path).unwrap();
    }

    #[test]
    fn analyze_photo_calcula_los_campos_basicos() {
        let dir = dir_temp("photo");
        let path = dir.join("a.png");
        guardar_png(&path, 200, 100, [120, 130, 140, 255]);

        let rec = analyze_photo(&path, None).unwrap();
        assert_eq!(rec.path, path);
        assert_eq!(rec.width, 200);
        assert_eq!(rec.height, 100);
        assert!(rec.file_size > 0);
        assert!(rec.aesthetic.is_none());
        assert!(rec.faces.is_none());
        assert!(rec.composite_score == 0.0); // aún no hay scoring de lote
        assert_eq!(rec.status, CullStatus::Undecided);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn analyze_batch_paraleliza_y_aplica_dedup_y_scoring() {
        let dir = dir_temp("batch");
        let p1 = dir.join("a.png");
        let p2 = dir.join("b.png");
        let p3 = dir.join("c.png");
        // a y b idénticas → mismo hash → mismo grupo de duplicados
        guardar_png(&p1, 800, 600, [10, 20, 30, 255]);
        guardar_png(&p2, 800, 600, [10, 20, 30, 255]);
        // c con dHash claramente distinto (gradiente decreciente).
        guardar_gradiente_decreciente(&p3, 400, 300);

        let (mut records, errors) = analyze_batch(
            &[p1.clone(), p2.clone(), p3.clone()],
            None,
            &ScoringWeights::default(),
        );
        assert_eq!(records.len(), 3);
        assert!(errors.is_empty());
        records.sort_by(|a, b| a.path.cmp(&b.path));
        // a y b deberían estar en un grupo, c sola
        assert!(records[0].duplicate_group == records[1].duplicate_group);
        assert!(records[0].duplicate_group.is_some());
        assert!(records[2].duplicate_group.is_none());
        // Todos deben tener composite_score asignado
        for r in &records {
            assert!(r.composite_score >= 0.0 && r.composite_score <= 1.0);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_archivo_invalido_no_detiene_el_lote() {
        let dir = dir_temp("err");
        let p1 = dir.join("ok.png");
        let p2 = dir.join("malo.png");
        guardar_png(&p1, 100, 100, [0, 0, 0, 255]);
        std::fs::write(&p2, b"esto no es un png valido").unwrap();

        let (records, errors) =
            analyze_batch(&[p1.clone(), p2.clone()], None, &ScoringWeights::default());
        assert_eq!(records.len(), 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, p2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
