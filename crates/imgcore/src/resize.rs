//! Redimensionado de imágenes con `fast_image_resize` (Lanczos3).
//!
//! Implementa los tres modos de [`ResizeMode`] y respeta `allow_upscale`:
//! si está apagado, nunca devuelve una imagen más grande que la original.

use fast_image_resize as fr;
use image::{DynamicImage, GenericImageView, RgbaImage};

use crate::config::{ResizeConfig, ResizeMode};
use crate::error::ImgError;

/// Redimensiona una imagen según una [`ResizeConfig`]. Si el resultado
/// de los cálculos coincide con el tamaño original, devuelve la imagen
/// sin reescalar.
pub fn resize_image(img: DynamicImage, cfg: &ResizeConfig) -> Result<DynamicImage, ImgError> {
    let (orig_w, orig_h) = img.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return Err(ImgError::processing("imagen con dimensión 0"));
    }

    let (target_w, target_h) = compute_target(orig_w, orig_h, cfg);
    let (target_w, target_h) =
        enforce_upscale(orig_w, orig_h, target_w, target_h, cfg.allow_upscale);

    if target_w == orig_w && target_h == orig_h {
        return Ok(img);
    }

    do_resize(img, target_w, target_h)
}

fn compute_target(orig_w: u32, orig_h: u32, cfg: &ResizeConfig) -> (u32, u32) {
    match cfg.mode {
        ResizeMode::MaxDimension => {
            let max_w = cfg.width.unwrap_or(orig_w).max(1) as f32;
            let max_h = cfg.height.unwrap_or(orig_h).max(1) as f32;
            let scale = (max_w / orig_w as f32).min(max_h / orig_h as f32);
            scale_pair(orig_w, orig_h, scale)
        }
        ResizeMode::Exact => {
            let w = cfg.width.unwrap_or(orig_w).max(1);
            let h = cfg.height.unwrap_or(orig_h).max(1);
            if cfg.keep_aspect_ratio {
                let scale = (w as f32 / orig_w as f32).min(h as f32 / orig_h as f32);
                scale_pair(orig_w, orig_h, scale)
            } else {
                (w, h)
            }
        }
        ResizeMode::Percentage => {
            let p = cfg.percentage.unwrap_or(1.0).max(0.0);
            scale_pair(orig_w, orig_h, p)
        }
    }
}

fn enforce_upscale(
    orig_w: u32,
    orig_h: u32,
    target_w: u32,
    target_h: u32,
    allow_upscale: bool,
) -> (u32, u32) {
    if allow_upscale {
        return (target_w, target_h);
    }
    if target_w <= orig_w && target_h <= orig_h {
        return (target_w, target_h);
    }
    let scale = (orig_w as f32 / target_w as f32).min(orig_h as f32 / target_h as f32);
    scale_pair(target_w, target_h, scale)
}

fn scale_pair(w: u32, h: u32, scale: f32) -> (u32, u32) {
    let s = if scale.is_finite() { scale } else { 1.0 };
    let nw = ((w as f32 * s).round() as i64).max(1) as u32;
    let nh = ((h as f32 * s).round() as i64).max(1) as u32;
    (nw, nh)
}

fn do_resize(img: DynamicImage, w: u32, h: u32) -> Result<DynamicImage, ImgError> {
    let rgba = img.to_rgba8();
    let (src_w, src_h) = rgba.dimensions();
    let src = fr::images::Image::from_vec_u8(src_w, src_h, rgba.into_raw(), fr::PixelType::U8x4)
        .map_err(|e| ImgError::processing(format!("imagen fuente fr: {e}")))?;
    let mut dst = fr::images::Image::new(w, h, fr::PixelType::U8x4);
    let mut resizer = fr::Resizer::new();
    let opts = fr::ResizeOptions::new()
        .resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3));
    resizer
        .resize(&src, &mut dst, &opts)
        .map_err(|e| ImgError::processing(format!("resize fr: {e}")))?;

    let buf = RgbaImage::from_raw(w, h, dst.into_vec())
        .ok_or_else(|| ImgError::processing("buffer de destino inválido"))?;
    Ok(DynamicImage::ImageRgba8(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn imagen(w: u32, h: u32) -> DynamicImage {
        let mut buf = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                buf.put_pixel(x, y, Rgba([(x % 255) as u8, (y % 255) as u8, 128, 255]));
            }
        }
        DynamicImage::ImageRgba8(buf)
    }

    #[test]
    fn max_dimension_cabe_en_la_caja() {
        let cfg = ResizeConfig {
            mode: ResizeMode::MaxDimension,
            width: Some(100),
            height: Some(100),
            percentage: None,
            keep_aspect_ratio: true,
            allow_upscale: false,
        };
        let out = resize_image(imagen(800, 400), &cfg).unwrap();
        assert!(out.width() <= 100 && out.height() <= 100);
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 50);
    }

    #[test]
    fn exact_sin_keep_aspect_devuelve_dimensiones_exactas() {
        let cfg = ResizeConfig {
            mode: ResizeMode::Exact,
            width: Some(123),
            height: Some(77),
            percentage: None,
            keep_aspect_ratio: false,
            allow_upscale: true,
        };
        let out = resize_image(imagen(500, 500), &cfg).unwrap();
        assert_eq!(out.width(), 123);
        assert_eq!(out.height(), 77);
    }

    #[test]
    fn exact_con_keep_aspect_respeta_proporcion() {
        let cfg = ResizeConfig {
            mode: ResizeMode::Exact,
            width: Some(200),
            height: Some(200),
            percentage: None,
            keep_aspect_ratio: true,
            allow_upscale: false,
        };
        let out = resize_image(imagen(400, 200), &cfg).unwrap();
        assert_eq!(out.width(), 200);
        assert_eq!(out.height(), 100);
    }

    #[test]
    fn percentage_escala_relativamente() {
        let cfg = ResizeConfig {
            mode: ResizeMode::Percentage,
            width: None,
            height: None,
            percentage: Some(0.25),
            keep_aspect_ratio: true,
            allow_upscale: false,
        };
        let out = resize_image(imagen(800, 400), &cfg).unwrap();
        assert_eq!(out.width(), 200);
        assert_eq!(out.height(), 100);
    }

    #[test]
    fn allow_upscale_apagado_no_agranda() {
        let cfg = ResizeConfig {
            mode: ResizeMode::Exact,
            width: Some(2000),
            height: Some(2000),
            percentage: None,
            keep_aspect_ratio: true,
            allow_upscale: false,
        };
        let out = resize_image(imagen(400, 200), &cfg).unwrap();
        assert!(out.width() <= 400 && out.height() <= 200);
    }

    #[test]
    fn allow_upscale_encendido_si_agranda() {
        let cfg = ResizeConfig {
            mode: ResizeMode::Percentage,
            width: None,
            height: None,
            percentage: Some(2.0),
            keep_aspect_ratio: true,
            allow_upscale: true,
        };
        let out = resize_image(imagen(100, 50), &cfg).unwrap();
        assert_eq!(out.width(), 200);
        assert_eq!(out.height(), 100);
    }
}
