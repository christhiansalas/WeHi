# Prompt 10 — Capa Tauri: comandos de procesamiento, cola masiva y eventos

**Objetivo:** exponer el motor de procesamiento a la interfaz y orquestar el lote.
**Requisito previo:** Prompt 09 terminado y compilando.
**Entregables:** `commands.rs`, `queue.rs`, comandos registrados en `main.rs`.

---

▼ INICIO DEL PROMPT

```
En `src-tauri`, implementa la orquestación del procesamiento sobre `imgcore`.

Módulo `commands.rs` con estos comandos `#[tauri::command]`:

Escaneo y procesamiento:
- `scan_folder(path)` -> escaneo recursivo; devuelve la lista de rutas de
  imágenes con extensión jpg, jpeg, png, webp, cr2, cr3, nef, nrw.
- `process_batch(files, preset)` -> procesa el lote en un hilo de fondo.
  Si el preset tiene logo, cárgalo UNA vez con imgcore::load_logo antes de
  empezar. Paraleliza con `rayon` (par_iter) sobre los archivos; cada uno
  llama a imgcore::process_file. Un error no detiene el lote.
- `process_preview(file, preset)` -> procesa UN solo archivo con
  imgcore::process_image y devuelve los bytes resultantes como base64,
  para mostrarlo en la previsualización. Debe ser rápido.
- `cancel_batch()` -> cancela el lote en curso de forma segura
  (AtomicBool compartido).

Gestión de logos (biblioteca de la app):
- `list_logos()` -> lista de logos guardados (id, nombre, ruta).
- `add_logo(source_path)` -> copia el archivo de logo al directorio
  app-data/logos, le asigna un id y lo registra.
- `delete_logo(id)` -> elimina un logo de la biblioteca.

Gestión de presets:
- `list_presets()`, `save_preset(preset)`, `delete_preset(name)` ->
  los presets se guardan como JSON en el directorio de datos de la app.

Eventos emitidos durante `process_batch`:
- `progress` -> { procesadas, total, archivo_actual }
- `file_done` -> { ruta, ok, error }
- `batch_done` -> { total, exitosas, fallidas, errores: [...] }

Permite configurar el número máximo de hilos del pool de rayon.
Registra todos los comandos en main.rs. Verifica que `cargo build` pase.
```

▲ FIN DEL PROMPT
