# Prompt 03 — `imgcore`: decodificación (estándar + RAW), EXIF y redimensionado

**Objetivo:** decodificar todos los formatos de entrada —incluido RAW de Canon/Nikon— y redimensionar.
**Requisito previo:** Prompt 02 terminado y compilando.
**Entregables:** módulos `exif.rs`, `raw.rs`, `resize.rs`, `decode.rs` con tests.

---

▼ INICIO DEL PROMPT

```
En `imgcore`, implementa la decodificación y el redimensionado.

Módulo `exif.rs`:
- Función que lee la orientación EXIF de un archivo (usa `kamadak-exif`)
  y aplica la rotación/volteo a una image::DynamicImage. Cubre los 8 casos.
  Si no hay EXIF, devuelve la imagen sin cambios.

Módulo `raw.rs`:
- Función que detecta si una ruta es RAW por su extensión
  (.cr2, .cr3, .nef, .nrw — sin distinción de mayúsculas/minúsculas).
- Función que, dado un archivo RAW, extrae el JPEG de previsualización
  embebido y lo devuelve como bytes. Usa el crate `rawler`.
  NO desarrolles el RAW (sin demosaicing): solo extrae la previsualización.
- Mantén el módulo aislado tras una interfaz simple
  (entrada: ruta; salida: Result<Vec<u8>, ImgError>) para poder cambiar
  el motor RAW más adelante sin tocar el resto del crate.

Módulo `resize.rs`:
- Función que recibe una DynamicImage y un ResizeConfig y devuelve la
  imagen redimensionada usando `fast_image_resize` con filtro Lanczos3.
- Implementa los tres modos:
  - MaxDimension: la imagen cabe dentro de un cuadro width x height
    respetando proporción.
  - Exact: dimensiones exactas (respeta keep_aspect_ratio si está activo).
  - Percentage: escala relativa.
- Si allow_upscale es false, nunca agrandar más allá del tamaño original.

Módulo `decode.rs`:
- Función `decode(path) -> Result<DynamicImage, ImgError>` que despacha
  según el tipo de archivo:
  - Estándar (jpg, jpeg, png, webp): decodifica con el crate `image`.
  - RAW (cr2, cr3, nef, nrw): extrae el JPEG embebido con raw.rs y
    decodifícalo con `image`.
  - Extensión desconocida: ImgError de formato no soportado.
  - En ambos casos aplica la orientación EXIF al final.

Agrega unit tests para resize.rs (genera imágenes de prueba en memoria
y verifica las dimensiones de salida en cada modo). Si tienes un archivo
RAW de prueba, agrega un test que verifique que decode() devuelve una
imagen válida desde un .CR3 o .NEF. Verifica `cargo test`.
```

▲ FIN DEL PROMPT
