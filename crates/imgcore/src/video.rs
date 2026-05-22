//! Procesamiento de video vía `ffmpeg`.
//!
//! El watermark se compone sobre los frames del video con el filtro
//! `overlay` de ffmpeg. El audio se copia sin re-encodear. El video
//! sale en MP4 (H.264). En macOS usa `h264_videotoolbox` (HW); en el
//! resto, `libx264`.
//!
//! `ffmpeg` debe estar instalado en el sistema. Esta capa lo busca
//! en:
//! 1. La variable de entorno `WEHI_FFMPEG_PATH`.
//! 2. Rutas comunes: `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`.
//! 3. El `PATH` del usuario (vía `which`).

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use image::DynamicImage;

use crate::config::{LogoBackground, LogoConfig, ResizeConfig, ResizeMode};
use crate::error::ImgError;

/// Extensiones consideradas video.
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v", "webm", "mkv", "avi"];

/// `true` si la extensión del path corresponde a un video.
pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.iter().any(|v| v.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Localiza el binario de `ffmpeg`.
pub fn find_ffmpeg() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("WEHI_FFMPEG_PATH") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }
    for absolute in [
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
        "/opt/local/bin/ffmpeg",
        "C:\\ffmpeg\\bin\\ffmpeg.exe",
        "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
    ] {
        let p = PathBuf::from(absolute);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(out) = Command::new("which").arg("ffmpeg").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    if let Ok(out) = Command::new("where").arg("ffmpeg").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}

fn require_ffmpeg() -> Result<PathBuf, ImgError> {
    find_ffmpeg().ok_or_else(|| {
        ImgError::processing(
            "no se encontró ffmpeg. Instálalo: 'brew install ffmpeg' (macOS), \
             'winget install ffmpeg' (Windows) o 'apt install ffmpeg' (Linux). \
             Alternativamente, define WEHI_FFMPEG_PATH apuntando al binario.",
        )
    })
}

/// Duración del video en microsegundos (de format=duration via ffprobe).
/// Devuelve 0 si no se pudo determinar — el callback de progreso
/// reportará `total_us = 0` y la UI lo manejará como "indeterminado".
pub fn probe_duration_us(input: &Path) -> u64 {
    let Some(ffmpeg) = find_ffmpeg() else {
        return 0;
    };
    let ffprobe = ffmpeg.with_file_name(if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    });
    let tool = if ffprobe.exists() {
        ffprobe
    } else {
        PathBuf::from(if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        })
    };
    let output = Command::new(&tool)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(input)
        .output();
    let Ok(output) = output else {
        return 0;
    };
    if !output.status.success() {
        return 0;
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    raw.trim()
        .parse::<f64>()
        .ok()
        .map(|s| (s * 1_000_000.0) as u64)
        .unwrap_or(0)
}

/// Dimensiones (width, height) del primer stream de video del archivo.
pub fn probe_dimensions(input: &Path) -> Result<(u32, u32), ImgError> {
    let ffmpeg = require_ffmpeg()?;
    // ffprobe suele estar junto a ffmpeg
    let ffprobe = ffmpeg.with_file_name(if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    });
    let tool = if ffprobe.exists() {
        ffprobe
    } else {
        PathBuf::from(if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        })
    };

    let output = Command::new(&tool)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=s=x:p=0",
        ])
        .arg(input)
        .output()
        .map_err(|e| ImgError::processing(format!("ejecutar ffprobe: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ImgError::processing(format!(
            "ffprobe falló para {}: {stderr}",
            input.display()
        )));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let trimmed = raw.trim();
    let mut parts = trimmed.split('x');
    let w: u32 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| {
            ImgError::processing(format!("ffprobe sin width para {}", input.display()))
        })?;
    let h: u32 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| {
            ImgError::processing(format!("ffprobe sin height para {}", input.display()))
        })?;
    Ok((w, h))
}

/// Extrae un frame del video como imagen decodificada en memoria.
/// Útil para previsualización.
pub fn extract_frame(input: &Path, time_secs: f32) -> Result<DynamicImage, ImgError> {
    let ffmpeg = require_ffmpeg()?;
    let t = time_secs.max(0.0);
    let output = Command::new(&ffmpeg)
        .args(["-y", "-hide_banner", "-loglevel", "error", "-ss"])
        .arg(format!("{t}"))
        .arg("-i")
        .arg(input)
        .args([
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .map_err(|e| ImgError::processing(format!("ejecutar ffmpeg: {e}")))?;

    if !output.status.success() || output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ImgError::processing(format!(
            "extraer frame de {}: {stderr}",
            input.display()
        )));
    }

    image::load_from_memory(&output.stdout).map_err(|e| ImgError::decode(input, e.to_string()))
}

/// Calcula las dimensiones de destino para un video aplicando el
/// mismo modo de resize que usamos en imágenes. Devuelve siempre
/// valores pares (h264 lo requiere).
pub fn compute_video_target(orig_w: u32, orig_h: u32, cfg: &ResizeConfig) -> (u32, u32) {
    let (mut tw, mut th) = match cfg.mode {
        ResizeMode::MaxDimension => {
            let max_w = cfg.width.unwrap_or(orig_w).max(1) as f32;
            let max_h = cfg.height.unwrap_or(orig_h).max(1) as f32;
            let scale = (max_w / orig_w as f32).min(max_h / orig_h as f32);
            (
                (orig_w as f32 * scale).round().max(1.0) as u32,
                (orig_h as f32 * scale).round().max(1.0) as u32,
            )
        }
        ResizeMode::Exact => {
            let w = cfg.width.unwrap_or(orig_w).max(1);
            let h = cfg.height.unwrap_or(orig_h).max(1);
            if cfg.keep_aspect_ratio {
                let scale = (w as f32 / orig_w as f32).min(h as f32 / orig_h as f32);
                (
                    (orig_w as f32 * scale).round().max(1.0) as u32,
                    (orig_h as f32 * scale).round().max(1.0) as u32,
                )
            } else {
                (w, h)
            }
        }
        ResizeMode::Percentage => {
            let p = cfg.percentage.unwrap_or(1.0).max(0.0);
            (
                (orig_w as f32 * p).round().max(1.0) as u32,
                (orig_h as f32 * p).round().max(1.0) as u32,
            )
        }
    };
    if !cfg.allow_upscale && (tw > orig_w || th > orig_h) {
        let scale = (orig_w as f32 / tw as f32).min(orig_h as f32 / th as f32);
        tw = ((tw as f32 * scale).round() as i64).max(1) as u32;
        th = ((th as f32 * scale).round() as i64).max(1) as u32;
    }
    // H264 requiere dimensiones pares.
    (tw & !1, th & !1)
}

/// Procesa un video aplicando el watermark del preset y escribe el
/// resultado a `out_path`. Sin callback de progreso ni cancelación.
///
/// `logo_path` aquí es por compatibilidad; conecta a la primera
/// marca del preset si existe.
pub fn process_video(
    input: &Path,
    out_path: &Path,
    preset: &crate::config::Preset,
    logo_path: Option<&Path>,
) -> Result<(), ImgError> {
    let logos: Vec<(&LogoConfig, &Path)> = if let Some(p) = logo_path {
        preset
            .all_logos()
            .next()
            .map(|cfg| vec![(cfg, p)])
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    process_video_with_progress(input, out_path, preset, &logos, None, |_, _| {})
}

/// Procesa un video aplicando el watermark del preset.
///
/// - `on_progress(current_us, total_us)` se invoca con el avance
///   parseado de `ffmpeg -progress pipe:1`. Si la duración total no
///   se pudo determinar, `total_us` viene en 0.
/// - `cancel` (opcional): si la `AtomicBool` se pone a `true` durante
///   el encoding, se mata el proceso ffmpeg, se borra el archivo de
///   salida parcial y se devuelve [`ImgError::Processing`] con
///   mensaje "cancelado".
pub fn process_video_with_progress<F>(
    input: &Path,
    out_path: &Path,
    preset: &crate::config::Preset,
    logos: &[(&LogoConfig, &Path)],
    cancel: Option<Arc<AtomicBool>>,
    mut on_progress: F,
) -> Result<(), ImgError>
where
    F: FnMut(u64, u64),
{
    let ffmpeg = require_ffmpeg()?;

    let total_us = probe_duration_us(input);
    let (orig_w, orig_h) = probe_dimensions(input)?;
    let (target_w, target_h) = compute_video_target(orig_w, orig_h, &preset.resize);

    let mut filters: Vec<String> = Vec::new();
    let needs_scale = (target_w, target_h) != (orig_w & !1, orig_h & !1);
    if needs_scale {
        filters.push(format!(
            "[0:v]scale={target_w}:{target_h}:flags=lanczos[vmain]"
        ));
    }

    // Etiqueta de la imagen base que se va actualizando a medida que
    // encadenamos ajustes + drawbox + overlay por cada logo.
    let mut current_main: String = if needs_scale {
        "[vmain]".to_string()
    } else {
        "[0:v]".to_string()
    };

    // Ajustes de imagen (brillo/contraste/saturación/temperatura)
    // aplicados antes de las marcas de agua.
    if let Some(adj_filter) = crate::adjust::build_ffmpeg_filter(&preset.adjustments) {
        let next = "[vadj]".to_string();
        filters.push(format!("{current_main}{adj_filter}{next}"));
        current_main = next;
    }

    // Para cada logo: opcionalmente drawbox (fondo) → scale+opacity →
    // overlay sobre la base actual. Cada paso renombra la etiqueta de
    // salida para que el siguiente lo use como entrada.
    for (idx, (logo_cfg, logo_path)) in logos.iter().enumerate() {
        let logo_input_idx = idx + 1; // [0:v] = video, [1:v] = primer logo, …

        let (logo_orig_w, logo_orig_h) = image::image_dimensions(logo_path)
            .map_err(|e| ImgError::processing(format!("logo dims {}: {e}", logo_path.display())))?;
        let logo_target_w = ((target_w as f32) * logo_cfg.scale_pct.clamp(0.001, 1.0))
            .round()
            .max(2.0) as u32;
        let logo_target_h = ((logo_target_w as f32) * (logo_orig_h as f32) / (logo_orig_w as f32))
            .round()
            .max(2.0) as u32;

        // (a) Fondo opcional con drawbox.
        if let Some(bg) = logo_cfg.background.as_ref() {
            let bg_filter = build_background_drawbox(bg, logo_cfg, logo_target_w, logo_target_h);
            let next_label = format!("[vbg{idx}]");
            filters.push(format!("{current_main}{bg_filter}{next_label}"));
            current_main = next_label;
        }

        // (b) Escalado + opacidad del logo.
        let wm_label = format!("[wm{idx}]");
        filters.push(format!(
            "[{logo_input_idx}:v]scale={logo_target_w}:-1,format=rgba,colorchannelmixer=aa={opacity}{wm_label}",
            opacity = logo_cfg.opacity.clamp(0.0, 1.0)
        ));

        // (c) Overlay sobre la base actual.
        let next_main = format!("[v{idx}]");
        let overlay = build_overlay_filter(logo_cfg);
        filters.push(format!("{current_main}{wm_label}{overlay}{next_main}"));
        current_main = next_main;
    }

    // Marcas de agua por TEXTO con drawtext. Cada una es un paso más
    // en la cadena. Necesita una fuente TTF del sistema.
    if !preset.text_watermarks.is_empty() {
        let font_path = crate::text::find_system_font().ok_or_else(|| {
            ImgError::processing("marcas de agua de texto: no se encontró fuente TTF en el sistema")
        })?;
        let font_str = font_path.to_string_lossy().replace('\\', "/");
        for (i, tw_cfg) in preset.text_watermarks.iter().enumerate() {
            let next_main = format!("[tv{i}]");
            let drawtext = build_drawtext_filter(tw_cfg, target_w, &font_str);
            filters.push(format!("{current_main}{drawtext}{next_main}"));
            current_main = next_main;
        }
    }

    let final_label = current_main;

    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y")
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(input);
    // Un `-i` por cada logo.
    for (_, logo_path) in logos {
        cmd.arg("-i").arg(logo_path);
    }
    if !filters.is_empty() {
        cmd.arg("-filter_complex").arg(filters.join(";"));
        cmd.arg("-map").arg(&final_label);
    } else {
        cmd.arg("-map").arg("0:v");
    }
    cmd.arg("-map").arg("0:a?");

    if cfg!(target_os = "macos") {
        cmd.arg("-c:v")
            .arg("h264_videotoolbox")
            .arg("-b:v")
            .arg("8M");
    } else {
        cmd.arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("fast")
            .arg("-crf")
            .arg("22");
    }

    cmd.arg("-c:a")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        // Reporta progreso máquina-parseable a stdout, una línea por
        // métrica, bloques delimitados por `progress=continue|end`.
        .arg("-progress")
        .arg("pipe:1")
        .arg(out_path);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| ImgError::processing(format!("ejecutar ffmpeg: {e}")))?;

    let mut cancelled = false;
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(flag) = cancel.as_ref() {
                if flag.load(Ordering::SeqCst) {
                    let _ = child.kill();
                    cancelled = true;
                    break;
                }
            }
            if let Some(value) = line.strip_prefix("out_time_us=") {
                if let Ok(us) = value.trim().parse::<u64>() {
                    on_progress(us, total_us);
                }
            } else if line.trim() == "progress=end" {
                on_progress(total_us, total_us);
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| ImgError::processing(format!("esperar ffmpeg: {e}")))?;

    if cancelled {
        // Limpia el archivo parcial que ffmpeg pudo haber escrito.
        let _ = std::fs::remove_file(out_path);
        return Err(ImgError::processing(format!(
            "cancelado: {}",
            input.display()
        )));
    }

    if !status.success() {
        let mut stderr_buf = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            use std::io::Read;
            let _ = stderr.read_to_string(&mut stderr_buf);
        }
        return Err(ImgError::processing(format!(
            "ffmpeg falló procesando {}: {stderr_buf}",
            input.display()
        )));
    }

    Ok(())
}

#[cfg(test)]
fn build_logo_filter(target_w: u32, logo_cfg: &LogoConfig) -> String {
    // Solo lo usan los tests; en producción el filtro se construye
    // inline por cada logo con su índice de input.
    let scale_pct = logo_cfg.scale_pct.clamp(0.001, 1.0);
    let logo_w = ((target_w as f32) * scale_pct).round().max(2.0) as u32;
    let opacity = logo_cfg.opacity.clamp(0.0, 1.0);
    format!("[1:v]scale={logo_w}:-1,format=rgba,colorchannelmixer=aa={opacity}[wm]")
}

fn build_overlay_filter(logo_cfg: &LogoConfig) -> String {
    let cx = logo_cfg.position.x.clamp(0.0, 1.0);
    let cy = logo_cfg.position.y.clamp(0.0, 1.0);
    // Posiciona el CENTRO del logo en (W*cx, H*cy) y recorta para que
    // no se salga de la imagen. El backslash escapa la coma dentro de
    // la cadena de filtro de ffmpeg.
    format!("overlay=x='max(0\\,min(W*{cx}-w/2\\,W-w))':y='max(0\\,min(H*{cy}-h/2\\,H-h))'")
}

/// Construye un filtro `drawtext` de ffmpeg a partir de una
/// `TextWatermark`. La fuente se pasa como ruta absoluta. El texto
/// se escapa para los caracteres reservados (`:`, `'`, `\`, `%`).
fn build_drawtext_filter(
    tw_cfg: &crate::config::TextWatermark,
    target_w: u32,
    font_path: &str,
) -> String {
    let text = escape_drawtext(&tw_cfg.text);
    let font_size = ((target_w as f32) * tw_cfg.font_size_pct.clamp(0.005, 0.5))
        .max(8.0)
        .round() as u32;
    let [r, g, b, a] = tw_cfg.color_rgba;
    let color_alpha = (a as f32 / 255.0) * tw_cfg.opacity.clamp(0.0, 1.0);
    let color = format!("0x{r:02X}{g:02X}{b:02X}@{color_alpha:.3}");
    let cx = tw_cfg.position.x.clamp(0.0, 1.0);
    let cy = tw_cfg.position.y.clamp(0.0, 1.0);
    let mut s = format!(
        "drawtext=fontfile='{font_path}':text='{text}':fontcolor={color}:fontsize={font_size}\
         :x='max(0\\,min(W*{cx}-text_w/2\\,W-text_w))'\
         :y='max(0\\,min(H*{cy}-text_h/2\\,H-text_h))'"
    );
    if let Some(bg) = tw_cfg.background.as_ref() {
        let [br, bg_, bb, ba] = bg.color_rgba;
        let bg_alpha = ba as f32 / 255.0;
        let pad = ((font_size as f32) * bg.padding_pct.max(0.0)).round() as u32;
        s.push_str(&format!(
            ":box=1:boxcolor=0x{br:02X}{bg_:02X}{bb:02X}@{bg_alpha:.3}:boxborderw={pad}"
        ));
    }
    s
}

/// Escapa caracteres reservados de drawtext para usar `text=` literal.
fn escape_drawtext(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\\\\\'"),
            ':' => out.push_str("\\:"),
            '%' => out.push_str("\\%"),
            ',' => out.push_str("\\,"),
            _ => out.push(c),
        }
    }
    out
}

/// Construye el `drawbox` que pinta el recuadro detrás del logo. El
/// tamaño se calcula a partir de las dimensiones reales del logo en
/// disco (probadas antes) más `padding_pct` del ancho del logo.
fn build_background_drawbox(
    bg: &LogoBackground,
    logo_cfg: &LogoConfig,
    logo_target_w: u32,
    logo_target_h: u32,
) -> String {
    let pad = ((logo_target_w as f32) * bg.padding_pct.max(0.0)).round() as u32;
    let bg_w = logo_target_w + 2 * pad;
    let bg_h = logo_target_h + 2 * pad;
    let [r, g, b, a] = bg.color_rgba;
    let alpha = (a as f32 / 255.0).clamp(0.0, 1.0);
    let cx = logo_cfg.position.x.clamp(0.0, 1.0);
    let cy = logo_cfg.position.y.clamp(0.0, 1.0);
    // Posición del fondo: centra alrededor del centro del logo y
    // clampea para que no se salga de la imagen.
    let bg_x = format!("max(0\\,min(W*{cx}-{}/2\\,W-{bg_w}))", logo_target_w);
    let bg_y = format!("max(0\\,min(H*{cy}-{}/2\\,H-{bg_h}))", logo_target_h);
    format!(
        "drawbox=x='{bg_x}':y='{bg_y}':w={bg_w}:h={bg_h}:color=0x{r:02X}{g:02X}{b:02X}@{alpha}:t=fill"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detecta_extensiones_de_video_sin_case() {
        assert!(is_video(&PathBuf::from("clip.MP4")));
        assert!(is_video(&PathBuf::from("a/b.mov")));
        assert!(is_video(&PathBuf::from("x.webm")));
        assert!(!is_video(&PathBuf::from("foto.jpg")));
        assert!(!is_video(&PathBuf::from("sin_extension")));
    }

    #[test]
    fn build_logo_filter_usa_porcentaje_del_ancho() {
        let cfg = LogoConfig {
            logo_id: "x".into(),
            position: crate::config::LogoPosition { x: 0.5, y: 0.5 },
            scale_pct: 0.25,
            opacity: 0.8,
            background: None,
        };
        let f = build_logo_filter(1920, &cfg);
        assert!(f.contains("scale=480:-1"));
        assert!(f.contains("aa=0.8"));
    }

    #[test]
    fn build_overlay_filter_clampea_dentro_de_la_imagen() {
        let cfg = LogoConfig {
            logo_id: "x".into(),
            position: crate::config::LogoPosition { x: 0.95, y: 0.95 },
            scale_pct: 0.2,
            opacity: 1.0,
            background: None,
        };
        let f = build_overlay_filter(&cfg);
        assert!(f.contains("max(0"));
        assert!(f.contains("W-w"));
        assert!(f.contains("H-h"));
    }

    #[test]
    fn compute_video_target_respeta_no_upscale_y_devuelve_pares() {
        let cfg = ResizeConfig {
            mode: ResizeMode::Exact,
            width: Some(3000),
            height: Some(3000),
            percentage: None,
            keep_aspect_ratio: true,
            allow_upscale: false,
        };
        let (w, h) = compute_video_target(1920, 1080, &cfg);
        assert!(w <= 1920);
        assert!(h <= 1080);
        assert_eq!(w % 2, 0);
        assert_eq!(h % 2, 0);
    }

    #[test]
    fn compute_video_target_percentage() {
        let cfg = ResizeConfig {
            mode: ResizeMode::Percentage,
            width: None,
            height: None,
            percentage: Some(0.5),
            keep_aspect_ratio: true,
            allow_upscale: false,
        };
        let (w, h) = compute_video_target(1920, 1080, &cfg);
        assert_eq!(w, 960);
        assert_eq!(h, 540);
    }
}
