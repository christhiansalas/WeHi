//! Tipo de error compartido por todo el crate `imgcore`.
//!
//! Las variantes cubren los modos de fallo conocidos del pipeline:
//! E/S, decodificación, formato no soportado, ausencia del logo
//! configurado y fallos al extraer la previsualización embebida en
//! archivos RAW.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImgError {
    #[error("error de E/S en {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("no se pudo decodificar la imagen en {path}: {message}")]
    Decode { path: PathBuf, message: String },

    #[error("formato no soportado para {path}")]
    UnsupportedFormat { path: PathBuf },

    #[error("logo no encontrado: {logo_id}")]
    LogoNotFound { logo_id: String },

    #[error("no se pudo extraer la previsualización RAW de {path}: {message}")]
    RawPreviewExtraction { path: PathBuf, message: String },

    #[error("error de procesamiento: {message}")]
    Processing { message: String },
}

impl ImgError {
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn decode(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Decode {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn unsupported(path: impl Into<PathBuf>) -> Self {
        Self::UnsupportedFormat { path: path.into() }
    }

    pub fn logo_not_found(logo_id: impl Into<String>) -> Self {
        Self::LogoNotFound {
            logo_id: logo_id.into(),
        }
    }

    pub fn raw_preview(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::RawPreviewExtraction {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn processing(message: impl Into<String>) -> Self {
        Self::Processing {
            message: message.into(),
        }
    }
}
