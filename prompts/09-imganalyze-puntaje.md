# Prompt 09 — `imganalyze`: puntaje, ranking, recomendación y orquestación

**Objetivo:** calcular el puntaje compuesto, elegir ganadoras, recomendar por red y orquestar el análisis completo.
**Requisito previo:** Prompt 08 terminado y compilando.
**Entregables:** módulos `scoring.rs`, `social.rs` y la orquestación `analyze`, con tests.

---

▼ INICIO DEL PROMPT

```
En `imganalyze`, implementa el puntaje, el ranking, la recomendación por red
social y la orquestación del análisis.

Módulo `scoring.rs`:
- struct ScoringWeights { w_sharpness, w_exposure, w_aesthetic, w_resolution,
  face_bonus_max } con Default sensato: w_sharpness 0.30, w_exposure 0.22,
  w_aesthetic 0.38, w_resolution 0.10; face_bonus_max 0.10.
- Calcula los SubScores normalizados a 0..1:
  - sharpness: rango percentil del sharpness_raw dentro del lote.
  - exposure: 1 - penalización. La penalización da más peso al recorte de
    altas luces que al de sombras, y penaliza también la desviación de
    mean_luma respecto a la banda ideal 90..160.
  - resolution: min(1, total_píxeles / píxeles_referencia).
  - aesthetic: (mean - 1) / 9 si hay AestheticScore; si no, un valor neutro.
  - face_bonus: derivado de FaceSummary. Si no hay caras es 0; NUNCA penaliza.
- composite_score = base ponderada (sharpness, exposure, aesthetic,
  resolution) + face_bonus, todo recortado a 0..1.
- Asigna is_group_winner = true a la foto con mayor composite_score dentro de
  cada duplicate_group.

Módulo `social.rs`:
- Una tabla de redes editable y data-driven: nombre, relación de aspecto,
  orientación, resolución sugerida. Incluye Instagram feed (4:5 / 1:1),
  Stories/Reels/TikTok (9:16), Facebook feed (1.91:1) y miniatura YouTube
  (16:9).
- Para una foto y una red, calcula el encaje de formato:
  retención_del_recorte * sujeto_dentro * resolución_suficiente.
  "sujeto" = la cara principal si la hay; si no, el centro de la imagen.
- recomendación = encaje * composite_score.

Orquestación (módulo `analyze.rs` o en lib.rs):
- `analyze_photo(path, ai: Option<&AiEngine>) -> Result<AnalysisRecord>`:
  ejecuta decodificar (vía imgcore) -> leer EXIF -> hashes -> métricas de
  calidad -> IA si se pasó un AiEngine -> componer el AnalysisRecord (aún sin
  el scoring de lote).
- `analyze_batch(paths, ai, weights) -> Vec<AnalysisRecord>`: corre
  analyze_photo en paralelo con rayon; luego aplica dedup (agrupación) y
  scoring (que necesita el lote completo para los percentiles). Un archivo
  con error no detiene el lote: se registra y se continúa.

Agrega unit tests del puntaje compuesto y del encaje por red social.
Verifica `cargo test`.
```

▲ FIN DEL PROMPT
