//! Inicialización del logging estructurado.
//!
//! Escribe a `<app_data>/logs/wehi.log.<YYYY-MM-DD>` con rotación
//! diaria. Filtro por defecto: `info`. Override con la variable
//! de entorno `WEHI_LOG` (sintaxis estándar de `tracing-subscriber`):
//! `WEHI_LOG=debug` o `WEHI_LOG=wehi=debug,imgcore=info`.
//!
//! El `WorkerGuard` del appender se "leak-ea" a propósito porque
//! debe vivir lo que dura el proceso; soltarlo cierra el flush
//! del archivo.

use std::path::Path;

use tracing_appender::rolling;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Inicializa el subscriber global. Llamar una sola vez al arrancar.
/// Si falla (raro: solo si no se pueden crear los directorios), el
/// proceso sigue sin logging a archivo, con un mensaje a stderr.
pub fn init(app_data_dir: &Path) {
    let log_dir = app_data_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("WeHi: no se pudo crear directorio de logs: {e}");
        return;
    }

    let file_appender = rolling::daily(&log_dir, "wehi.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    // El guard debe vivir el proceso entero. Sin esto el writer se
    // cierra al salir del scope y los logs se truncan.
    std::mem::forget(guard);

    let env_filter = EnvFilter::try_from_env("WEHI_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let result = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .try_init();

    if let Err(e) = result {
        eprintln!("WeHi: no se pudo inicializar tracing: {e}");
    } else {
        tracing::info!(
            log_dir = %log_dir.display(),
            "logging inicializado"
        );
    }
}
