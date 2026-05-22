//! Métricas locales del uso de WeHi.
//!
//! Solo viven en `<app_data>/metrics.json`. No salen de la máquina:
//! sirven para que el usuario vea cuánto se procesa y a qué redes
//! exporta, y para decidir qué pulir.
//!
//! Las métricas se incrementan en memoria con un `Mutex` y se
//! persisten en cada actualización (escritura completa, archivo
//! pequeño <1 KB). Si el usuario desactiva el tracking (`enabled = false`)
//! las llamadas a `increment_*` salen sin tocar disco.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct Metrics {
    pub enabled: bool,
    #[ts(type = "number")]
    pub images_processed: u64,
    #[ts(type = "number")]
    pub videos_processed: u64,
    #[ts(type = "number")]
    pub files_failed: u64,
    #[ts(type = "number")]
    pub batches_completed: u64,
    #[ts(type = "number")]
    pub batches_cancelled: u64,
    #[ts(type = "number")]
    pub analyses_completed: u64,
    #[ts(type = "number")]
    pub rejects_moved: u64,
    #[ts(type = "number")]
    pub recommendations_exported: u64,
    #[ts(type = "number")]
    pub first_seen_unix: u64,
}

#[derive(Debug)]
pub struct MetricsState {
    inner: Mutex<Metrics>,
    path: Mutex<Option<PathBuf>>,
}

impl MetricsState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Metrics::default()),
            path: Mutex::new(None),
        }
    }

    /// Llamar una vez al arrancar la app. Carga lo que haya en disco
    /// o crea un registro nuevo desactivado.
    pub fn init(&self, app_data_dir: &Path) {
        let path = app_data_dir.join("metrics.json");
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<Metrics>(&raw) {
                if let Ok(mut guard) = self.inner.lock() {
                    *guard = parsed;
                }
            }
        } else if let Ok(mut guard) = self.inner.lock() {
            guard.first_seen_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
        }
        if let Ok(mut guard) = self.path.lock() {
            *guard = Some(path);
        }
    }

    fn persist(&self) {
        let path = match self.path.lock() {
            Ok(p) => p.clone(),
            Err(_) => return,
        };
        let Some(path) = path else {
            return;
        };
        let metrics = match self.inner.lock() {
            Ok(m) => m.clone(),
            Err(_) => return,
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&metrics) {
            let _ = std::fs::write(&path, json);
        }
    }

    fn enabled(&self) -> bool {
        self.inner.lock().map(|m| m.enabled).unwrap_or(false)
    }

    pub fn record_batch_done(&self, exitosas: u64, fallidas: u64, cancelado: bool, videos: u64) {
        if !self.enabled() {
            return;
        }
        if let Ok(mut m) = self.inner.lock() {
            m.images_processed += exitosas.saturating_sub(videos);
            m.videos_processed += videos.min(exitosas);
            m.files_failed += fallidas;
            if cancelado {
                m.batches_cancelled += 1;
            } else {
                m.batches_completed += 1;
            }
        }
        self.persist();
    }

    pub fn record_analysis_done(&self) {
        if !self.enabled() {
            return;
        }
        if let Ok(mut m) = self.inner.lock() {
            m.analyses_completed += 1;
        }
        self.persist();
    }

    pub fn record_rejects_moved(&self, n: u64) {
        if !self.enabled() {
            return;
        }
        if let Ok(mut m) = self.inner.lock() {
            m.rejects_moved += n;
        }
        self.persist();
    }

    #[allow(dead_code)]
    pub fn record_recommendation_export(&self, n: u64) {
        if !self.enabled() {
            return;
        }
        if let Ok(mut m) = self.inner.lock() {
            m.recommendations_exported += n;
        }
        self.persist();
    }
}

impl Default for MetricsState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub fn get_metrics(state: State<'_, MetricsState>) -> Result<Metrics, String> {
    state
        .inner
        .lock()
        .map(|m| m.clone())
        .map_err(|e| format!("metrics lock: {e}"))
}

#[tauri::command]
pub fn set_metrics_enabled(state: State<'_, MetricsState>, enabled: bool) -> Result<(), String> {
    {
        let mut m = state
            .inner
            .lock()
            .map_err(|e| format!("metrics lock: {e}"))?;
        m.enabled = enabled;
    }
    state.persist();
    Ok(())
}

#[tauri::command]
pub fn clear_metrics(state: State<'_, MetricsState>) -> Result<(), String> {
    {
        let mut m = state
            .inner
            .lock()
            .map_err(|e| format!("metrics lock: {e}"))?;
        let enabled = m.enabled;
        let first_seen = m.first_seen_unix;
        *m = Metrics::default();
        m.enabled = enabled;
        m.first_seen_unix = first_seen;
    }
    state.persist();
    Ok(())
}
