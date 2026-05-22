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
    analyze_photo, assign_duplicate_groups, default_networks, format_fit, recommendation,
    score_batch, AiEngine, AiEngineConfig, AnalysisRecord, CullStatus, DedupConfig, NetworkSpec,
    ScoringWeights,
};

use crate::cache::{compute_model_version, default_cache_path, AnalysisCache, CachedEntry, SCHEMA_VERSION};
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

fn ai_engine_paths(app: &AppHandle) -> (Option<PathBuf>, Option<PathBuf>) {
    (
        find_model(app, "aesthetic.onnx"),
        find_model(app, "faces.onnx"),
    )
}

fn build_ai_engine(app: &AppHandle, use_ai: bool) -> (Option<AiEngine>, Vec<PathBuf>) {
    if !use_ai {
        return (None, Vec::new());
    }
    let (aest, faces) = ai_engine_paths(app);
    let mut cfg = AiEngineConfig::new();
    cfg.aesthetic_model = aest.clone();
    cfg.faces_model = faces.clone();
    let engine = AiEngine::new(&cfg).ok();
    let mut paths = Vec::new();
    if let Some(p) = aest {
        paths.push(p);
    }
    if let Some(p) = faces {
        paths.push(p);
    }
    (engine, paths)
}

fn recompute_ai_fields(
    file: &Path,
    mut record: AnalysisRecord,
    ai: Option<&AiEngine>,
) -> Result<AnalysisRecord, imgcore::ImgError> {
    if let Some(engine) = ai {
        let img = imgcore::decode(file)?;
        record.aesthetic = engine.aesthetic(&img).and_then(|r| r.ok());
        record.faces = engine.faces(&img).and_then(|r| r.ok());
    } else {
        record.aesthetic = None;
        record.faces = None;
    }
    Ok(record)
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

    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let cache_path = default_cache_path(&app_data);

    let (ai_engine, model_paths) = build_ai_engine(&app, use_ai);
    let path_refs: Vec<&Path> = model_paths.iter().map(|p| p.as_path()).collect();
    let model_version = compute_model_version(use_ai && ai_engine.is_some(), &path_refs);

    let app_clone = app.clone();
    std::thread::spawn(move || {
        run_analysis(
            app_clone,
            files,
            ai_engine,
            model_version,
            cache_path,
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
    cache_path: PathBuf,
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
    let cache = AnalysisCache::open(&cache_path).ok();

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
    //  - Full hit: schema + model coinciden → usa registro cacheado.
    //  - Partial hit: schema coincide pero model no → reusa métricas,
    //    recalcula SOLO la parte de IA.
    //  - Miss: análisis completo.
    let progress = std::sync::atomic::AtomicUsize::new(0);
    let ai_ref = ai_engine.as_ref();
    let computed: Vec<ComputedEntry> = prepared
        .par_iter()
        .map(|(file, key, cached)| {
            if cancel.load(Ordering::SeqCst) {
                return (file.clone(), *key, Err("cancelado".to_string()));
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

            let result: Result<(AnalysisRecord, CacheHit), String> = match cached {
                Some(entry) if entry.schema_version == SCHEMA_VERSION => {
                    if entry.model_version == model_version {
                        Ok((entry.record.clone(), CacheHit::Full))
                    } else {
                        recompute_ai_fields(file, entry.record.clone(), ai_ref)
                            .map(|r| (r, CacheHit::Partial))
                            .map_err(|e| e.to_string())
                    }
                }
                _ => analyze_photo(file, ai_ref)
                    .map(|r| (r, CacheHit::Miss))
                    .map_err(|e| e.to_string()),
            };
            (file.clone(), *key, result)
        })
        .collect();

    // Paso 3: separa registros, errores y prepara escritura al caché.
    let mut records: Vec<AnalysisRecord> = Vec::new();
    let mut desde_cache = 0usize;
    let mut to_write: Vec<([u8; 32], CachedEntry)> = Vec::new();
    for (path, key, result) in computed {
        match result {
            Ok((rec, hit)) => {
                if matches!(hit, CacheHit::Full) {
                    desde_cache += 1;
                } else {
                    to_write.push((
                        key,
                        CachedEntry {
                            schema_version: SCHEMA_VERSION,
                            model_version,
                            record: rec.clone(),
                        },
                    ));
                }
                records.push(rec);
            }
            Err(e) => errores.push(AnalysisFileError {
                ruta: path,
                mensaje: e,
            }),
        }
    }

    // Escribe todas las entradas nuevas/actualizadas en una sola
    // transacción para minimizar el overhead.
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

type ComputedEntry = (PathBuf, [u8; 32], Result<(AnalysisRecord, CacheHit), String>);

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
    let resultados = {
        let guard = analysis_state
            .results
            .lock()
            .map_err(|e| format!("lock envenenado: {e}"))?;
        guard.clone()
    };

    let mut movidos = Vec::new();
    let mut errores = Vec::new();
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
            Ok(()) => movidos.push(dest),
            Err(rename_err) => {
                // Fallback cross-device (EXDEV) o errores de TCC:
                // intentar copy + remove explícito.
                match std::fs::copy(&rec.path, &dest)
                    .and_then(|_| std::fs::remove_file(&rec.path))
                {
                    Ok(()) => movidos.push(dest),
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

    if let Some(m) = app.try_state::<MetricsState>() {
        m.record_rejects_moved(movidos.len() as u64);
    }

    Ok(MoveRejectsReport {
        considerados,
        movidos,
        errores,
    })
}
