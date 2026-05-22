# Prompt 04 — `imgcore`: marca de agua, codificación y pipeline completo

**Objetivo:** componer el logo en posición normalizada (con fondo opcional), codificar y unir todo el pipeline.
**Requisito previo:** Prompt 03 terminado y compilando.
**Entregables:** módulos `overlay.rs`, `encode.rs`, `pipeline.rs` con tests de integración.

---

▼ INICIO DEL PROMPT

```
En `imgcore`, completa el motor de procesamiento.

Módulo `overlay.rs`:
- Tipo `LoadedLogo` que representa un logo ya decodificado en memoria,
  más una función `load_logo(path) -> Result<LoadedLogo, ImgError>`.
  Esto permite cargar el logo UNA sola vez y reutilizarlo en todo el lote.
- Función que compone un LoadedLogo sobre una imagen base según un
  LogoConfig:
  - Reescala el logo a `scale_pct` del ancho de la imagen base.
  - Calcula la posición en píxeles a partir de `position` (el centro
    normalizado 0–1 del logo) y del tamaño ya reescalado del logo.
  - Restringe (clamp) la caja del logo para que quede SIEMPRE
    completamente dentro de la imagen base, incluso si `position` la
    empujaría fuera.
  - Si `background` está presente: dibuja primero un recuadro detrás del
    logo, del color y translucidez de color_rgba, con `padding_pct` de
    relleno alrededor del logo.
  - Mezcla el logo aplicando `opacity` (blending de alfa).

Módulo `encode.rs`:
- Función que codifica una imagen a bytes según un OutputConfig:
  - Jpeg: con la calidad indicada.
  - Png: sin pérdida.
  - WebP: lossy con calidad, usando el crate `webp`.

Módulo `pipeline.rs`:
- Función `process_image(input: &Path, preset: &Preset,
  logo: Option<&LoadedLogo>) -> Result<EncodedImage, ImgError>` que ejecuta
  la cadena completa: decodificar -> redimensionar -> componer logo (si
  aplica) -> codificar. Devuelve los bytes codificados, sin escribir a disco.
- Función `process_file(input: &Path, preset: &Preset,
  logo: Option<&LoadedLogo>) -> Result<(), ImgError>` que llama a
  process_image, resuelve la ruta de salida (patrón de nombre +
  preserve_structure) y escribe el archivo.
- Libera todos los buffers al terminar cada imagen.

NOTA: separar process_image (devuelve bytes) de process_file (escribe)
es intencional: la previsualización usará process_image y el lote usará
process_file, garantizando que ambos pasan por el mismo motor.

Agrega un test de integración que procese una imagen real de prueba con
un preset que incluya logo y fondo, y verifique que el resultado es válido.
Verifica `cargo test`.
```

▲ FIN DEL PROMPT
