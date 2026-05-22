//! Tethered shoot: observa una carpeta y emite eventos cuando aparece
//! un archivo nuevo soportado.
//!
//! Útil para fotógrafos en evento: la cámara escribe en una carpeta y
//! WeHi añade automáticamente cada foto a la cola (y opcionalmente la
//! procesa al instante).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use ts_rs::TS;


/// Mismo set que `commands::SUPPORTED_EXTENSIONS`. Lo replicamos aquí
/// para no acoplar módulos.
const SUPPORTED_EXTENSIONS: &[&str] = &[
    // Imágenes
    "jpg", "jpeg", "png", "webp", "cr2", "cr3", "nef", "nrw",
    // Videos
    "mp4", "mov", "m4v", "webm", "mkv", "avi",
];

fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.iter().any(|s| s.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

#[derive(Debug, Default)]
pub struct WatchState {
    inner: Mutex<Option<WatchSession>>,
}

#[derive(Debug)]
struct WatchSession {
    _watcher: RecommendedWatcher,
    folder: PathBuf,
}

impl WatchState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct FileAppearedEvent {
    #[ts(type = "string")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct WatchStatus {
    pub watching: bool,
    #[ts(type = "string | null")]
    pub folder: Option<PathBuf>,
}

/// Las cámaras suelen escribir el archivo en varios pasos (touch +
/// rename desde un .tmp). Eventos `Create` pueden dispararse antes de
/// que el archivo esté completo. Esperamos a que el tamaño se
/// estabilice antes de emitirlo.
fn wait_until_stable(path: &Path, max_wait: Duration) -> bool {
    let start = Instant::now();
    let mut last_size: Option<u64> = None;
    while start.elapsed() < max_wait {
        let Ok(meta) = std::fs::metadata(path) else {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        };
        let size = meta.len();
        match last_size {
            Some(s) if s == size && size > 0 => return true,
            _ => {
                last_size = Some(size);
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    false
}

#[tauri::command]
pub fn start_watching(
    app: AppHandle,
    state: State<'_, WatchState>,
    path: PathBuf,
) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!("no es una carpeta: {}", path.display()));
    }
    // Cierra cualquier watcher previo antes de crear el nuevo.
    {
        let mut guard = state
            .inner
            .lock()
            .map_err(|e| format!("watch lock: {e}"))?;
        *guard = None;
    }

    let app_for_event = app.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            let Ok(event) = res else {
                return;
            };
            // Solo nos interesan creaciones (cámara escribiendo).
            if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                return;
            }
            for path in event.paths {
                if !path.is_file() || !is_supported(&path) {
                    continue;
                }
                // Espera a que el archivo termine de escribirse (5 s max).
                if !wait_until_stable(&path, Duration::from_secs(5)) {
                    tracing::warn!(
                        path = %path.display(),
                        "archivo no se estabilizó; lo emitimos igual"
                    );
                }
                tracing::info!(path = %path.display(), "tethered: archivo nuevo");
                let _ = app_for_event.emit(
                    "file_appeared",
                    FileAppearedEvent { path },
                );
            }
        },
        Config::default(),
    )
    .map_err(|e| format!("crear watcher: {e}"))?;

    watcher
        .watch(&path, RecursiveMode::Recursive)
        .map_err(|e| format!("watch {}: {e}", path.display()))?;

    let mut guard = state
        .inner
        .lock()
        .map_err(|e| format!("watch lock: {e}"))?;
    *guard = Some(WatchSession {
        _watcher: watcher,
        folder: path.clone(),
    });

    tracing::info!(folder = %path.display(), "tethered shoot iniciado");
    Ok(())
}

#[tauri::command]
pub fn stop_watching(state: State<'_, WatchState>) -> Result<(), String> {
    let mut guard = state
        .inner
        .lock()
        .map_err(|e| format!("watch lock: {e}"))?;
    let had = guard.is_some();
    *guard = None;
    if had {
        tracing::info!("tethered shoot detenido");
    }
    Ok(())
}

#[tauri::command]
pub fn watch_status(state: State<'_, WatchState>) -> Result<WatchStatus, String> {
    let guard = state
        .inner
        .lock()
        .map_err(|e| format!("watch lock: {e}"))?;
    Ok(match guard.as_ref() {
        Some(session) => WatchStatus {
            watching: true,
            folder: Some(session.folder.clone()),
        },
        None => WatchStatus {
            watching: false,
            folder: None,
        },
    })
}
