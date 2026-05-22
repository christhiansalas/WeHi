mod analysis;
mod cache;
mod commands;
mod logging;
mod metrics;
mod queue;
mod state;
mod watch;

use metrics::MetricsState;
use state::{AnalysisState, AppState};
use tauri::Manager;
use watch::WatchState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            if let Ok(dir) = app.path().app_data_dir() {
                logging::init(&dir);
                let metrics_state = app.state::<MetricsState>();
                metrics_state.init(&dir);
            }
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "WeHi arrancando");
            Ok(())
        })
        .manage(AppState::new())
        .manage(AnalysisState::new())
        .manage(MetricsState::new())
        .manage(WatchState::new())
        .invoke_handler(tauri::generate_handler![
            commands::scan_folder,
            commands::scan_paths,
            commands::process_batch,
            commands::process_preview,
            commands::cancel_batch,
            commands::set_max_threads,
            commands::set_video_concurrency,
            commands::clear_thumbnail_cache,
            metrics::get_metrics,
            metrics::set_metrics_enabled,
            metrics::clear_metrics,
            watch::start_watching,
            watch::stop_watching,
            watch::watch_status,
            commands::list_logos,
            commands::add_logo,
            commands::delete_logo,
            commands::get_logo_data_url,
            commands::get_file_data_url,
            commands::get_thumbnail_data_url,
            commands::check_ffmpeg,
            commands::check_font,
            commands::read_preferences,
            commands::write_preferences,
            commands::get_active_batch,
            commands::clear_active_batch,
            commands::list_presets,
            commands::save_preset,
            commands::delete_preset,
            analysis::analyze_batch,
            analysis::cancel_analysis,
            analysis::get_analysis_results,
            analysis::set_cull_status,
            analysis::get_recommendations,
            analysis::list_networks,
            analysis::move_rejects,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
