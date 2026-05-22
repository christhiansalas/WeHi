//! Motor de curación de WeHi.
//!
//! Crate de Rust puro. Depende de [`imgcore`] (decodificación
//! compartida) pero `imgcore` NO depende de `imganalyze`.

pub mod ai;
pub mod analyze;
pub mod cache;
pub mod dedup;
pub mod hashing;
pub mod persons;
pub mod quality;
pub mod record;
pub mod scoring;
pub mod social;

pub use ai::{AiEngine, AiEngineConfig, FaceBox};
pub use analyze::{analyze_batch, analyze_photo, analyze_photo_with_embedding, BatchError};
pub use dedup::{assign_duplicate_groups, DedupConfig};
pub use persons::{best_photo_per_person, cluster_persons};
pub use record::{
    AestheticScore, AnalysisRecord, CullStatus, FaceSummary, QualityMetrics, SubScores,
};
pub use scoring::{score_batch, ScoringWeights};
pub use social::{default_networks, format_fit, recommendation, NetworkOrientation, NetworkSpec};
