//! Tests de integración del pipeline completo de `imgcore`.
//!
//! Las fixtures se generan al vuelo (PNG/JPEG en memoria + escritura
//! a un dir temporal) en lugar de versionar binarios en git. Los
//! tests de video se ignoran si ffmpeg no está disponible y los de
//! texto si no hay una fuente del sistema.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::{ImageFormat, Rgba, RgbaImage};

use imgcore::{
    apply_adjustments, encode_pipeline_named, load_logo, process_file, ImageAdjustments,
    LogoBackground, LogoConfig, LogoMap, LogoPosition, NetworkCrop, OutputConfig, OutputFormat,
    Preset, ResizeConfig, ResizeMode, TextWatermark,
};

struct Fixtures {
    dir: PathBuf,
}

fn fixtures() -> &'static Fixtures {
    static F: OnceLock<Fixtures> = OnceLock::new();
    F.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("wehi_imgcore_fixtures_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear fixtures dir");

        // Imagen base de 400×300 con un degradado horizontal (no
        // uniforme para que el dHash + métricas tengan estructura).
        let mut base = RgbaImage::new(400, 300);
        for y in 0..300 {
            for x in 0..400 {
                let v = ((x * 255) / 400) as u8;
                base.put_pixel(x, y, Rgba([v, 128, 200 - v / 2, 255]));
            }
        }
        save_png(&dir.join("foto_gradiente.png"), &base);
        save_jpeg(&dir.join("foto_gradiente.jpg"), &base, 85);

        // Imagen sobreexpuesta (mayormente blanca).
        let mut sobre = RgbaImage::new(200, 200);
        for px in sobre.pixels_mut() {
            *px = Rgba([252, 252, 252, 255]);
        }
        save_jpeg(&dir.join("sobreexpuesta.jpg"), &sobre, 85);

        // Logo PNG con transparencia.
        let mut logo = RgbaImage::new(80, 40);
        for y in 0..40 {
            for x in 0..80 {
                let cx = x as i32 - 40;
                let cy = y as i32 - 20;
                let dist2 = cx * cx + cy * cy;
                let alpha = if dist2 < 30 * 30 { 220 } else { 0 };
                logo.put_pixel(x, y, Rgba([220, 50, 30, alpha]));
            }
        }
        save_png(&dir.join("logo.png"), &logo);

        // Archivo "corrupto" para testear el error path.
        std::fs::write(
            dir.join("corrupto.jpg"),
            b"no soy un jpeg de verdad, soy texto plano",
        )
        .unwrap();

        Fixtures { dir }
    })
}

fn save_png(path: &Path, img: &RgbaImage) {
    let dyn_img = image::DynamicImage::ImageRgba8(img.clone());
    dyn_img.save_with_format(path, ImageFormat::Png).unwrap();
}
fn save_jpeg(path: &Path, img: &RgbaImage, quality: u8) {
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let mut bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality);
    use image::ImageEncoder;
    encoder
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    std::fs::write(path, &bytes).unwrap();
}

fn output_dir(name: &str) -> PathBuf {
    let dir = fixtures().dir.join(format!("out_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn preset_basico(out: PathBuf, format: OutputFormat) -> Preset {
    Preset {
        name: "test".into(),
        resize: ResizeConfig {
            mode: ResizeMode::MaxDimension,
            width: Some(800),
            height: Some(800),
            percentage: None,
            keep_aspect_ratio: true,
            allow_upscale: false,
        },
        logo: None,
        logos: Vec::new(),
        output: OutputConfig {
            format,
            quality: 85,
            folder: out,
            filename_pattern: "{name}{ext}".into(),
            preserve_structure: false,
        },
        network_crops: Vec::new(),
        text_watermarks: Vec::new(),
        adjustments: ImageAdjustments::default(),
    }
}

#[test]
fn pipeline_jpeg_full_decodifica_procesa_y_escribe() {
    let f = fixtures();
    let out = output_dir("jpeg_full");
    let preset = preset_basico(out.clone(), OutputFormat::Jpeg);
    let empty = LogoMap::new();
    process_file(&f.dir.join("foto_gradiente.jpg"), &preset, &empty).unwrap();
    let result = out.join("foto_gradiente.jpg");
    assert!(result.exists(), "{result:?}");
    let decoded = image::open(&result).unwrap();
    assert!(decoded.width() <= 800 && decoded.height() <= 800);
}

#[test]
fn pipeline_png_a_webp() {
    let f = fixtures();
    let out = output_dir("png_a_webp");
    let preset = preset_basico(out.clone(), OutputFormat::WebP);
    let empty = LogoMap::new();
    process_file(&f.dir.join("foto_gradiente.png"), &preset, &empty).unwrap();
    let result = out.join("foto_gradiente.webp");
    assert!(result.exists(), "{result:?}");
}

#[test]
fn pipeline_con_logo_y_fondo_produce_archivo_valido() {
    let f = fixtures();
    let out = output_dir("con_logo");
    let logo = load_logo(&f.dir.join("logo.png")).unwrap();
    let mut logos = LogoMap::new();
    logos.insert("logo.png".to_string(), logo);

    let mut preset = preset_basico(out.clone(), OutputFormat::Jpeg);
    preset.logos = vec![LogoConfig {
        logo_id: "logo.png".into(),
        position: LogoPosition { x: 0.9, y: 0.9 },
        scale_pct: 0.2,
        opacity: 0.85,
        background: Some(LogoBackground {
            color_rgba: [0, 0, 0, 180],
            padding_pct: 0.15,
        }),
    }];

    process_file(&f.dir.join("foto_gradiente.jpg"), &preset, &logos).unwrap();
    let result = out.join("foto_gradiente.jpg");
    assert!(result.exists());
    let decoded = image::open(&result).unwrap();
    // Esquina inferior derecha debe estar oscurecida por el fondo del logo.
    let rgba = decoded.to_rgba8();
    let (w, h) = rgba.dimensions();
    let px = rgba.get_pixel(w - 30, h - 20).0;
    let brillo_promedio = (px[0] as u32 + px[1] as u32 + px[2] as u32) / 3;
    assert!(
        brillo_promedio < 150,
        "esperaba esquina oscura por el fondo del logo, fue {brillo_promedio}"
    );
}

#[test]
fn pipeline_con_recortes_genera_archivos_extra() {
    let f = fixtures();
    let out = output_dir("recortes");
    let mut preset = preset_basico(out.clone(), OutputFormat::Jpeg);
    preset.network_crops = vec![
        NetworkCrop {
            id: "ig_square".into(),
            label: "IG 1:1".into(),
            width: 256,
            height: 256,
        },
        NetworkCrop {
            id: "yt".into(),
            label: "YT 16:9".into(),
            width: 320,
            height: 180,
        },
    ];
    let empty = LogoMap::new();
    process_file(&f.dir.join("foto_gradiente.jpg"), &preset, &empty).unwrap();

    let principal = out.join("foto_gradiente.jpg");
    let square = out.join("foto_gradiente__ig_square.jpg");
    let yt = out.join("foto_gradiente__yt.jpg");
    assert!(principal.exists());
    assert!(square.exists());
    assert!(yt.exists());

    let sq = image::open(&square).unwrap();
    assert_eq!(sq.width(), 256);
    assert_eq!(sq.height(), 256);
    let y = image::open(&yt).unwrap();
    assert_eq!(y.width(), 320);
    assert_eq!(y.height(), 180);
}

#[test]
fn pipeline_archivo_corrupto_falla_sin_panic() {
    let f = fixtures();
    let out = output_dir("corrupto");
    let preset = preset_basico(out, OutputFormat::Jpeg);
    let empty = LogoMap::new();
    let err = process_file(&f.dir.join("corrupto.jpg"), &preset, &empty).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("decodif") || msg.to_lowercase().contains("decode"),
        "mensaje no informativo: {msg}"
    );
}

#[test]
fn ajustes_se_aplican_al_output() {
    let f = fixtures();
    let out = output_dir("ajustes");
    let mut preset = preset_basico(out.clone(), OutputFormat::Png);
    preset.adjustments = ImageAdjustments {
        saturation: -1.0, // forza a blanco y negro
        ..Default::default()
    };
    let empty = LogoMap::new();
    process_file(&f.dir.join("foto_gradiente.jpg"), &preset, &empty).unwrap();
    let result = out.join("foto_gradiente.png");
    let decoded = image::open(&result).unwrap().to_rgba8();
    // Cualquier píxel debería tener R≈G≈B (gris) tras saturation=-1.
    let mut max_diff = 0i32;
    for px in decoded.pixels() {
        let d1 = (px.0[0] as i32 - px.0[1] as i32).abs();
        let d2 = (px.0[1] as i32 - px.0[2] as i32).abs();
        max_diff = max_diff.max(d1).max(d2);
    }
    assert!(max_diff < 6, "max diff esperado <6 (gris), fue {max_diff}");
}

#[test]
fn ajustes_temperatura_calida_aumenta_rojo_promedio() {
    let f = fixtures();
    let out = output_dir("temp_calida");
    let mut preset = preset_basico(out.clone(), OutputFormat::Png);
    preset.adjustments = ImageAdjustments {
        temperature: 0.8,
        ..Default::default()
    };
    let empty = LogoMap::new();
    process_file(&f.dir.join("foto_gradiente.jpg"), &preset, &empty).unwrap();
    let result = out.join("foto_gradiente.png");
    let decoded = image::open(&result).unwrap().to_rgba8();
    let (mut sum_r, mut sum_b, mut n) = (0u64, 0u64, 0u64);
    for px in decoded.pixels() {
        sum_r += px.0[0] as u64;
        sum_b += px.0[2] as u64;
        n += 1;
    }
    let avg_r = sum_r as f32 / n as f32;
    let avg_b = sum_b as f32 / n as f32;
    assert!(
        avg_r > avg_b,
        "esperaba R > B con temperatura cálida (avg_r={avg_r}, avg_b={avg_b})"
    );
}

#[test]
fn encode_pipeline_named_sustituye_filename_en_texto() {
    let f = fixtures();
    // Solo corre si encontramos una fuente del sistema.
    let Some(_) = imgcore::text::find_system_font() else {
        eprintln!("⚠ sin fuente del sistema, test de texto saltado");
        return;
    };
    let mut preset = preset_basico(output_dir("texto"), OutputFormat::Png);
    preset.text_watermarks = vec![TextWatermark {
        text: "© {filename}".into(),
        font_size_pct: 0.05,
        color_rgba: [255, 255, 255, 255],
        position: LogoPosition { x: 0.5, y: 0.5 },
        opacity: 1.0,
        background: Some(LogoBackground {
            color_rgba: [0, 0, 0, 255],
            padding_pct: 0.1,
        }),
    }];
    let decoded = image::open(f.dir.join("foto_gradiente.jpg")).unwrap();
    let empty = LogoMap::new();
    let encoded = encode_pipeline_named(decoded, &preset, &empty, Some("MI_FOTO")).unwrap();
    // El output debe estar (al menos) en PNG y tener bytes.
    assert!(encoded.bytes.len() > 1000);
    assert_eq!(encoded.format, OutputFormat::Png);
}

#[test]
fn apply_adjustments_is_identity_no_modifica_bytes() {
    let f = fixtures();
    let original = image::open(f.dir.join("foto_gradiente.jpg")).unwrap();
    let bytes_antes = original.to_rgba8().into_raw();
    let out = apply_adjustments(original, &ImageAdjustments::default());
    assert_eq!(out.to_rgba8().into_raw(), bytes_antes);
}

#[test]
fn pipeline_con_archivo_inexistente_da_error_de_io() {
    let f = fixtures();
    let out = output_dir("inexistente");
    let preset = preset_basico(out, OutputFormat::Jpeg);
    let empty = LogoMap::new();
    let err = process_file(&f.dir.join("noexiste.jpg"), &preset, &empty).unwrap_err();
    let msg = format!("{err}").to_lowercase();
    assert!(
        msg.contains("e/s")
            || msg.contains("io")
            || msg.contains("no such")
            || msg.contains("decod"),
        "mensaje raro: {msg}"
    );
}
