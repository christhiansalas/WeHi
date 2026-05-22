//! Composición de la marca de agua (logo y fondo opcional) sobre la
//! imagen base.
//!
//! El logo se reescala una sola vez por archivo (no por píxel) y se
//! componente con blending de alfa contra la imagen base. Si el
//! `LogoConfig` define un `LogoBackground`, primero se pinta un
//! recuadro detrás del logo con `padding_pct` de relleno.

use std::path::{Path, PathBuf};

use fast_image_resize as fr;
use image::{DynamicImage, Rgba, RgbaImage};

use crate::config::LogoConfig;
use crate::decode::decode;
use crate::error::ImgError;

/// Logo decodificado en memoria, listo para reusar a lo largo del lote.
///
/// `path` se conserva porque el pipeline de VIDEO (vía ffmpeg) necesita
/// la ruta del archivo del logo en disco, no los píxeles decodificados.
#[derive(Debug, Clone)]
pub struct LoadedLogo {
    pub image: RgbaImage,
    pub path: PathBuf,
}

impl LoadedLogo {
    pub fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }
}

/// Decodifica un archivo de logo (PNG / JPEG / WebP) a RGBA8.
///
/// Cargar una vez por lote y reutilizar para cada imagen.
pub fn load_logo(path: &Path) -> Result<LoadedLogo, ImgError> {
    let img = decode(path)?;
    Ok(LoadedLogo {
        image: img.to_rgba8(),
        path: path.to_path_buf(),
    })
}

/// Compone el logo (y su fondo opcional) sobre `base` según `cfg`.
///
/// - Reescala el logo a `scale_pct` del ancho de `base`.
/// - Posiciona usando `cfg.position` (centro normalizado 0–1).
/// - Recorta (clamp) la caja del logo para que quede COMPLETAMENTE
///   dentro de la imagen base.
/// - Si `cfg.background` está presente, pinta el recuadro detrás del
///   logo antes de aplicarlo.
/// - Aplica `cfg.opacity` como multiplicador del canal alfa del logo.
pub fn compose_logo(
    base: DynamicImage,
    logo: &LoadedLogo,
    cfg: &LogoConfig,
) -> Result<DynamicImage, ImgError> {
    let mut base_rgba = base.to_rgba8();
    let (bw, bh) = base_rgba.dimensions();
    if bw == 0 || bh == 0 {
        return Err(ImgError::processing("imagen base con dimensión 0"));
    }

    let (logo_w, logo_h) = logo.dimensions();
    if logo_w == 0 || logo_h == 0 {
        return Err(ImgError::processing("logo con dimensión 0"));
    }

    let scale = cfg.scale_pct.clamp(0.001, 1.0);
    let target_w = ((bw as f32 * scale).round() as i64).max(1).min(bw as i64) as u32;
    let target_h = {
        let ratio = logo_h as f32 / logo_w as f32;
        ((target_w as f32 * ratio).round() as i64)
            .max(1)
            .min(bh as i64) as u32
    };

    let logo_resized = resize_logo(&logo.image, target_w, target_h)?;

    let cx = bw as f32 * cfg.position.x.clamp(0.0, 1.0);
    let cy = bh as f32 * cfg.position.y.clamp(0.0, 1.0);
    let mut x = (cx - target_w as f32 / 2.0).round() as i64;
    let mut y = (cy - target_h as f32 / 2.0).round() as i64;
    if x < 0 {
        x = 0;
    }
    if y < 0 {
        y = 0;
    }
    if x + target_w as i64 > bw as i64 {
        x = bw as i64 - target_w as i64;
    }
    if y + target_h as i64 > bh as i64 {
        y = bh as i64 - target_h as i64;
    }
    let x = x.max(0) as u32;
    let y = y.max(0) as u32;

    if let Some(bg) = cfg.background {
        let pad = (target_w as f32 * bg.padding_pct.max(0.0)).round() as i64;
        let mut bx = x as i64 - pad;
        let mut by = y as i64 - pad;
        let mut bbw = target_w as i64 + 2 * pad;
        let mut bbh = target_h as i64 + 2 * pad;
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

    composite_rgba(&mut base_rgba, &logo_resized, x, y, cfg.opacity);

    Ok(DynamicImage::ImageRgba8(base_rgba))
}

fn resize_logo(logo: &RgbaImage, w: u32, h: u32) -> Result<RgbaImage, ImgError> {
    let (orig_w, orig_h) = logo.dimensions();
    if w == orig_w && h == orig_h {
        return Ok(logo.clone());
    }
    let src = fr::images::Image::from_vec_u8(
        orig_w,
        orig_h,
        logo.clone().into_raw(),
        fr::PixelType::U8x4,
    )
    .map_err(|e| ImgError::processing(format!("fr fuente logo: {e}")))?;
    let mut dst = fr::images::Image::new(w, h, fr::PixelType::U8x4);
    let mut resizer = fr::Resizer::new();
    let opts =
        fr::ResizeOptions::new().resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3));
    resizer
        .resize(&src, &mut dst, &opts)
        .map_err(|e| ImgError::processing(format!("fr resize logo: {e}")))?;
    RgbaImage::from_raw(w, h, dst.into_vec())
        .ok_or_else(|| ImgError::processing("buffer logo redimensionado inválido"))
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

fn composite_rgba(base: &mut RgbaImage, top: &RgbaImage, x: u32, y: u32, opacity: f32) {
    let (top_w, top_h) = top.dimensions();
    let (base_w, base_h) = base.dimensions();
    let opacity = opacity.clamp(0.0, 1.0);
    for ty in 0..top_h {
        let by = y + ty;
        if by >= base_h {
            break;
        }
        for tx in 0..top_w {
            let bx = x + tx;
            if bx >= base_w {
                break;
            }
            let mut over = top.get_pixel(tx, ty).0;
            over[3] = ((over[3] as f32) * opacity).round().clamp(0.0, 255.0) as u8;
            if over[3] == 0 {
                continue;
            }
            let base_pix = base.get_pixel(bx, by).0;
            base.put_pixel(bx, by, Rgba(alpha_blend(base_pix, over)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LogoBackground, LogoPosition};

    fn imagen_base(w: u32, h: u32, color: [u8; 4]) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba(color));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    fn logo_de(w: u32, h: u32, color: [u8; 4]) -> LoadedLogo {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba(color));
            }
        }
        LoadedLogo {
            image: img,
            path: std::path::PathBuf::from(format!("test_logo_{w}x{h}.png")),
        }
    }

    #[test]
    fn alpha_blend_opaco_remplaza() {
        let base = [100, 100, 100, 255];
        let over = [200, 0, 0, 255];
        assert_eq!(alpha_blend(base, over), [200, 0, 0, 255]);
    }

    #[test]
    fn alpha_blend_transparente_no_modifica() {
        let base = [50, 60, 70, 255];
        let over = [200, 200, 200, 0];
        assert_eq!(alpha_blend(base, over), base);
    }

    #[test]
    fn compose_logo_centrado_pinta_logo_en_el_centro() {
        let base = imagen_base(100, 100, [255, 255, 255, 255]);
        let logo = logo_de(20, 20, [255, 0, 0, 255]);
        let cfg = LogoConfig {
            logo_id: "x".into(),
            position: LogoPosition { x: 0.5, y: 0.5 },
            scale_pct: 0.2,
            opacity: 1.0,
            background: None,
        };
        let salida = compose_logo(base, &logo, &cfg).unwrap().to_rgba8();
        // El centro de la imagen debe ser rojo opaco.
        let c = salida.get_pixel(50, 50).0;
        assert_eq!(c, [255, 0, 0, 255]);
        // Una esquina lejana sigue siendo blanca.
        let e = salida.get_pixel(2, 2).0;
        assert_eq!(e, [255, 255, 255, 255]);
    }

    #[test]
    fn compose_logo_clamp_si_position_empuja_fuera() {
        let base = imagen_base(100, 100, [0, 0, 0, 255]);
        let logo = logo_de(40, 40, [255, 255, 255, 255]);
        let cfg = LogoConfig {
            logo_id: "x".into(),
            position: LogoPosition { x: 1.5, y: 1.5 },
            scale_pct: 0.4,
            opacity: 1.0,
            background: None,
        };
        let salida = compose_logo(base, &logo, &cfg).unwrap().to_rgba8();
        // La esquina inferior derecha está dentro del logo
        // (40x40, anclado a la esquina por el clamp).
        let c = salida.get_pixel(99, 99).0;
        assert_eq!(c, [255, 255, 255, 255]);
        // El logo NO se sale: cualquier píxel dentro de la imagen es válido.
        assert_eq!(salida.dimensions(), (100, 100));
    }

    #[test]
    fn compose_logo_con_fondo_oscurece_alrededor() {
        let base = imagen_base(100, 100, [255, 255, 255, 255]);
        let logo = logo_de(20, 20, [255, 0, 0, 255]);
        let cfg = LogoConfig {
            logo_id: "x".into(),
            position: LogoPosition { x: 0.5, y: 0.5 },
            scale_pct: 0.2,
            opacity: 1.0,
            background: Some(LogoBackground {
                color_rgba: [0, 0, 0, 255],
                padding_pct: 0.5,
            }),
        };
        let salida = compose_logo(base, &logo, &cfg).unwrap().to_rgba8();
        // Justo afuera del logo pero dentro del fondo: debe ser negro.
        // Logo 20x20 centrado en (50,50): cubre x in 40..60, y in 40..60.
        // Padding 0.5 * 20 = 10 px: el fondo cubre 30..70.
        let pix_fondo = salida.get_pixel(35, 50).0;
        assert_eq!(pix_fondo[..3], [0, 0, 0][..]);
        let pix_logo = salida.get_pixel(50, 50).0;
        assert_eq!(pix_logo[..3], [255, 0, 0][..]);
    }
}
