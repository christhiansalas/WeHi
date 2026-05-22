# Prompt 07 — `imganalyze`: métricas de calidad objetivas

**Objetivo:** calcular nitidez, exposición y resolución de cada foto.
**Requisito previo:** Prompt 06 terminado y compilando.
**Entregables:** módulo `quality.rs` con tests.

---

▼ INICIO DEL PROMPT

```
En `imganalyze`, implementa las métricas de calidad objetivas en el módulo
`quality.rs`.

Todas las métricas se calculan sobre el canal de LUMINANCIA de una copia de
trabajo reducida (lado mayor ~1024 px), para que los valores sean
comparables entre fotos de distinta resolución.

- Nitidez: `sharpness_raw` = varianza del Laplaciano. Convoluciona la
  luminancia con el kernel Laplaciano 3x3 y toma la varianza del resultado.
  Mayor varianza = más detalle de borde = más nítida.
- Exposición, a partir del histograma de luminancia:
  - clipped_highlights_pct = % de píxeles con luma >= 250
  - clipped_shadows_pct    = % de píxeles con luma <= 5
  - mean_luma              = luminancia media (0..255)
  - contrast               = desviación estándar de la luminancia
- La función principal recibe una image::DynamicImage y devuelve un
  QualityMetrics (el struct del módulo `record`).

Agrega unit tests con imágenes sintéticas: una imagen nítida debe dar mayor
sharpness_raw que su versión desenfocada; una imagen mayormente blanca debe
dar un clipped_highlights_pct alto; una mayormente negra, clipped_shadows_pct
alto. Verifica `cargo test`.
```

▲ FIN DEL PROMPT
