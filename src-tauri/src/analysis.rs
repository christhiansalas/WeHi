//! Orquestación del análisis (curación) desde la capa Tauri:
//! comandos para iniciar/cancelar, consultar resultados, marcar
//! descartes y mover rechazados.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

use imganalyze::{
    analyze_photo_with_embedding, assign_duplicate_groups, cluster_persons, default_networks,
    format_fit, recommendation, score_batch, AiEngine, AiEngineConfig, AnalysisRecord, CullStatus,
    DedupConfig, NetworkSpec, ScoringWeights,
};

/// Umbral de similitud coseno para considerar misma persona en el
/// clustering del lote. Coincide con el default del crate.
const ARCFACE_CLUSTER_THRESHOLD: f32 = 0.5;

use crate::cache::{
    compute_model_version, default_cache_path, AnalysisCache, CachedEntry, SCHEMA_VERSION,
};
use crate::metrics::MetricsState;
use crate::state::AnalysisState;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct AnalysisProgressEvent {
    pub analizadas: usize,
    pub total: usize,
    pub archivo_actual: String,
    pub etapa: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct AnalysisFileError {
    #[ts(type = "string")]
    pub ruta: PathBuf,
    pub mensaje: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct AnalysisDoneEvent {
    pub total: usize,
    pub analizadas: usize,
    pub desde_cache: usize,
    pub cancelado: bool,
    pub errores: Vec<AnalysisFileError>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct Recommendation {
    #[ts(type = "string")]
    pub path: PathBuf,
    pub composite_score: f32,
    pub fit: f32,
    pub recommendation: f32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct MoveRejectsReport {
    /// Cuántos registros tenían `status == Reject` cuando se invocó
    /// el comando. Si es 0, no había nada que mover.
    pub considerados: usize,
    #[ts(type = "string[]")]
    pub movidos: Vec<PathBuf>,
    pub errores: Vec<AnalysisFileError>,
}

/// Lista de directorios donde buscar los modelos ONNX.
///
/// Prioridad:
/// 1. `app_data/models` — permite al usuario sustituir modelos sin
///    reinstalar la app.
/// 2. `resource_dir/models` — modelos empaquetados con la app (lo que
///    declara `bundle.resources` en `tauri.conf.json`).
fn model_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(data_dir) = app.path().app_data_dir() {
        dirs.push(data_dir.join("models"));
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        dirs.push(resource_dir.join("models"));
    }
    dirs
}

fn find_model(app: &AppHandle, file_name: &str) -> Option<PathBuf> {
    for dir in model_dirs(app) {
        let candidate = dir.join(file_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn ai_engine_paths(app: &AppHandle) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    (
        find_model(app, "aesthetic.onnx"),
        find_model(app, "faces.onnx"),
        find_model(app, "arcface.onnx"),
    )
}

fn build_ai_engine(app: &AppHandle, use_ai: bool) -> (Option<AiEngine>, Vec<PathBuf>) {
    if !use_ai {
        return (None, Vec::new());
    }
    let (aest, faces, arcface) = ai_engine_paths(app);
    let mut cfg = AiEngineConfig::new();
    cfg.aesthetic_model = aest.clone();
    cfg.faces_model = faces.clone();
    cfg.arcface_model = arcface.clone();
    let engine = AiEngine::new(&cfg).ok();
    let mut paths = Vec::new();
    if let Some(p) = aest {
        paths.push(p);
    }
    if let Some(p) = faces {
        paths.push(p);
    }
    if let Some(p) = arcface {
        paths.push(p);
    }
    (engine, paths)
}

fn recompute_ai_fields(
    file: &Path,
    mut record: AnalysisRecord,
    ai: Option<&AiEngine>,
) -> Result<(AnalysisRecord, Option<Vec<f32>>), imgcore::ImgError> {
    let Some(engine) = ai else {
        record.aesthetic = None;
        record.faces = None;
        record.person_id = None;
        return Ok((record, None));
    };
    let img = imgcore::decode(file)?;
    record.aesthetic = engine.aesthetic(&img).and_then(|r| r.ok());

    // Caras: usamos faces_with_boxes para tener la caja dominante
    // disponible y poder alimentar ArcFace si está cargado.
    let (face_summary, main_box) = match engine.faces_with_boxes(&img) {
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
    record.faces = face_summary;

    let embedding = if engine.has_arcface() {
        match main_box {
            Some(b) => {
                let crop = imganalyze::ai::crop_face_with_margin(&img, &b);
                engine.arcface_embed(&crop).and_then(|r| r.ok())
            }
            None => None,
        }
    } else {
        None
    };
    Ok((record, embedding))
}

#[tauri::command]
pub fn analyze_batch(
    app: AppHandle,
    analysis_state: State<'_, AnalysisState>,
    files: Vec<PathBuf>,
    use_ai: bool,
    dedup_config: Option<DedupConfig>,
) -> Result<(), String> {
    analysis_state.reset_cancel();
    if let Ok(mut results) = analysis_state.results.lock() {
        results.clear();
    }

    let cancel = analysis_state.cancel_flag.clone();
    let results_arc = analysis_state.results.clone();
    let dedup = dedup_config.unwrap_or_default();

    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let cache_path = default_cache_path(&app_data);

    let (ai_engine, model_paths) = build_ai_engine(&app, use_ai);
    let path_refs: Vec<&Path> = model_paths.iter().map(|p| p.as_path()).collect();
    let model_version = compute_model_version(use_ai && ai_engine.is_some(), &path_refs);

    // Reusa la conexión redb cacheada en AnalysisState — abrir la DB
    // adquiere file lock exclusivo, así que abrirla en cada batch
    // bloqueaba ~50 ms y rompía con corridas concurrentes.
    let cache_handle = {
        let mut guard = analysis_state
            .cache
            .lock()
            .map_err(|e| format!("lock cache: {e}"))?;
        if guard.is_none() {
            if let Ok(c) = AnalysisCache::open(&cache_path) {
                *guard = Some(Arc::new(c));
            }
        }
        guard.clone()
    };

    let app_clone = app.clone();
    std::thread::spawn(move || {
        run_analysis(
            app_clone,
            files,
            ai_engine,
            model_version,
            cache_handle,
            cancel,
            results_arc,
            dedup,
        );
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_analysis(
    app: AppHandle,
    files: Vec<PathBuf>,
    ai_engine: Option<AiEngine>,
    model_version: u32,
    cache: Option<Arc<AnalysisCache>>,
    cancel: Arc<AtomicBool>,
    results_arc: Arc<Mutex<Vec<AnalysisRecord>>>,
    dedup_config: DedupConfig,
) {
    let total = files.len();
    tracing::info!(
        total,
        has_ai = ai_engine.is_some(),
        hamming_threshold = dedup_config.hamming_threshold,
        time_window_seconds = dedup_config.time_window_seconds,
        "análisis del lote iniciado"
    );

    // Paso 1: metadata + lookup en caché por archivo (rápido, secuencial).
    let mut prepared: Vec<(PathBuf, [u8; 32], Option<CachedEntry>)> = Vec::with_capacity(total);
    let mut errores: Vec<AnalysisFileError> = Vec::new();

    for file in &files {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        match std::fs::metadata(file) {
            Ok(meta) => {
                let modified_secs = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let key = AnalysisCache::compute_key(file, modified_secs, meta.len());
                let cached = cache.as_ref().and_then(|c| c.get(&key));
                prepared.push((file.clone(), key, cached));
            }
            Err(e) => {
                errores.push(AnalysisFileError {
                    ruta: file.clone(),
                    mensaje: e.to_string(),
                });
            }
        }
    }

    // Paso 2: cómputo paralelo. Cada entrada puede:
    //  - Full hit: schema + model coinciden → usa registro cacheado
    //    junto con el embedding cacheado (None si fue cacheado pre-ArcFace).
    //  - Partial hit: schema coincide pero model no → reusa métricas
    //    objetivas, recalcula la parte de IA Y el embedding.
    //  - Miss: análisis completo (record + embedding).
    let progress = std::sync::atomic::AtomicUsize::new(0);
    let ai_ref = ai_engine.as_ref();
    // `None` = el archivo se saltó por cancelación; NO se reporta como
    // error en la UI. La señal de cancelación al final del lote se
    // propaga aparte via `cancelado`.
    let computed: Vec<Option<ComputedEntry>> = prepared
        .par_iter()
        .map(|(file, key, cached)| {
            if cancel.load(Ordering::SeqCst) {
                return None;
            }
            let n = progress.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = app.emit(
                "analysis_progress",
                AnalysisProgressEvent {
                    analizadas: n,
                    total,
                    archivo_actual: file.to_string_lossy().into_owned(),
                    etapa: "analizando".into(),
                },
            );

            let result: Result<(AnalysisRecord, Option<Vec<f32>>, CacheHit), String> = match cached
            {
                Some(entry) if entry.schema_version == SCHEMA_VERSION => {
                    if entry.model_version == model_version {
                        Ok((
                            entry.record.clone(),
                            entry.face_embedding.clone(),
                            CacheHit::Full,
                        ))
                    } else {
                        recompute_ai_fields(file, entry.record.clone(), ai_ref)
                            .map(|(r, emb)| (r, emb, CacheHit::Partial))
                            .map_err(|e| e.to_string())
                    }
                }
                _ => analyze_photo_with_embedding(file, ai_ref)
                    .map(|(r, emb)| (r, emb, CacheHit::Miss))
                    .map_err(|e| e.to_string()),
            };
            Some((file.clone(), *key, result))
        })
        .collect();

    // Paso 3: separa registros, errores, embeddings y prepara escritura al caché.
    // Los `None` (archivos cancelados) se filtran aquí — no se reportan como error.
    let mut records: Vec<AnalysisRecord> = Vec::new();
    let mut embeddings: Vec<Option<Vec<f32>>> = Vec::new();
    let mut hits: Vec<CacheHit> = Vec::new();
    let mut keys_for_write: Vec<(usize, [u8; 32])> = Vec::new();
    let mut desde_cache = 0usize;
    for (path, key, result) in computed.into_iter().flatten() {
        match result {
            Ok((rec, emb, hit)) => {
                // Usamos la longitud ACTUAL de `records` como índice
                // antes del push: así los índices de `keys_for_write`
                // siempre apuntan a una posición válida de `records`,
                // sin importar cuántos errores hubo antes.
                let i = records.len();
                if matches!(hit, CacheHit::Full) {
                    desde_cache += 1;
                } else {
                    keys_for_write.push((i, key));
                }
                records.push(rec);
                embeddings.push(emb);
                hits.push(hit);
            }
            Err(e) => errores.push(AnalysisFileError {
                ruta: path,
                mensaje: e,
            }),
        }
    }

    // Paso 3.5: clustering de personas sobre todo el lote (cache hits
    // incluidos cuando tienen embedding cacheado).
    if embeddings.iter().any(|e| e.is_some()) {
        let assignments = cluster_persons(&embeddings, ARCFACE_CLUSTER_THRESHOLD);
        for (rec, pid) in records.iter_mut().zip(assignments) {
            // Si tenemos embedding (fresh o cached) la nueva asignación
            // es autoritativa. Si no, conservamos el person_id previo
            // (que vendrá de un análisis anterior si lo había).
            if pid.is_some() {
                rec.person_id = pid;
            }
        }
    }

    // Escribe TODAS las entradas no-full-hit a la caché ahora que ya
    // tienen el person_id final del clustering.
    let to_write: Vec<([u8; 32], CachedEntry)> = keys_for_write
        .into_iter()
        .map(|(i, key)| {
            (
                key,
                CachedEntry {
                    schema_version: SCHEMA_VERSION,
                    model_version,
                    record: records[i].clone(),
                    face_embedding: embeddings[i].clone(),
                },
            )
        })
        .collect();
    if let Some(cache) = cache.as_ref() {
        let _ = cache.put_many(&to_write);
    }

    // Paso 4: agrupación de duplicados y puntaje sobre el lote completo.
    assign_duplicate_groups(&mut records, &dedup_config);
    score_batch(&mut records, &ScoringWeights::default());

    // Paso 5: pre-marca como descarte sugerido las fotos que son
    // claramente borrosas, casi-duplicadas no ganadoras o con
    // exposición severa. El usuario puede revertir cada decisión.
    for rec in records.iter_mut() {
        if should_pre_reject(rec) {
            rec.status = CullStatus::Reject;
        }
    }

    // Guarda resultados.
    if let Ok(mut guard) = results_arc.lock() {
        *guard = records.clone();
    }

    let cancelado = cancel.load(Ordering::SeqCst);
    tracing::info!(
        total,
        analizadas = records.len(),
        desde_cache,
        errores = errores.len(),
        cancelado,
        "análisis del lote terminado"
    );
    if !cancelado {
        if let Some(m) = app.try_state::<MetricsState>() {
            m.record_analysis_done();
        }
    }
    let _ = app.emit(
        "analysis_done",
        AnalysisDoneEvent {
            total,
            analizadas: records.len(),
            desde_cache,
            cancelado,
            errores,
        },
    );
}

#[derive(Debug, Clone, Copy)]
enum CacheHit {
    Full,
    Partial,
    Miss,
}

type ComputedEntry = (
    PathBuf,
    [u8; 32],
    Result<(AnalysisRecord, Option<Vec<f32>>, CacheHit), String>,
);

fn should_pre_reject(rec: &AnalysisRecord) -> bool {
    if rec.sub_scores.sharpness < 0.20 {
        return true;
    }
    if rec.sub_scores.exposure < 0.25 {
        return true;
    }
    if rec.duplicate_group.is_some() && !rec.is_group_winner {
        return true;
    }
    false
}

#[tauri::command]
pub fn list_networks() -> Vec<NetworkSpec> {
    default_networks()
}

#[tauri::command]
pub fn cancel_analysis(analysis_state: State<'_, AnalysisState>) -> Result<(), String> {
    analysis_state.request_cancel();
    Ok(())
}

#[tauri::command]
pub fn get_analysis_results(
    analysis_state: State<'_, AnalysisState>,
) -> Result<Vec<AnalysisRecord>, String> {
    let guard = analysis_state
        .results
        .lock()
        .map_err(|e| format!("lock envenenado: {e}"))?;
    Ok(guard.clone())
}

#[tauri::command]
pub fn set_cull_status(
    analysis_state: State<'_, AnalysisState>,
    path: PathBuf,
    status: CullStatus,
) -> Result<(), String> {
    let mut guard = analysis_state
        .results
        .lock()
        .map_err(|e| format!("lock envenenado: {e}"))?;
    for rec in guard.iter_mut() {
        if rec.path == path {
            rec.status = status;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_recommendations(
    analysis_state: State<'_, AnalysisState>,
    network: String,
) -> Result<Vec<Recommendation>, String> {
    let redes = default_networks();
    let red = redes
        .iter()
        .find(|n| n.id == network)
        .ok_or_else(|| format!("red desconocida: {network}"))?;

    let guard = analysis_state
        .results
        .lock()
        .map_err(|e| format!("lock envenenado: {e}"))?;
    let mut recs: Vec<Recommendation> = guard
        .iter()
        .map(|r| {
            let (sx, sy) = (0.5f32, 0.5f32);
            let fit = format_fit(r.width, r.height, sx, sy, red);
            let rec = recommendation(fit, r.composite_score);
            Recommendation {
                path: r.path.clone(),
                composite_score: r.composite_score,
                fit,
                recommendation: rec,
            }
        })
        .collect();
    recs.sort_by(|a, b| {
        b.recommendation
            .partial_cmp(&a.recommendation)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(recs)
}

#[tauri::command]
pub fn move_rejects(
    app: AppHandle,
    analysis_state: State<'_, AnalysisState>,
    target_subfolder: String,
) -> Result<MoveRejectsReport, String> {
    // Tomamos snapshot bajo lock, soltamos para no bloquear durante I/O.
    let resultados = {
        let guard = analysis_state
            .results
            .lock()
            .map_err(|e| format!("lock envenenado: {e}"))?;
        guard.clone()
    };

    let mut movidos = Vec::new();
    let mut errores = Vec::new();
    let mut path_updates: Vec<(PathBuf, PathBuf)> = Vec::new(); // (vieja, nueva)
    let considerados = resultados
        .iter()
        .filter(|r| r.status == CullStatus::Reject)
        .count();

    for rec in &resultados {
        if rec.status != CullStatus::Reject {
            continue;
        }
        let Some(parent) = rec.path.parent() else {
            errores.push(AnalysisFileError {
                ruta: rec.path.clone(),
                mensaje: "archivo sin directorio padre".into(),
            });
            continue;
        };
        let dest_dir = parent.join(&target_subfolder);
        if let Err(e) = std::fs::create_dir_all(&dest_dir) {
            errores.push(AnalysisFileError {
                ruta: rec.path.clone(),
                mensaje: format!("crear destino {}: {e}", dest_dir.display()),
            });
            continue;
        }
        let Some(file_name) = rec.path.file_name() else {
            errores.push(AnalysisFileError {
                ruta: rec.path.clone(),
                mensaje: "nombre de archivo inválido".into(),
            });
            continue;
        };
        let dest = dest_dir.join(file_name);
        match std::fs::rename(&rec.path, &dest) {
            Ok(()) => {
                path_updates.push((rec.path.clone(), dest.clone()));
                movidos.push(dest);
            }
            Err(rename_err) => {
                // Fallback cross-device (EXDEV) o errores de TCC:
                // intentar copy + remove explícito.
                match std::fs::copy(&rec.path, &dest).and_then(|_| std::fs::remove_file(&rec.path))
                {
                    Ok(()) => {
                        path_updates.push((rec.path.clone(), dest.clone()));
                        movidos.push(dest);
                    }
                    Err(copy_err) => errores.push(AnalysisFileError {
                        ruta: rec.path.clone(),
                        mensaje: format!(
                            "no se pudo mover a {}: rename={rename_err}, copy+remove={copy_err}",
                            dest.display()
                        ),
                    }),
                }
            }
        }
    }

    // Sincroniza el estado en memoria con las rutas nuevas para que
    // `get_analysis_results` no devuelva paths fantasma. La UI sigue
    // mostrando el registro (con status=Reject) pero apuntando al
    // archivo realmente movido.
    if !path_updates.is_empty() {
        if let Ok(mut guard) = analysis_state.results.lock() {
            for rec in guard.iter_mut() {
                if let Some((_, new_path)) = path_updates.iter().find(|(old, _)| *old == rec.path) {
                    rec.path = new_path.clone();
                }
            }
        }
    }

    if let Some(m) = app.try_state::<MetricsState>() {
        m.record_rejects_moved(movidos.len() as u64);
    }

    Ok(MoveRejectsReport {
        considerados,
        movidos,
        errores,
    })
}
