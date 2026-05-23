//! Benchmark sintético del pipeline completo de análisis.
//!
//! Genera N JPEGs en /tmp, corre `analyze_batch` y reporta latencias.
//! Útil para comparar antes/después de cambios o validar que el lote
//! no se cuelga con muchos archivos. NO sustituye un benchmark real
//! con fotos RAW de cámara.
//!
//! Uso: `cargo run -p imganalyze --release --example bench_pipeline -- 50`

use std::path::PathBuf;
use std::time::Instant;

use image::{ImageBuffer, Rgb};
use imganalyze::{analyze_batch, ScoringWeights};

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    let dir = std::env::temp_dir().join(format!("wehi_bench_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("crear tempdir");
    println!("→ generando {n} JPEGs en {dir:?}");

    let t0 = Instant::now();
    let mut paths: Vec<PathBuf> = Vec::with_capacity(n);
    for i in 0..n {
        let w = 3000u32;
        let h = 2000u32;
        let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(w, h);
        for y in 0..h {
            for x in 0..w {
                // Gradiente único por imagen para que dedup no las junte.
                let r = ((x + y + i as u32 * 50) % 256) as u8;
                let g = ((x * 2 + i as u32 * 7) % 256) as u8;
                let b = ((y * 3 + i as u32 * 13) % 256) as u8;
                img.put_pixel(x, y, Rgb([r, g, b]));
            }
        }
        let path = dir.join(format!("img_{i:03}.jpg"));
        img.save(&path).expect("guardar jpeg");
        paths.push(path);
    }
    let gen_ms = t0.elapsed().as_millis();
    let total_bytes: u64 = paths
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();
    println!(
        "   {n} imgs · {:.1} MB total · gen en {gen_ms} ms",
        total_bytes as f64 / 1_048_576.0
    );

    println!("→ analyze_batch (sin IA)...");
    let t1 = Instant::now();
    let (records, errors) = analyze_batch(&paths, None, &ScoringWeights::default());
    let analyze_ms = t1.elapsed().as_millis();
    println!(
        "   {} OK · {} err · {analyze_ms} ms total · {:.1} ms/imagen",
        records.len(),
        errors.len(),
        analyze_ms as f64 / n as f64
    );
    let throughput = (n as f64 * 1000.0) / analyze_ms as f64;
    println!("   throughput: {throughput:.1} img/s");

    let _ = std::fs::remove_dir_all(&dir);
    println!("✓ cleanup OK");
}
