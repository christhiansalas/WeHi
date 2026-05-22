//! Capa de IA enchufable: puntaje estético (estilo NIMA) y
//! detección de caras, ambos como modelos ONNX externos ejecutados
//! en CPU con `tract`.
//!
//! Los modelos se tratan como recurso enchufable: la ruta a cada
//! archivo ONNX es configurable y la falta de un modelo NO bloquea
//! el análisis (los campos `aesthetic` y `faces` del registro
//! quedarán en `None`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::{imageops::FilterType, DynamicImage};
use tract_onnx::prelude::*;

use imgcore::ImgError;

use crate::record::{AestheticScore, FaceSummary};

/// Ruta a cada modelo ONNX. Cualquiera puede ser `None`; en ese
/// caso el motor simplemente no produce ese tipo de salida.
#[derive(Debug, Clone, Default)]
pub struct AiEngineConfig {
    pub aesthetic_model: Option<PathBuf>,
    pub faces_model: Option<PathBuf>,
    /// Lado al que se redimensiona la imagen antes de pasarla al
    /// modelo de estética. La mayoría de los modelos NIMA esperan 224.
    pub aesthetic_input_size: u32,
    /// Lado al que se redimensiona la imagen antes del detector de
    /// caras. Modelos típicos (BlazeFace/UltraFace) usan 320 o 640.
    pub faces_input_size: u32,
    /// Umbral de confianza para considerar una detección como cara.
    pub faces_confidence_threshold: f32,
}

impl AiEngineConfig {
    pub fn new() -> Self {
        Self {
            aesthetic_model: None,
            faces_model: None,
            aesthetic_input_size: 224,
            faces_input_size: 320,
            faces_confidence_threshold: 0.6,
        }
    }
}

type RunnableModel = Arc<TypedRunnableModel<TypedModel>>;

/// Motor de IA listo para usar en el lote. Construye los planes
/// ejecutables UNA sola vez; clonar `AiEngine` es barato gracias a
/// `Arc` (compartir el plan entre hilos).
#[derive(Clone)]
pub struct AiEngine {
    aesthetic: Option<AestheticEngine>,
    faces: Option<FaceEngine>,
}

#[derive(Clone)]
struct AestheticEngine {
    model: RunnableModel,
    input_size: u32,
}

#[derive(Clone)]
struct FaceEngine {
    model: RunnableModel,
    input_size: u32,
    confidence_threshold: f32,
}

impl AiEngine {
    /// Carga los modelos disponibles. Si un archivo no existe o falla
    /// la construcción del plan, se devuelve [`ImgError::Processing`]
    /// con un mensaje descriptivo: el llamador puede decidir si
    /// continuar el análisis sin IA.
    pub fn new(cfg: &AiEngineConfig) -> Result<Self, ImgError> {
        let aesthetic = match cfg.aesthetic_model.as_deref() {
            Some(p) => Some(AestheticEngine::load(p, cfg.aesthetic_input_size)?),
            None => None,
        };
        let faces = match cfg.faces_model.as_deref() {
            Some(p) => Some(FaceEngine::load(
                p,
                cfg.faces_input_size,
                cfg.faces_confidence_threshold,
            )?),
            None => None,
        };
        Ok(Self { aesthetic, faces })
    }

    pub fn has_aesthetic(&self) -> bool {
        self.aesthetic.is_some()
    }

    pub fn has_faces(&self) -> bool {
        self.faces.is_some()
    }

    /// Aplica el modelo de estética. Devuelve `None` si no hay
    /// modelo cargado.
    pub fn aesthetic(&self, img: &DynamicImage) -> Option<Result<AestheticScore, ImgError>> {
        self.aesthetic.as_ref().map(|m| m.score(img))
    }

    /// Aplica el detector de caras. Devuelve `None` si no hay modelo
    /// cargado.
    pub fn faces(&self, img: &DynamicImage) -> Option<Result<FaceSummary, ImgError>> {
        self.faces.as_ref().map(|m| m.detect(img))
    }
}

fn load_runnable(path: &Path) -> Result<RunnableModel, ImgError> {
    if !path.exists() {
        return Err(ImgError::processing(format!(
            "modelo ONNX no encontrado: {}",
            path.display()
        )));
    }
    let model = tract_onnx::onnx()
        .model_for_path(path)
        .and_then(|m| m.into_optimized())
        .and_then(|m| m.into_runnable())
        .map_err(|e| ImgError::processing(format!("carga ONNX {}: {e}", path.display())))?;
    Ok(Arc::new(model))
}

impl AestheticEngine {
    fn load(path: &Path, input_size: u32) -> Result<Self, ImgError> {
        Ok(Self {
            model: load_runnable(path)?,
            input_size,
        })
    }

    /// Forma del tensor esperada por defecto: `[1, 3, H, W]` (NCHW),
    /// RGB normalizado a `[0,1]` y luego (x - 0.5) / 0.5. La salida
    /// se interpreta como distribución de probabilidad sobre las
    /// categorías 1..=10.
    fn score(&self, img: &DynamicImage) -> Result<AestheticScore, ImgError> {
        let tensor = preprocess_nchw(img, self.input_size, self.input_size);
        let outputs = self
            .model
            .run(tvec!(tensor.into()))
            .map_err(|e| ImgError::processing(format!("inferencia estética: {e}")))?;
        let view = outputs
            .first()
            .ok_or_else(|| ImgError::processing("salida estética vacía"))?;
        let probs = extract_probs(view, 10)?;
        let mean = (1..=10)
            .zip(probs.iter())
            .map(|(i, p)| i as f32 * p)
            .sum::<f32>();
        let var = probs
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let d = (i as f32 + 1.0) - mean;
                d * d * p
            })
            .sum::<f32>();
        Ok(AestheticScore {
            mean,
            std_dev: var.max(0.0).sqrt(),
        })
    }
}

impl FaceEngine {
    fn load(path: &Path, input_size: u32, confidence_threshold: f32) -> Result<Self, ImgError> {
        Ok(Self {
            model: load_runnable(path)?,
            input_size,
            confidence_threshold,
        })
    }

    /// Espera salidas con cajas delimitadoras y confianza. La forma
    /// exacta depende del modelo (RetinaFace, UltraFace, BlazeFace);
    /// se intenta una interpretación genérica donde la última
    /// dimensión incluye `(x1, y1, x2, y2, score)` en coordenadas
    /// normalizadas 0..1.
    fn detect(&self, img: &DynamicImage) -> Result<FaceSummary, ImgError> {
        let tensor = preprocess_nchw(img, self.input_size, self.input_size);
        let outputs = self
            .model
            .run(tvec!(tensor.into()))
            .map_err(|e| ImgError::processing(format!("inferencia caras: {e}")))?;
        let view = outputs
            .first()
            .ok_or_else(|| ImgError::processing("salida de caras vacía"))?;
        let boxes = extract_face_boxes(view, self.confidence_threshold)?;
        Ok(summarize_faces(&boxes))
    }
}

fn preprocess_nchw(img: &DynamicImage, w: u32, h: u32) -> Tensor {
    let small = img.resize_exact(w, h, FilterType::Triangle).to_rgb8();
    let mut data = vec![0f32; 3 * (w as usize) * (h as usize)];
    let plane = (w as usize) * (h as usize);
    for (i, pixel) in small.pixels().enumerate() {
        let [r, g, b] = pixel.0;
        let nr = (r as f32 / 255.0 - 0.5) / 0.5;
        let ng = (g as f32 / 255.0 - 0.5) / 0.5;
        let nb = (b as f32 / 255.0 - 0.5) / 0.5;
        data[i] = nr;
        data[plane + i] = ng;
        data[2 * plane + i] = nb;
    }
    tract_ndarray::Array4::from_shape_vec((1, 3, h as usize, w as usize), data)
        .expect("forma NCHW válida")
        .into_tensor()
}

fn extract_probs(tensor: &Tensor, expected: usize) -> Result<Vec<f32>, ImgError> {
    let view = tensor
        .to_array_view::<f32>()
        .map_err(|e| ImgError::processing(format!("salida estética no f32: {e}")))?;
    let flat: Vec<f32> = view.iter().copied().collect();
    if flat.len() < expected {
        return Err(ImgError::processing(format!(
            "salida estética con {} elementos, se esperaban ≥ {expected}",
            flat.len()
        )));
    }
    let probs: Vec<f32> = flat[..expected].to_vec();
    let sum: f32 = probs.iter().sum();
    if sum > 0.0 {
        Ok(probs.into_iter().map(|p| p / sum).collect())
    } else {
        Ok(vec![1.0 / expected as f32; expected])
    }
}

/// Caja delimitadora normalizada: (cx, cy, w, h, score) en 0..1.
#[derive(Debug, Clone, Copy)]
struct FaceBox {
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
}

fn extract_face_boxes(tensor: &Tensor, threshold: f32) -> Result<Vec<FaceBox>, ImgError> {
    let view = tensor
        .to_array_view::<f32>()
        .map_err(|e| ImgError::processing(format!("salida caras no f32: {e}")))?;
    let shape = view.shape().to_vec();
    let last = *shape.last().unwrap_or(&0);
    if last < 5 {
        return Ok(Vec::new());
    }
    let flat: Vec<f32> = view.iter().copied().collect();
    let mut boxes = Vec::new();
    for chunk in flat.chunks(last) {
        let score = chunk[4];
        if score < threshold {
            continue;
        }
        let (x1, y1, x2, y2) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        let w = (x2 - x1).abs();
        let h = (y2 - y1).abs();
        let cx = (x1 + x2) / 2.0;
        let cy = (y1 + y2) / 2.0;
        if w > 0.0 && h > 0.0 {
            boxes.push(FaceBox { cx, cy, w, h });
        }
    }
    Ok(boxes)
}

fn summarize_faces(boxes: &[FaceBox]) -> FaceSummary {
    if boxes.is_empty() {
        return FaceSummary::default();
    }
    let count = boxes.len() as u32;
    let largest = boxes
        .iter()
        .map(|b| b.w * b.h)
        .fold(0.0f32, f32::max)
        .clamp(0.0, 1.0);
    let main = boxes
        .iter()
        .max_by(|a, b| {
            (a.w * a.h)
                .partial_cmp(&(b.w * b.h))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        .unwrap_or(FaceBox {
            cx: 0.5,
            cy: 0.5,
            w: 0.0,
            h: 0.0,
        });
    // Distancia normalizada al centro (0..1 aprox); convertir a
    // "centradez" en 0..1 (1 = perfectamente centrada).
    let dx = (main.cx - 0.5).abs();
    let dy = (main.cy - 0.5).abs();
    let dist = (dx * dx + dy * dy).sqrt();
    let centered = (1.0 - (dist / 0.707).min(1.0)).clamp(0.0, 1.0);
    FaceSummary {
        count,
        largest_face_frac: largest,
        main_face_centered: centered,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn imagen() -> DynamicImage {
        let mut img = RgbaImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                img.put_pixel(x, y, Rgba([(x * 4) as u8, (y * 4) as u8, 128, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn motor_sin_modelos_es_valido_y_devuelve_none() {
        let engine = AiEngine::new(&AiEngineConfig::new()).unwrap();
        assert!(!engine.has_aesthetic());
        assert!(!engine.has_faces());
        let img = imagen();
        assert!(engine.aesthetic(&img).is_none());
        assert!(engine.faces(&img).is_none());
    }

    #[test]
    fn modelo_inexistente_devuelve_error_controlado() {
        let mut cfg = AiEngineConfig::new();
        cfg.aesthetic_model = Some(PathBuf::from("/no/existe/modelo.onnx"));
        match AiEngine::new(&cfg) {
            Err(ImgError::Processing { .. }) => {}
            Err(e) => panic!("error inesperado: {e}"),
            Ok(_) => panic!("se esperaba error"),
        }
    }

    #[test]
    fn extract_probs_normaliza_a_distribucion() {
        let t = tract_ndarray::Array1::from(vec![1.0f32, 1.0, 2.0, 2.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5])
            .into_tensor();
        let probs = extract_probs(&t, 10).unwrap();
        let suma: f32 = probs.iter().sum();
        assert!((suma - 1.0).abs() < 1e-5, "suma fue {suma}");
        assert_eq!(probs.len(), 10);
    }

    #[test]
    fn summarize_faces_centro_y_tamano_se_calculan_bien() {
        let boxes = vec![FaceBox {
            cx: 0.5,
            cy: 0.5,
            w: 0.2,
            h: 0.2,
        }];
        let s = summarize_faces(&boxes);
        assert_eq!(s.count, 1);
        assert!((s.largest_face_frac - 0.04).abs() < 1e-4);
        assert!(s.main_face_centered > 0.99);
    }

    #[test]
    fn summarize_faces_vacio_es_default() {
        let s = summarize_faces(&[]);
        assert_eq!(s, FaceSummary::default());
    }
}
