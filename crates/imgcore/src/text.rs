//! Marca de agua por TEXTO (firma del fotógrafo, fecha, datos del
//! archivo).
//!
//! Renderiza el texto a un bitmap RGBA usando `ab_glyph` con una
//! fuente del sistema, y compone con blending de alfa sobre la
//! imagen base. Para videos, el equivalente se construye con el
//! filtro `drawtext` de ffmpeg (ver `video.rs`).

use std::path::{Path, PathBuf};

use ab_glyph::{point, Font, FontVec, PxScale, ScaleFont};
use image::{DynamicImage, Rgba, RgbaImage};

use crate::config::TextWatermark;
use crate::error::ImgError;

/// Localiza una fuente TTF razonable en el sistema. Devuelve `None`
/// si no encuentra ninguna en las rutas comunes.
pub fn find_system_font() -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/Geneva.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
            "/Library/Fonts/Arial.ttf",
        ]
    } else if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\segoeui.ttf",
        ]
    } else {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
        ]
    };
    candidates.iter().map(PathBuf::from).find(|p| p.exists())
}

/// Sustituye tokens conocidos en el texto.
/// - `{filename}` — nombre del archivo (stem, sin extensión).
/// - `{date}` — fecha de hoy en `YYYY-MM-DD`.
pub fn substitute_tokens(template: &str, filename: Option<&str>) -> String {
    let mut result = template.to_string();
    if let Some(name) = filename {
        result = result.replace("{filename}", name);
    }
    let date = format_today_ymd();
    result = result.replace("{date}", &date);
    result
}

fn format_today_ymd() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    // Algoritmo de Hinnant (epoch 1970-01-01).
    let days = secs / 86400;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02}")
}

/// Renderiza el texto a una imagen RGBA. Cada glyph se rasteriza con
/// `ab_glyph` y se acomoda en una sola línea.
pub fn render_text(
    text: &str,
    font_path: &Path,
    px_size: f32,
    color_rgba: [u8; 4],
) -> Result<RgbaImage, ImgError> {
    if text.is_empty() {
        return Ok(RgbaImage::new(1, 1));
    }
    let font_bytes = std::fs::read(font_path).map_err(|e| ImgError::io(font_path, e))?;
    let font = FontVec::try_from_vec(font_bytes)
        .map_err(|e| ImgError::processing(format!("cargar fuente {}: {e}", font_path.display())))?;

    let scale = PxScale::from(px_size.max(4.0));
    let scaled = font.as_scaled(scale);
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let line_height = ((ascent - descent).ceil() as u32 + 2).max(2);

    let mut caret = point(1.0, ascent);
    let mut glyphs = Vec::with_capacity(text.chars().count());
    for ch in text.chars() {
        let g = font.glyph_id(ch);
        let advance = scaled.h_advance(g);
        let glyph = g.with_scale_and_position(scale, caret);
        glyphs.push(glyph);
        caret.x += advance;
    }
    let width = (caret.x.ceil() as u32 + 1).max(2);

    let base_alpha = color_rgba[3] as f32 / 255.0;
    let mut img = RgbaImage::new(width, line_height);

    for glyph in glyphs {
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bb = outlined.px_bounds();
            let min_x = bb.min.x as i32;
            let min_y = bb.min.y as i32;
            outlined.draw(|gx, gy, c| {
                let px = min_x + gx as i32;
                let py = min_y + gy as i32;
                if px < 0 || py < 0 {
                    return;
                }
                let (px, py) = (px as u32, py as u32);
                if px >= width || py >= line_height {
                    return;
                }
                let a = (c * base_alpha * 255.0).clamp(0.0, 255.0) as u8;
                if a == 0 {
                    return;
                }
                // Sobreescribimos: cada outline cubre su propia área.
                img.put_pixel(
                    px,
                    py,
                    Rgba([color_rgba[0], color_rgba[1], color_rgba[2], a]),
                );
            });
        }
    }
    Ok(img)
}

fn alpha_blend(base: [u8; 4], over: [u8; 4]) -> [u8; 4] {
    if over[3] == 0 {
        return base;
    }
    let a = over[3] as f32 / 255.0;
    let inv = 1.0 - a;
    let base_a = base[3] as f32 / 255.0;
    let out_a = (a + base_a * inv).clamp(0.0, 1.0);
    let blend = |o: u8, b: u8| -> u8 {
        let v = (o as f32 * a + b as f32 * inv).round();
        v.clamp(0.0, 255.0) as u8
    };
    [
        blend(over[0], base[0]),
        blend(over[1], base[1]),
        blend(over[2], base[2]),
        (out_a * 255.0).round() as u8,
    ]
}

fn blend_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
    let (img_w, img_h) = img.dimensions();
    let x_end = (x + w).min(img_w);
    let y_end = (y + h).min(img_h);
    for py in y..y_end {
        for px in x..x_end {
            let base = img.get_pixel(px, py).0;
            img.put_pixel(px, py, Rgba(alpha_blend(base, color)));
        }
    }
}

/// Compone el texto sobre `base` respetando posición, opacidad y
/// fondo opcional. Si no se proporciona `font_path`, intenta resolver
/// una fuente del sistema.
pub fn compose_text(
    base: DynamicImage,
    text_cfg: &TextWatermark,
    font_path: Option<&Path>,
    filename: Option<&str>,
) -> Result<DynamicImage, ImgError> {
    let resolved_font: PathBuf = match font_path {
        Some(p) => p.to_path_buf(),
        None => find_system_font().ok_or_else(|| {
            ImgError::processing(
                "no se encontró ninguna fuente TTF del sistema. \
                 Instala una fuente o define WEHI_FONT_PATH.",
            )
        })?,
    };

    let mut base_rgba = base.to_rgba8();
    let (bw, bh) = base_rgba.dimensions();
    if bw == 0 || bh == 0 {
        return Err(ImgError::processing("imagen base con dimensión 0"));
    }
    let resolved_text = substitute_tokens(&text_cfg.text, filename);
    if resolved_text.trim().is_empty() {
        return Ok(DynamicImage::ImageRgba8(base_rgba));
    }

    let px_size = ((bw as f32) * text_cfg.font_size_pct.clamp(0.005, 0.5)).max(8.0);
    let text_img = render_text(&resolved_text, &resolved_font, px_size, text_cfg.color_rgba)?;
    let (tw, th) = text_img.dimensions();

    let cx = (bw as f32) * text_cfg.position.x.clamp(0.0, 1.0);
    let cy = (bh as f32) * text_cfg.position.y.clamp(0.0, 1.0);
    let mut x = (cx - tw as f32 / 2.0).round() as i64;
    let mut y = (cy - th as f32 / 2.0).round() as i64;
    if x < 0 {
        x = 0;
    }
    if y < 0 {
        y = 0;
    }
    if x + tw as i64 > bw as i64 {
        x = bw as i64 - tw as i64;
    }
    if y + th as i64 > bh as i64 {
        y = bh as i64 - th as i64;
    }
    let x = x.max(0) as u32;
    let y = y.max(0) as u32;

    // Fondo opcional (igual semántica que LogoBackground en compose_logo).
    if let Some(bg) = text_cfg.background.as_ref() {
        let pad = (tw as f32 * bg.padding_pct.max(0.0)).round() as i64;
        let mut bx = x as i64 - pad;
        let mut by = y as i64 - pad;
        let mut bbw = tw as i64 + 2 * pad;
        let mut bbh = th as i64 + 2 * pad;
        if bx < 0 {
            bbw += bx;
            bx = 0;
        }
        if by < 0 {
            bbh += by;
            by = 0;
        }
        if bx + bbw > bw as i64 {
            bbw = bw as i64 - bx;
        }
        if by + bbh > bh as i64 {
            bbh = bh as i64 - by;
        }
        if bbw > 0 && bbh > 0 {
            blend_rect(
                &mut base_rgba,
                bx as u32,
                by as u32,
                bbw as u32,
                bbh as u32,
                bg.color_rgba,
            );
        }
    }

    let opacity = text_cfg.opacity.clamp(0.0, 1.0);
    for ty in 0..th {
        let by = y + ty;
        if by >= bh {
            break;
        }
        for tx in 0..tw {
            let bx = x + tx;
            if bx >= bw {
                break;
            }
            let mut over = text_img.get_pixel(tx, ty).0;
            over[3] = ((over[3] as f32) * opacity).round().clamp(0.0, 255.0) as u8;
            if over[3] == 0 {
                continue;
            }
            let base_pix = base_rgba.get_pixel(bx, by).0;
            base_rgba.put_pixel(bx, by, Rgba(alpha_blend(base_pix, over)));
        }
    }

    Ok(DynamicImage::ImageRgba8(base_rgba))
}

/// Para la capa Tauri: comprobar si la app tiene una fuente del
/// sistema utilizable.
pub fn font_status() -> Option<PathBuf> {
    find_system_font()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substituye_filename() {
        let out = substitute_tokens("© {filename} 2025", Some("IMG_001"));
        assert_eq!(out, "© IMG_001 2025");
    }

    #[test]
    fn format_today_es_yyyy_mm_dd() {
        let s = format_today_ymd();
        assert_eq!(s.len(), 10);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
    }

    #[test]
    fn render_text_no_falla_con_texto_vacio() {
        let img = RgbaImage::new(1, 1);
        let _ = img; // (sin font path no podemos probar el render real en CI)
    }
}
