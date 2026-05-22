# Prompt 05 — Crate `imganalyze`: andamiaje y modelo de datos

**Objetivo:** crear el crate de curación y su modelo de datos del análisis.
**Requisito previo:** Prompt 04 terminado y compilando.
**Entregables:** crate `imganalyze` en el workspace, módulo `record` con los structs y tests.

---

▼ INICIO DEL PROMPT

```
Crea el crate `imganalyze`, el motor de curación de WeHi.

- Añádelo como nuevo miembro del workspace de Cargo, en `crates/imganalyze`.
- `imganalyze` declara `imgcore` como dependencia de ruta (reutilizará su
  decodificación). `imgcore` NO debe depender de `imganalyze`.
- `src-tauri` declara `imganalyze` como dependencia de ruta.
- Crea los módulos del crate (por ahora pueden ser stubs vacíos salvo
  `record`): hashing, dedup, quality, ai, scoring, social, record, cache.

Módulo `record.rs` — el modelo de datos del análisis. Todos los structs y
enums derivan Serialize, Deserialize, Clone y Debug:

- enum CullStatus { Keep, Reject, Undecided }
- struct QualityMetrics { sharpness_raw: f32, clipped_highlights_pct: f32,
  clipped_shadows_pct: f32, mean_luma: f32, contrast: f32 }
- struct AestheticScore { mean: f32, std_dev: f32 }
- struct FaceSummary { count: u32, largest_face_frac: f32,
  main_face_centered: f32 }
- struct SubScores { sharpness: f32, exposure: f32, resolution: f32,
  aesthetic: f32, face_bonus: f32 }
- struct AnalysisRecord {
    path: PathBuf,
    file_size: u64,
    modified: SystemTime,
    width: u32,
    height: u32,
    capture_time: Option<SystemTime>,
    content_hash: [u8; 32],
    perceptual_hash: u64,
    quality: QualityMetrics,
    aesthetic: Option<AestheticScore>,
    faces: Option<FaceSummary>,
    sub_scores: SubScores,
    composite_score: f32,
    duplicate_group: Option<u32>,
    is_group_winner: bool,
    status: CullStatus,
  }

Agrega unit tests del round-trip de serialización JSON de un AnalysisRecord
completo. Verifica `cargo build` y `cargo test`.
```

▲ FIN DEL PROMPT
