//! Estado global del proceso Tauri.
//!
//! - [`AppState`] mantiene la bandera de cancelación del lote de
//!   PROCESAMIENTO y el número máximo de hilos del pool de rayon.
//! - [`AnalysisState`] mantiene la cancelación del ANÁLISIS y los
//!   últimos resultados del lote (compartidos con todos los
//!   comandos de curación).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use image::DynamicImage;
use imganalyze::AnalysisRecord;

/// LRU simple para imágenes decodificadas usadas en la previsualización.
/// Capacidad por defecto 8 entradas; cada `DynamicImage` reescalada a
/// 1024 px lado mayor ocupa ~4 MB → cache total ~32 MB en RAM.
#[derive(Debug)]
pub struct PreviewCache {
    entries: VecDeque<(String, Arc<DynamicImage>)>,
    cap: usize,
}

impl PreviewCache {
    pub fn new(cap: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<Arc<DynamicImage>> {
        let idx = self.entries.iter().position(|(k, _)| k == key)?;
        let (k, v) = self.entries.remove(idx)?;
        let clone = v.clone();
        // mover al final = más reciente
        self.entries.push_back((k, v));
        Some(clone)
    }

    pub fn put(&mut self, key: String, value: Arc<DynamicImage>) {
        if let Some(idx) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(idx);
        }
        self.entries.push_back((key, value));
        while self.entries.len() > self.cap {
            self.entries.pop_front();
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub cancel_flag: Arc<AtomicBool>,
    /// 0 = usar el valor por defecto de rayon (cores lógicos).
    pub max_threads: AtomicUsize,
    /// Concurrencia máxima de procesos ffmpeg simultáneos en el lote.
    /// Los videos saturan CPU; demasiados en paralelo se ralentizan
    /// entre sí. Default: 2.
    pub video_concurrency: AtomicUsize,
    /// LRU de imágenes decodificadas para previsualización rápida.
    pub preview_cache: Mutex<PreviewCache>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            max_threads: AtomicUsize::new(0),
            video_concurrency: AtomicUsize::new(2),
            preview_cache: Mutex::new(PreviewCache::new(8)),
        }
    }

    pub fn reset_cancel(&self) {
        self.cancel_flag.store(false, Ordering::SeqCst);
    }

    pub fn request_cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct AnalysisState {
    pub cancel_flag: Arc<AtomicBool>,
    pub results: Arc<Mutex<Vec<AnalysisRecord>>>,
}

impl AnalysisState {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn reset_cancel(&self) {
        self.cancel_flag.store(false, Ordering::SeqCst);
    }

    pub fn request_cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }
}

impl Default for AnalysisState {
    fn default() -> Self {
        Self::new()
    }
}
