//! Motor de procesamiento de imágenes de WeHi.
//!
//! Crate de Rust puro: no depende de Tauri. El pipeline completo
//! vive en [`pipeline`], y los puntos de entrada usados desde la
//! capa Tauri son [`pipeline::process_image`] (previsualización) y
//! [`pipeline::process_file`] (lote masivo).

pub mod adjust;
pub mod config;
pub mod decode;
pub mod encode;
pub mod error;
pub mod exif;
#[cfg(feature = "gpu")]
pub mod gpu_overlay;
pub mod overlay;
pub mod pipeline;
pub mod raw;
pub mod resize;
pub mod text;
pub mod video;

pub use adjust::{apply_adjustments, ImageAdjustments};
pub use config::{
    LogoBackground, LogoConfig, LogoPosition, NetworkCrop, OutputConfig, OutputFormat, Preset,
    ResizeConfig, ResizeMode, TextWatermark,
};
pub use decode::decode;
pub use encode::encode_image;
pub use error::ImgError;
#[cfg(feature = "gpu")]
pub use gpu_overlay::{compose_logo_auto, compose_logo_gpu, GpuOverlay};
pub use overlay::{compose_logo, load_logo, LoadedLogo};

/// Compositor activo en runtime: GPU si el feature está prendido y el
/// adapter pudo inicializarse, CPU en caso contrario. Las llamadas
/// internas del pipeline pasan por aquí.
#[cfg(not(feature = "gpu"))]
pub use overlay::compose_logo as compose_logo_auto;
pub use pipeline::{
    encode_pipeline, encode_pipeline_named, process_file, process_file_with_progress,
    process_image, EncodedImage, LogoMap,
};
pub use resize::resize_image;

/// Versión del crate `imgcore` declarada en `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_no_esta_vacia() {
        assert!(!version().is_empty());
    }
}
