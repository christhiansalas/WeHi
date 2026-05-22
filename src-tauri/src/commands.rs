//! Comandos Tauri de procesamiento, biblioteca de logos y presets.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

use imgcore::{OutputFormat, Preset};

use crate::queue;
use crate::state::AppState;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    // Imágenes
    "jpg", "jpeg", "png", "webp", "cr2", "cr3", "nef", "nrw", // Videos (vía ffmpeg)
    "mp4", "mov", "m4v", "webm", "mkv", "avi",
];

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct LogoEntry {
    pub id: String,
    pub name: String,
    #[ts(type = "string")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct PreviewResult {
    pub base64: String,
    pub width: u32,
    pub height: u32,
    pub format: OutputFormat,
}

fn logos_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(base.join("logos"))
}

fn presets_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(base.join("presets"))
}

fn preferences_path(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(base.join("preferences.json"))
}

fn thumbs_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(base.join("thumbs"))
}

fn file_fingerprint(file: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(file).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some((mtime, meta.len()))
}

fn thumb_cache_key(file: &Path, size: u32) -> Option<String> {
    let (mtime, size_bytes) = file_fingerprint(file)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(file.to_string_lossy().as_bytes());
    hasher.update(&mtime.to_le_bytes());
    hasher.update(&size_bytes.to_le_bytes());
    hasher.update(&size.to_le_bytes());
    Some(hasher.finalize().to_hex().to_string())
}

fn preview_cache_key(file: &Path) -> String {
    if let Some((mtime, size)) = file_fingerprint(file) {
        format!("{}|{mtime}|{size}", file.display())
    } else {
        file.display().to_string()
    }
}

const PREVIEW_MAX_DIM: u32 = 1024;

/// Decodifica `file` (imagen o frame de video) y, si supera
/// `PREVIEW_MAX_DIM`, lo reescala. Sirve como capa rápida para la
/// previsualización: en imágenes grandes (24 MP+) ahorra ~95% del
/// cómputo en cada cambio de slider del preset.
fn decode_for_preview(file: &Path) -> Result<image::DynamicImage, String> {
    let decoded = if imgcore::video::is_video(file) {
        imgcore::video::extract_frame(file, 0.5).map_err(|e| e.to_string())?
    } else {
        imgcore::decode(file).map_err(|e| e.to_string())?
    };
    use image::GenericImageView;
    let (w, h) = decoded.dimensions();
    if w.max(h) <= PREVIEW_MAX_DIM {
        return Ok(decoded);
    }
    imgcore::resize_image(
        decoded,
        &imgcore::ResizeConfig {
            mode: imgcore::ResizeMode::MaxDimension,
            width: Some(PREVIEW_MAX_DIM),
            height: Some(PREVIEW_MAX_DIM),
            percentage: None,
            keep_aspect_ratio: true,
            allow_upscale: false,
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_preferences(app: AppHandle) -> Result<serde_json::Value, String> {
    let path = preferences_path(&app)?;
    if !path.exists() {
        return Ok(serde_json::Value::Object(Default::default()));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_preferences(app: AppHandle, preferences: serde_json::Value) -> Result<(), String> {
    let path = preferences_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&preferences).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_active_batch(app: AppHandle) -> Option<crate::queue::ActiveBatch> {
    crate::queue::read_active_batch(&app)
}

#[tauri::command]
pub fn clear_active_batch(app: AppHandle) -> Result<(), String> {
    crate::queue::clear_active_batch(&app);
    Ok(())
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "sin_nombre".to_string()
    } else {
        cleaned
    }
}

fn walk_supported(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ftype = entry.file_type()?;
        if ftype.is_dir() {
            walk_supported(&path, out)?;
        } else if ftype.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SUPPORTED_EXTENSIONS
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case(ext))
                {
                    out.push(path);
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn scan_folder(path: PathBuf) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    walk_supported(&path, &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Acepta una lista de rutas (archivos y/o carpetas) y devuelve los
/// archivos soportados. Las carpetas se recorren recursivamente.
/// Usado por el drag-and-drop.
#[tauri::command]
pub fn scan_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            walk_supported(&path, &mut out).map_err(|e| e.to_string())?;
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SUPPORTED_EXTENSIONS
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case(ext))
                {
                    out.push(path);
                }
            }
        }
    }
    Ok(out)
}

fn resolve_logos(app: &AppHandle, preset: &Preset) -> Result<imgcore::LogoMap, String> {
    use std::collections::HashMap;
    let mut map: HashMap<String, imgcore::LoadedLogo> = HashMap::new();
    let dir = logos_dir(app)?;
    for cfg in preset.all_logos() {
        if map.contains_key(&cfg.logo_id) {
            continue;
        }
        let path = dir.join(&cfg.logo_id);
        let logo = imgcore::load_logo(&path).map_err(|e| e.to_string())?;
        map.insert(cfg.logo_id.clone(), logo);
    }
    Ok(map)
}

#[tauri::command]
pub fn process_batch(
    app: AppHandle,
    state: State<'_, AppState>,
    files: Vec<PathBuf>,
    preset: Preset,
) -> Result<(), String> {
    state.reset_cancel();
    let cancel = state.cancel_flag.clone();
    let max_threads = state.max_threads.load(Ordering::Relaxed);
    let video_concurrency = state.video_concurrency.load(Ordering::Relaxed).max(1);

    let logos = resolve_logos(&app, &preset)?;
    let app_clone = app.clone();
    std::thread::spawn(move || {
        queue::run_batch(
            app_clone,
            files,
            preset,
            logos,
            cancel,
            max_threads,
            video_concurrency,
        );
    });
    Ok(())
}

#[tauri::command]
pub fn process_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    file: PathBuf,
    preset: Preset,
) -> Result<PreviewResult, String> {
    let logos = resolve_logos(&app, &preset)?;

    // Cache LRU de imágenes ya decodificadas + reescaladas a ≤1024 px.
    // Hace que mover sliders del preset reuse la decodificación.
    let key = preview_cache_key(&file);
    let cached = {
        let mut guard = state
            .preview_cache
            .lock()
            .map_err(|e| format!("preview_cache lock: {e}"))?;
        guard.get(&key)
    };

    let source: std::sync::Arc<image::DynamicImage> = if let Some(c) = cached {
        c
    } else {
        let decoded = decode_for_preview(&file)?;
        let arc = std::sync::Arc::new(decoded);
        if let Ok(mut guard) = state.preview_cache.lock() {
            guard.put(key, arc.clone());
        }
        arc
    };

    let img = (*source).clone();
    let encoded = imgcore::encode_pipeline(img, &preset, &logos).map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&encoded.bytes);
    Ok(PreviewResult {
        base64: b64,
        width: encoded.width,
        height: encoded.height,
        format: encoded.format,
    })
}

#[tauri::command]
pub fn cancel_batch(state: State<'_, AppState>) -> Result<(), String> {
    state.request_cancel();
    Ok(())
}

#[tauri::command]
pub fn set_max_threads(state: State<'_, AppState>, max: usize) -> Result<(), String> {
    state.max_threads.store(max, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn set_video_concurrency(state: State<'_, AppState>, max: usize) -> Result<(), String> {
    state.video_concurrency.store(max.max(1), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn list_logos(app: AppHandle) -> Result<Vec<LogoEntry>, String> {
    let dir = logos_dir(&app)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let id = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "nombre de archivo inválido".to_string())?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| id.clone());
        out.push(LogoEntry { id, name, path });
    }
    Ok(out)
}

#[tauri::command]
pub fn add_logo(app: AppHandle, source_path: PathBuf) -> Result<LogoEntry, String> {
    let dir = logos_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let original = source_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "ruta de origen inválida".to_string())?
        .to_string();
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("logo");
    let ext = source_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let id = format!("{}_{}.{}", sanitize(stem), timestamp, ext);
    let dest = dir.join(&id);
    std::fs::copy(&source_path, &dest).map_err(|e| e.to_string())?;
    Ok(LogoEntry {
        id,
        name: original,
        path: dest,
    })
}

#[tauri::command]
pub fn delete_logo(app: AppHandle, id: String) -> Result<(), String> {
    let dir = logos_dir(&app)?;
    let path = dir.join(&id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_thumbnail_data_url(app: AppHandle, file: PathBuf, size: u32) -> Result<String, String> {
    // 1) Si hay caché en disco para este (archivo + mtime + size + dim),
    //    devolverlo sin decodificar nada.
    let dir = thumbs_dir(&app)?;
    let key = thumb_cache_key(&file, size);
    if let Some(ref k) = key {
        let cached = dir.join(format!("{k}.jpg"));
        if let Ok(bytes) = std::fs::read(&cached) {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(format!("data:image/jpeg;base64,{b64}"));
        }
    }

    // 2) Generar. Soporta imagen y video (frame del medio del video).
    let img = if imgcore::video::is_video(&file) {
        imgcore::video::extract_frame(&file, 0.5).map_err(|e| e.to_string())?
    } else {
        imgcore::decode(&file).map_err(|e| e.to_string())?
    };
    let resized = imgcore::resize_image(
        img,
        &imgcore::ResizeConfig {
            mode: imgcore::ResizeMode::MaxDimension,
            width: Some(size),
            height: Some(size),
            percentage: None,
            keep_aspect_ratio: true,
            allow_upscale: false,
        },
    )
    .map_err(|e| e.to_string())?;
    let bytes = imgcore::encode_image(
        &resized,
        &imgcore::OutputConfig {
            format: OutputFormat::Jpeg,
            quality: 70,
            folder: PathBuf::from("."),
            filename_pattern: String::new(),
            preserve_structure: false,
        },
    )
    .map_err(|e| e.to_string())?;

    // 3) Persistir a disco (best effort: si falla, sigue funcionando
    //    pero el thumbnail se regenerará la próxima vez).
    if let Some(ref k) = key {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(format!("{k}.jpg")), &bytes);
    }

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/jpeg;base64,{b64}"))
}

/// Borra el caché de miniaturas. Útil tras renombrar/mover archivos
/// o si el caché creció demasiado.
#[tauri::command]
pub fn clear_thumbnail_cache(app: AppHandle) -> Result<usize, String> {
    let dir = thumbs_dir(&app)?;
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jpg")
            && std::fs::remove_file(&path).is_ok()
        {
            count += 1;
        }
    }
    Ok(count)
}

/// Estado de ffmpeg: ruta detectada (si la hay) y por tanto si el
/// procesamiento de video está disponible.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct FfmpegStatus {
    pub available: bool,
    #[ts(type = "string | null")]
    pub path: Option<PathBuf>,
}

#[tauri::command]
pub fn check_ffmpeg() -> FfmpegStatus {
    let path = imgcore::video::find_ffmpeg();
    FfmpegStatus {
        available: path.is_some(),
        path,
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct FontStatus {
    pub available: bool,
    #[ts(type = "string | null")]
    pub path: Option<PathBuf>,
}

#[tauri::command]
pub fn check_font() -> FontStatus {
    let path = imgcore::text::find_system_font();
    FontStatus {
        available: path.is_some(),
        path,
    }
}

#[tauri::command]
pub fn get_file_data_url(file: PathBuf) -> Result<String, String> {
    let (bytes, mime) = if imgcore::raw::is_raw(&file) {
        let preview = imgcore::raw::extract_raw_preview(&file).map_err(|e| e.to_string())?;
        (preview, "image/jpeg")
    } else {
        let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mime = match ext.as_str() {
            "png" => "image/png",
            "webp" => "image/webp",
            _ => "image/jpeg",
        };
        (bytes, mime)
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

#[tauri::command]
pub fn get_logo_data_url(app: AppHandle, id: String) -> Result<String, String> {
    let dir = logos_dir(&app)?;
    let path = dir.join(&id);
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{mime};base64,{b64}"))
}

#[tauri::command]
pub fn list_presets(app: AppHandle) -> Result<Vec<Preset>, String> {
    let dir = presets_dir(&app)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Ok(mut preset) = serde_json::from_str::<Preset>(&json) {
            // Migración: presets viejos solo tenían `logo: Option<LogoConfig>`.
            // Si después de deserializar el campo `logos` quedó vacío
            // pero hay un `logo` legacy, lo movemos al nuevo Vec y
            // limpiamos el slot legacy. Así la UI nueva solo tiene
            // que mirar `logos`.
            if preset.logos.is_empty() {
                if let Some(legacy) = preset.logo.take() {
                    preset.logos.push(legacy);
                }
            }
            out.push(preset);
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn save_preset(app: AppHandle, preset: Preset) -> Result<(), String> {
    let dir = presets_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", sanitize(&preset.name)));
    let json = serde_json::to_string_pretty(&preset).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_preset(app: AppHandle, name: String) -> Result<(), String> {
    let dir = presets_dir(&app)?;
    let path = dir.join(format!("{}.json", sanitize(&name)));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
