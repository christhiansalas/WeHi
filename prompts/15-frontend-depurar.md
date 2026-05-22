# Prompt 15 — Frontend: etapa "Depurar / Revisar"

**Objetivo:** ejecutar el análisis y dejar que el usuario revise y decida qué conservar.
**Requisito previo:** Prompt 14 terminado y compilando.
**Entregables:** botón de análisis, grilla de revisión, gestión de descartes.

---

▼ INICIO DEL PROMPT

```
Implementa la etapa "Depurar / Revisar" en el frontend de WeHi.

- Añade a `src/types.ts` los tipos del crate `imganalyze`: AnalysisRecord,
  QualityMetrics, AestheticScore, FaceSummary, SubScores, CullStatus,
  ScoringWeights.
- Botón "Analizar" con una casilla "incluir análisis de IA" (estética y
  caras). Invoca el comando `analyze_batch`. Muestra el progreso escuchando
  los eventos `analysis_progress` y `analysis_done`.
- Componente de revisión:
  - Grilla de fotos ordenada por composite_score, de mejor a peor,
    virtualizada para soportar miles de elementos.
  - Cada foto muestra su miniatura y sus SUB-PUNTAJES (nitidez, exposición,
    estética, caras), no solo el número final: el usuario debe poder ver
    por qué una foto quedó donde quedó.
  - Las fotos del mismo duplicate_group se muestran agrupadas, con la
    ganadora (is_group_winner) destacada.
  - Controles Keep / Reject / Undecided por foto (comando `set_cull_status`).
  - Los candidatos a descarte (borrosas, casi-duplicadas que no son
    ganadoras, sobre o subexpuestas) vienen pre-marcados como Reject.
- Panel de pesos de puntaje (ScoringWeights) que reordena la grilla en vivo
  al ajustar los pesos.
- Acción "Mover descartes": llama al comando `move_rejects`, SIEMPRE con una
  confirmación explícita del usuario. Deja claro en la interfaz que mueve los
  archivos a una subcarpeta, NO los elimina.

Verifica el flujo con un lote de prueba que incluya ráfagas y fotos borrosas.
```

▲ FIN DEL PROMPT
