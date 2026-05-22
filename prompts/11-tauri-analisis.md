# Prompt 11 — Capa Tauri: comandos de análisis y caché

**Objetivo:** exponer la curación a la interfaz, con caché de resultados.
**Requisito previo:** Prompt 10 terminado y compilando.
**Entregables:** módulo de caché `redb`, comandos de análisis registrados en `main.rs`.

---

▼ INICIO DEL PROMPT

```
En `src-tauri`, expone la curación a la interfaz, sobre el crate `imganalyze`.

Módulo de caché (usa el crate `redb`):
- Almacén embebido redb en el directorio de datos de la app.
- Clave de cada registro: BLAKE3 de (ruta absoluta + fecha de modificación +
  tamaño del archivo).
- Versiona por separado las métricas objetivas (un schema_version) y los
  resultados de IA (un model_version). Si cambia el modelo de IA, se
  recalcula SOLO la parte de IA; las métricas objetivas se reutilizan del
  caché.

Comandos `#[tauri::command]`:
- `analyze_batch(files, use_ai)` -> corre imganalyze sobre los archivos en un
  hilo de fondo. Para cada foto consulta el caché primero; solo analiza lo
  nuevo, lo modificado o lo que cambió de modelo de IA. Si use_ai es true,
  carga el AiEngine UNA sola vez antes de empezar. Emite eventos de progreso.
- `cancel_analysis()` -> cancela el análisis en curso (AtomicBool).
- `get_analysis_results()` -> devuelve los AnalysisRecord del lote actual.
- `set_cull_status(path, status)` -> actualiza el CullStatus de una foto.
- `get_recommendations(network)` -> devuelve las fotos del lote ordenadas por
  su puntaje de recomendación para una red social.
- `move_rejects(target_subfolder)` -> MUEVE (nunca elimina) los archivos con
  status Reject a una subcarpeta. Devuelve un reporte de lo movido.

Eventos emitidos durante `analyze_batch`:
- `analysis_progress` -> { analizadas, total, archivo_actual, etapa }
- `analysis_done` -> { total, analizadas, desde_cache, errores: [...] }

Registra todos los comandos en main.rs. Verifica que `cargo build` pase.
```

▲ FIN DEL PROMPT
