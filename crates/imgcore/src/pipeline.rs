//! Pipeline completo: decodificar → redimensionar → componer logo
//! (opcional) → codificar. Hay dos puntos de entrada:
//!
//! - [`process_image`]: devuelve los bytes codificados, sin escribir
//!   a disco. La previsualización lo usa.
//! - [`process_file`]: llama a `process_image`, resuelve la ruta de
//!   salida (patrón + `preserve_structure`) y escribe el archivo. El
//!   lote masivo lo usa.
//!
//! Separar ambos garantiza que la previsualización y el lote pasan
//! por el MISMO motor: lo que se ve es lo que se obtiene.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::DynamicImage;

use crate::adjust;
use crate::config::{OutputFormat, Preset, ResizeConfig, ResizeMode};
use crate::decode::decode;
use crate::encode::encode_image;
use crate::error::ImgError;
use crate::overlay::{compose_logo, LoadedLogo};
use crate::resize::resize_image;
use crate::text;
use crate::video;

/// Mapa `logo_id → LoadedLogo` cargado UNA vez por lote y reutilizado
/// para cada archivo. Es el contenedor canónico que pasa a través del
/// pipeline cuando un preset tiene una o más marcas de agua.
pub type LogoMap = HashMap<String, LoadedLogo>;

/// Imagen ya codificada lista para escribirse a disco o enviarse al
/// frontend (en el comando de previsualización).
#[derive(Debug, Clone)]
pub struct EncodedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: OutputFormat,
}

/// Procesa una imagen y devuelve los bytes codificados.
///
/// El `logo` debe pasarse precargado (`load_logo` una sola vez por
/// lote) si el preset incluye `LogoConfig`. Si el preset declara un
/// logo pero no se proporciona, se devuelve [`ImgError::LogoNotFound`].
pub fn process_image(
    input: &Path,
    preset: &Preset,
    logos: &LogoMap,
) -> Result<EncodedImage, ImgError> {
    // Para previsualizar un video, extraemos un frame y lo pasamos por
    // el mismo pipeline de imagen. Para el lote real, `process_file`
    // despacha a `video::process_video` directamente.
    let decoded = if video::is_video(input) {
        video::extract_frame(input, 0.5)?
    } else {
        decode(input)?
    };
    encode_pipeline(decoded, preset, logos)
}

/// Pipeline a partir de una imagen YA decodificada. Útil cuando se
/// quiere reusar una decodificación cacheada (previsualización).
///
/// Compone TODAS las marcas declaradas en el preset en orden
/// (`preset.all_logos()`). Si un `logo_id` no está en el mapa,
/// devuelve [`ImgError::LogoNotFound`].
pub fn encode_pipeline(
    img: image::DynamicImage,
    preset: &Preset,
    logos: &LogoMap,
) -> Result<EncodedImage, ImgError> {
    encode_pipeline_named(img, preset, logos, None)
}

/// Igual que [`encode_pipeline`] pero con el nombre de archivo
/// disponible para sustituir tokens en marcas de agua de texto.
pub fn encode_pipeline_named(
    img: image::DynamicImage,
    preset: &Preset,
    logos: &LogoMap,
    filename: Option<&str>,
) -> Result<EncodedImage, ImgError> {
    let mut composed = resize_image(img, &preset.resize)?;
    // Ajustes de imagen (brillo/contraste/saturación/temperatura) se
    // aplican aquí, después del resize y ANTES de las marcas de agua
    // para que las firmas/logos no se vean afectados por la corrección.
    composed = adjust::apply_adjustments(composed, &preset.adjustments);
    for logo_cfg in preset.all_logos() {
        let logo_obj = logos
            .get(&logo_cfg.logo_id)
            .ok_or_else(|| ImgError::logo_not_found(&logo_cfg.logo_id))?;
        composed = compose_logo(composed, logo_obj, logo_cfg)?;
    }
    // Texto se compone DESPUÉS de los logos para que quede encima.
    for text_cfg in &preset.text_watermarks {
        composed = text::compose_text(composed, text_cfg, None, filename)?;
    }
    let width = composed.width();
    let height = composed.height();
    let bytes = encode_image(&composed, &preset.output)?;
    Ok(EncodedImage {
        bytes,
        width,
        height,
        format: preset.output.format,
    })
}

/// Procesa una imagen y la escribe al directorio configurado en el
/// preset, respetando el patrón de nombre y `preserve_structure`.
/// Recorta `img` centrado al aspect ratio `target_w/target_h` y luego
/// la reescala a esas dimensiones exactas. Usado por
/// `Preset.network_crops` para producir variantes por red social.
pub fn center_crop_resize(
    img: &DynamicImage,
    target_w: u32,
    target_h: u32,
) -> Result<DynamicImage, ImgError> {
    if target_w == 0 || target_h == 0 {
        return Err(ImgError::processing("dimensiones de recorte inválidas"));
    }
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return Err(ImgError::processing("imagen base con dimensión 0"));
    }
    let target_aspect = (target_w as f32) / (target_h as f32);
    let img_aspect = (w as f32) / (h as f32);

    let (crop_w, crop_h) = if img_aspect > target_aspect {
        // Imagen más ancha que el target → recortamos los lados.
        let cw = ((h as f32) * target_aspect).round() as u32;
        (cw.min(w).max(1), h)
    } else {
        // Imagen más alta que el target → recortamos arriba/abajo.
        let ch = ((w as f32) / target_aspect).round() as u32;
        (w, ch.min(h).max(1))
    };

    let crop_x = (w - crop_w) / 2;
    let crop_y = (h - crop_h) / 2;
    let cropped = img.crop_imm(crop_x, crop_y, crop_w, crop_h);

    resize_image(
        cropped,
        &ResizeConfig {
            mode: ResizeMode::Exact,
            width: Some(target_w),
            height: Some(target_h),
            percentage: None,
            keep_aspect_ratio: false,
            // Si el recorte es más pequeño que el target, dejamos que
            // suba: el output debe coincidir EXACTO con las dimensiones
            // pedidas por la red social.
            allow_upscale: true,
        },
    )
}

pub fn process_file(input: &Path, preset: &Preset, logos: &LogoMap) -> Result<(), ImgError> {
    process_file_with_progress(input, preset, logos, None, |_, _| {})
}

/// Igual que [`process_file`] pero:
/// - Invoca `on_video_progress` cuando ffmpeg reporta avance (solo
///   aplica a archivos de video).
/// - Si `cancel` es `Some` y su flag se pone a `true`, mata el
///   proceso ffmpeg y devuelve error de cancelación.
pub fn process_file_with_progress<F>(
    input: &Path,
    preset: &Preset,
    logos: &LogoMap,
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    on_video_progress: F,
) -> Result<(), ImgError>
where
    F: FnMut(u64, u64),
{
    let out_path = resolve_output_path(input, preset);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ImgError::io(parent, e))?;
    }
    if video::is_video(input) {
        // Recolecta pares (LogoConfig, &Path) en el orden de composición.
        let video_logos: Vec<(&crate::config::LogoConfig, &Path)> = preset
            .all_logos()
            .map(|cfg| {
                let lo = logos
                    .get(&cfg.logo_id)
                    .ok_or_else(|| ImgError::logo_not_found(&cfg.logo_id))?;
                Ok::<_, ImgError>((cfg, lo.path.as_path()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        video::process_video_with_progress(
            input,
            &out_path,
            preset,
            &video_logos,
            cancel,
            on_video_progress,
        )?;
        // Los `network_crops` aplican solo a imágenes en v1.
    } else if preset.network_crops.is_empty() {
        // Camino simple: solo el output principal.
        let filename = input
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        let decoded = decode(input)?;
        let encoded = encode_pipeline_named(decoded, preset, logos, filename.as_deref())?;
        std::fs::write(&out_path, &encoded.bytes).map_err(|e| ImgError::io(&out_path, e))?;
    } else {
        // Camino con recortes: decodifica + redimensiona + watermark
        // UNA vez, luego genera el principal y un archivo por cada
        // recorte.
        let filename = input
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        let decoded = decode(input)?;
        let mut composed = resize_image(decoded, &preset.resize)?;
        composed = adjust::apply_adjustments(composed, &preset.adjustments);
        for logo_cfg in preset.all_logos() {
            let logo_obj = logos
                .get(&logo_cfg.logo_id)
                .ok_or_else(|| ImgError::logo_not_found(&logo_cfg.logo_id))?;
            composed = compose_logo(composed, logo_obj, logo_cfg)?;
        }
        for text_cfg in &preset.text_watermarks {
            composed = text::compose_text(composed, text_cfg, None, filename.as_deref())?;
        }

        let main_bytes = encode_image(&composed, &preset.output)?;
        std::fs::write(&out_path, &main_bytes).map_err(|e| ImgError::io(&out_path, e))?;

        for crop in &preset.network_crops {
            let cropped = center_crop_resize(&composed, crop.width, crop.height)?;
            let bytes = encode_image(&cropped, &preset.output)?;
            let crop_path = resolve_crop_output_path(input, preset, &crop.id);
            if let Some(parent) = crop_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ImgError::io(parent, e))?;
            }
            std::fs::write(&crop_path, &bytes).map_err(|e| ImgError::io(&crop_path, e))?;
        }
    }
    Ok(())
}

fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn resolve_crop_output_path(input: &Path, preset: &Preset, crop_id: &str) -> PathBuf {
    let ext = match preset.output.format {
        OutputFormat::Jpeg => ".jpg",
        OutputFormat::Png => ".png",
        OutputFormat::WebP => ".webp",
    };
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imagen");
    let safe_id = sanitize_id(crop_id);
    let combined = format!("{stem}__{safe_id}");
    let filename = preset
        .output
        .filename_pattern
        .replace("{name}", &combined)
        .replace("{ext}", ext);
    let mut out = preset.output.folder.clone();
    if preset.output.preserve_structure {
        if let Some(parent_name) = input.parent().and_then(|p| p.file_name()) {
            out.push(parent_name);
        }
    }
    out.push(filename);
    out
}

fn resolve_output_path(input: &Path, preset: &Preset) -> PathBuf {
    // Para video, el output siempre es MP4. Para imagen, el formato
    // del preset.
    let ext = if video::is_video(input) {
        ".mp4"
    } else {
        match preset.output.format {
            OutputFormat::Jpeg => ".jpg",
            OutputFormat::Png => ".png",
            OutputFormat::WebP => ".webp",
        }
    };
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imagen");
    let filename = preset
        .output
        .filename_pattern
        .replace("{name}", stem)
        .replace("{ext}", ext);
    let mut out = preset.output.folder.clone();
    if preset.output.preserve_structure {
        if let Some(parent_name) = input.parent().and_then(|p| p.file_name()) {
            out.push(parent_name);
        }
    }
    out.push(filename);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        LogoBackground, LogoConfig, LogoPosition, OutputConfig, ResizeConfig, ResizeMode,
    };
    use crate::overlay::load_logo;
    use image::{Rgba, RgbaImage};

    fn dir_temporal(nombre: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wehi_pipeline_{}_{}", std::process::id(), nombre));
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

    fn preset_completo(folder: &Path) -> Preset {
        Preset {
            name: "test".into(),
            resize: ResizeConfig {
                mode: ResizeMode::Exact,
                width: Some(120),
                height: Some(120),
                percentage: None,
                keep_aspect_ratio: false,
                allow_upscale: false,
            },
            logo: Some(LogoConfig {
                logo_id: "logo_principal".into(),
                position: LogoPosition { x: 0.5, y: 0.5 },
                scale_pct: 0.25,
                opacity: 0.85,
                background: Some(LogoBackground {
                    color_rgba: [0, 0, 0, 180],
                    padding_pct: 0.1,
                }),
            }),
            output: OutputConfig {
                format: OutputFormat::Jpeg,
                quality: 80,
                folder: folder.to_path_buf(),
                filename_pattern: "{name}_wehi{ext}".into(),
                preserve_structure: false,
            },
            logos: Vec::new(),
            network_crops: Vec::new(),
            text_watermarks: Vec::new(),
            adjustments: crate::adjust::ImageAdjustments::default(),
        }
    }

    fn one_logo_map(logo: LoadedLogo, id: &str) -> LogoMap {
        let mut m = LogoMap::new();
        m.insert(id.to_string(), logo);
        m
    }

    #[test]
    fn process_image_aplica_logo_y_fondo_y_devuelve_bytes_validos() {
        let dir = dir_temporal("integracion");
        let input_path = dir.join("entrada.png");
        let logo_path = dir.join("logo.png");
        guardar_png(&input_path, 240, 240, [100, 150, 200, 255]);
        guardar_png(&logo_path, 40, 40, [255, 0, 0, 220]);
        let logo = load_logo(&logo_path).unwrap();

        let preset = preset_completo(&dir);
        let logos = one_logo_map(logo, "logo_principal");
        let encoded = process_image(&input_path, &preset, &logos).unwrap();

        assert_eq!(encoded.format, OutputFormat::Jpeg);
        assert_eq!(encoded.width, 120);
        assert_eq!(encoded.height, 120);
        let decoded = image::load_from_memory(&encoded.bytes).unwrap();
        assert_eq!(decoded.width(), 120);
        assert_eq!(decoded.height(), 120);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn process_image_sin_logo_pasa_solo_por_resize_y_encode() {
        let dir = dir_temporal("sin_logo");
        let input_path = dir.join("entrada.png");
        guardar_png(&input_path, 300, 300, [10, 20, 30, 255]);

        let preset = Preset {
            logo: None,
            logos: Vec::new(),
            ..preset_completo(&dir)
        };
        let empty = LogoMap::new();
        let encoded = process_image(&input_path, &preset, &empty).unwrap();
        assert!(!encoded.bytes.is_empty());
        let decoded = image::load_from_memory(&encoded.bytes).unwrap();
        assert_eq!(decoded.width(), 120);
        assert_eq!(decoded.height(), 120);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn process_image_con_preset_logo_pero_sin_logo_cargado_devuelve_error() {
        let dir = dir_temporal("err_logo");
        let input_path = dir.join("entrada.png");
        guardar_png(&input_path, 50, 50, [0, 0, 0, 255]);

        let preset = preset_completo(&dir);
        let empty = LogoMap::new();
        let err = process_image(&input_path, &preset, &empty).unwrap_err();
        assert!(matches!(err, ImgError::LogoNotFound { .. }));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn process_file_escribe_a_disco_con_patron_de_nombre() {
        let dir = dir_temporal("file");
        let input_path = dir.join("foto.png");
        let out_dir = dir.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        guardar_png(&input_path, 80, 80, [50, 50, 50, 255]);

        let preset = Preset {
            logo: None,
            logos: Vec::new(),
            output: OutputConfig {
                format: OutputFormat::Png,
                quality: 100,
                folder: out_dir.clone(),
                filename_pattern: "{name}_wehi{ext}".into(),
                preserve_structure: false,
            },
            ..preset_completo(&dir)
        };
        let empty = LogoMap::new();
        process_file(&input_path, &preset, &empty).unwrap();
        let esperado = out_dir.join("foto_wehi.png");
        assert!(esperado.exists(), "no se escribió {:?}", esperado);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
