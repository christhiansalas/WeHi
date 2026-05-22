use image::{DynamicImage, Rgba, RgbaImage};
use imganalyze::{AiEngine, AiEngineConfig};
use std::path::PathBuf;

fn main() {
    let model = PathBuf::from("src-tauri/models/arcface.onnx");
    let mut cfg = AiEngineConfig::new();
    cfg.arcface_model = Some(model.clone());
    println!("Cargando {} ...", model.display());
    let engine = match AiEngine::new(&cfg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("ERROR carga: {e}");
            std::process::exit(1);
        }
    };
    println!("Carga OK. has_arcface={}", engine.has_arcface());

    let mut img = RgbaImage::new(112, 112);
    for y in 0..112 {
        for x in 0..112 {
            img.put_pixel(
                x,
                y,
                Rgba([
                    ((x * 2) as u8).wrapping_add(50),
                    ((y * 2) as u8).wrapping_add(100),
                    ((x + y) as u8).wrapping_add(150),
                    255,
                ]),
            );
        }
    }
    let crop = DynamicImage::ImageRgba8(img);
    match engine.arcface_embed(&crop) {
        Some(Ok(emb)) => {
            println!(
                "Embedding OK: dim={}, primeros 5={:?}",
                emb.len(),
                &emb[..5.min(emb.len())]
            );
            let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
            println!("L2 norm={norm:.6} (esperado ~1.0)");
        }
        Some(Err(e)) => {
            eprintln!("ERROR inferencia: {e}");
            std::process::exit(1);
        }
        None => {
            eprintln!("ERROR: no hay arcface");
            std::process::exit(1);
        }
    }
}
