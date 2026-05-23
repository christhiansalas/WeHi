//! Orquestación de la cola de procesamiento.
//!
//! Se ejecuta en un hilo separado del runtime de Tauri, paraleliza
//! sobre `rayon` y emite eventos `progress`, `file_done` y
//! `batch_done` al frontend.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use imgcore::{LogoMap, Preset};

use crate::metrics::MetricsState;

/// Semáforo contado para limitar procesos ffmpeg concurrentes.
/// Bloquea hilos rayon cuando se alcanza el límite, evitando que
/// múltiples ffmpeg compitan por CPU.
struct VideoSemaphore {
    inner: Mutex<usize>,
    cv: Condvar,
}

impl VideoSemaphore {
    fn new(n: usize) -> Self {
        Self {
            inner: Mutex::new(n.max(1)),
            cv: Condvar::new(),
        }
    }

    fn permit(self: &Arc<Self>) -> VideoPermit {
        let mut count = self.inner.lock().unwrap();
        while *count == 0 {
            count = self.cv.wait(count).unwrap();
        }
        *count -= 1;
        VideoPermit {
            sem: Arc::clone(self),
        }
    }
}

struct VideoPermit {
    sem: Arc<VideoSemaphore>,
}

impl Drop for VideoPermit {
    fn drop(&mut self) {
        let mut count = self.sem.inner.lock().unwrap();
        *count += 1;
        self.sem.cv.notify_one();
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct ProgressEvent {
    pub procesadas: usize,
    pub total: usize,
    pub archivo_actual: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct FileDoneEvent {
    #[ts(type = "string")]
    pub ruta: PathBuf,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct FileError {
    #[ts(type = "string")]
    pub ruta: PathBuf,
    pub mensaje: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct BatchDoneEvent {
    pub total: usize,
    pub exitosas: usize,
    pub fallidas: usize,
    pub cancelado: bool,
    pub errores: Vec<FileError>,
}

/// Avance dentro de un video puntual mientras ffmpeg lo procesa.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct VideoProgressEvent {
    pub ruta: String,
    /// Microsegundos procesados del video.
    #[ts(type = "number")]
    pub current_us: u64,
    /// Duración total en microsegundos (0 si no se pudo determinar).
    #[ts(type = "number")]
    pub total_us: u64,
}

/// Sesión activa de procesamiento. Se persiste a disco en cada
/// `file_done` para permitir reanudar tras cierre o cancelación.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct ActiveBatch {
    pub started_at: String,
    pub preset: Preset,
    pub files: Vec<String>,
    pub completed: Vec<String>,
    pub failed: Vec<String>,
}

fn active_batch_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|p| p.join("active_batch.json"))
}

fn write_active_batch(app: &AppHandle, batch: &ActiveBatch) {
    // Escribe atómico: tmp + rename. Sin esto un crash a mitad del
    // write deja `active_batch.json` truncado y la próxima corrida
    // no puede deserializarlo (silent corruption → batch perdido).
    let Some(path) = active_batch_path(app) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(json) = serde_json::to_string(batch) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &json).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

fn clear_active_batch_file(app: &AppHandle) {
    if let Some(path) = active_batch_path(app) {
        let _ = std::fs::remove_file(path);
    }
}

pub fn read_active_batch(app: &AppHandle) -> Option<ActiveBatch> {
    let path = active_batch_path(app)?;
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn clear_active_batch(app: &AppHandle) {
    clear_active_batch_file(app);
}

fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Unix timestamp seconds — el frontend lo formatea si lo necesita.
    format!("{secs}")
}

fn path_to_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Ejecuta el lote: carga el logo UNA vez si aplica, paraleliza
/// con un pool de rayon de tamaño `max_threads` (0 = default). Los
/// videos pasan por un semáforo de tamaño `video_concurrency` para
/// no saturar CPU.
#[allow(clippy::too_many_arguments)]
pub fn run_batch(
    app: AppHandle,
    files: Vec<PathBuf>,
    preset: Preset,
    logos: LogoMap,
    cancel: Arc<AtomicBool>,
    max_threads: usize,
    video_concurrency: usize,
) {
    let total = files.len();
    let procesadas = AtomicUsize::new(0);
    let exitosas = AtomicUsize::new(0);
    let fallidas = AtomicUsize::new(0);
    let errores: Mutex<Vec<FileError>> = Mutex::new(Vec::new());

    let videos = files.iter().filter(|p| imgcore::video::is_video(p)).count();
    let n_logos = preset.all_logos().count();
    tracing::info!(
        total,
        videos,
        max_threads,
        video_concurrency,
        n_logos,
        output_format = ?preset.output.format,
        "lote de procesamiento iniciado"
    );

    // Inicializa el archivo de batch activo para que se pueda
    // reanudar si la app se cierra o se cancela.
    let active_state = Arc::new(Mutex::new(ActiveBatch {
        started_at: now_iso(),
        preset: preset.clone(),
        files: files.iter().map(|p| path_to_string(p)).collect(),
        completed: Vec::new(),
        failed: Vec::new(),
    }));
    if let Ok(guard) = active_state.lock() {
        write_active_batch(&app, &guard);
    }

    let pool = if max_threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(max_threads)
            .build()
            .ok()
    } else {
        None
    };

    let logos_ref = &logos;
    let preset_ref = &preset;
    let video_sem = Arc::new(VideoSemaphore::new(video_concurrency));

    let work = || {
        files.par_iter().for_each(|file| {
            if cancel.load(Ordering::SeqCst) {
                return;
            }
            // Si es video, espera turno en el semáforo. Suelta al
            // final del scope (Drop). Los archivos no-video pasan
            // libremente y aprovechan todos los hilos del pool.
            let _video_permit = if imgcore::video::is_video(file) {
                Some(video_sem.permit())
            } else {
                None
            };
            if cancel.load(Ordering::SeqCst) {
                return;
            }
            // Para videos: emite `video_progress` mientras ffmpeg
            // trabaja, con throttling a ~10 Hz para no saturar el bridge.
            let file_str = file.to_string_lossy().into_owned();
            let app_for_progress = app.clone();
            let last_emit_us = AtomicU64::new(0);
            let on_video_progress = move |current_us: u64, total_us: u64| {
                let last = last_emit_us.load(Ordering::Relaxed);
                if current_us == total_us || current_us.saturating_sub(last) >= 100_000 {
                    last_emit_us.store(current_us, Ordering::Relaxed);
                    let _ = app_for_progress.emit(
                        "video_progress",
                        VideoProgressEvent {
                            ruta: file_str.clone(),
                            current_us,
                            total_us,
                        },
                    );
                }
            };
            let resultado = imgcore::process_file_with_progress(
                file,
                preset_ref,
                logos_ref,
                Some(cancel.clone()),
                on_video_progress,
            );
            let ok = resultado.is_ok();
            let err = resultado.err().map(|e| e.to_string());

            let n = procesadas.fetch_add(1, Ordering::SeqCst) + 1;
            if ok {
                exitosas.fetch_add(1, Ordering::SeqCst);
            } else {
                fallidas.fetch_add(1, Ordering::SeqCst);
                if let Some(msg) = &err {
                    tracing::warn!(
                        file = %file.display(),
                        error = %msg,
                        "archivo falló en el lote"
                    );
                    if let Ok(mut guard) = errores.lock() {
                        guard.push(FileError {
                            ruta: file.clone(),
                            mensaje: msg.clone(),
                        });
                    }
                }
            }

            // Actualiza el batch activo en disco con el progreso.
            if let Ok(mut state) = active_state.lock() {
                let path_str = path_to_string(file);
                if ok {
                    state.completed.push(path_str);
                } else {
                    state.failed.push(path_str);
                }
                write_active_batch(&app, &state);
            }

            let _ = app.emit(
                "progress",
                ProgressEvent {
                    procesadas: n,
                    total,
                    archivo_actual: file.to_string_lossy().into_owned(),
                },
            );
            let _ = app.emit(
                "file_done",
                FileDoneEvent {
                    ruta: file.clone(),
                    ok,
                    error: err,
                },
            );
        });
    };

    if let Some(pool) = pool {
        pool.install(work);
    } else {
        work();
    }

    let cancelado = cancel.load(Ordering::SeqCst);
    let exitosas_n = exitosas.load(Ordering::SeqCst) as u64;
    let fallidas_n = fallidas.load(Ordering::SeqCst) as u64;
    tracing::info!(
        total,
        exitosas = exitosas_n,
        fallidas = fallidas_n,
        cancelado,
        "lote de procesamiento terminado"
    );
    // Si terminó limpio (no cancelado), borra el archivo de batch activo.
    if !cancelado {
        clear_active_batch_file(&app);
    }
    // Registra métricas (no-op si el usuario las tiene desactivadas).
    if let Some(metrics) = app.try_state::<MetricsState>() {
        metrics.record_batch_done(exitosas_n, fallidas_n, cancelado, videos as u64);
    }

    let _ = app.emit(
        "batch_done",
        BatchDoneEvent {
            total,
            exitosas: exitosas.load(Ordering::SeqCst),
            fallidas: fallidas.load(Ordering::SeqCst),
            cancelado,
            errores: errores.into_inner().unwrap_or_default(),
        },
    );
}
