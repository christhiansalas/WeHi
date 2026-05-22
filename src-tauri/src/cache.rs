//! Caché embebido de resultados del análisis.
//!
//! - Backed por `redb` en `app_data/cache/analysis_v1.redb`.
//! - Clave: BLAKE3 de (ruta absoluta + fecha de modificación en
//!   segundos + tamaño del archivo).
//! - Versiona por separado:
//!   - `schema_version` — cambia si cambia el formato de las métricas
//!     objetivas. Un cambio invalida toda la entrada.
//!   - `model_version` — cambia si cambian los archivos del modelo de
//!     IA. Un cambio invalida SOLO la parte de IA (estética y caras);
//!     las métricas objetivas se reutilizan.

use std::path::{Path, PathBuf};

use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};

use imganalyze::AnalysisRecord;

pub const SCHEMA_VERSION: u32 = 1;

const TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("analysis_v1");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    pub schema_version: u32,
    pub model_version: u32,
    pub record: AnalysisRecord,
}

pub struct AnalysisCache {
    db: Database,
}

impl AnalysisCache {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Database::create(path).map_err(|e| e.to_string())?;
        // Asegura que la tabla existe.
        let txn = db.begin_write().map_err(|e| e.to_string())?;
        {
            let _ = txn.open_table(TABLE).map_err(|e| e.to_string())?;
        }
        txn.commit().map_err(|e| e.to_string())?;
        Ok(Self { db })
    }

    pub fn compute_key(path: &Path, modified_secs: u64, file_size: u64) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(&modified_secs.to_le_bytes());
        hasher.update(&file_size.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn get(&self, key: &[u8; 32]) -> Option<CachedEntry> {
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(TABLE).ok()?;
        let value = table.get(key.as_slice()).ok().flatten()?;
        serde_json::from_slice(value.value()).ok()
    }

    pub fn put_many(&self, entries: &[([u8; 32], CachedEntry)]) -> Result<(), String> {
        let txn = self.db.begin_write().map_err(|e| e.to_string())?;
        {
            let mut table = txn.open_table(TABLE).map_err(|e| e.to_string())?;
            for (key, entry) in entries {
                let bytes = serde_json::to_vec(entry).map_err(|e| e.to_string())?;
                table
                    .insert(key.as_slice(), bytes.as_slice())
                    .map_err(|e| e.to_string())?;
            }
        }
        txn.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Calcula un "model_version" como BLAKE3 truncado a 32 bits de
/// (rutas + tamaños + mtime) de los archivos de modelo dados. Si
/// `use_ai` es false, devuelve 0 (sin IA).
pub fn compute_model_version(use_ai: bool, paths: &[&Path]) -> u32 {
    if !use_ai {
        return 0;
    }
    let mut hasher = blake3::Hasher::new();
    for p in paths {
        let Ok(meta) = std::fs::metadata(p) else {
            continue;
        };
        hasher.update(p.to_string_lossy().as_bytes());
        hasher.update(&meta.len().to_le_bytes());
        if let Ok(modified) = meta.modified() {
            if let Ok(d) = modified.duration_since(std::time::UNIX_EPOCH) {
                hasher.update(&d.as_secs().to_le_bytes());
            }
        }
    }
    let bytes = hasher.finalize();
    u32::from_le_bytes(bytes.as_bytes()[..4].try_into().unwrap())
}

pub fn default_cache_path(app_data: &Path) -> PathBuf {
    app_data.join("cache").join("analysis_v1.redb")
}
