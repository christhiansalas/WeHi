# Modelos ONNX

Coloca aquí los modelos ONNX que la capa de IA carga al activar
"Incluir análisis de IA" en la etapa Depurar:

- `aesthetic.onnx` — modelo de puntaje estético tipo NIMA.
- `faces.onnx` — modelo de detección de caras.
- `arcface.onnx` — modelo de reconocimiento facial (embeddings 512D).
  Habilita el agrupamiento por persona en la etapa Depurar. Sin este
  archivo, todas las demás funciones (estética, detección, scoring)
  siguen funcionando — solo `person_id` queda en `null`.
  Espera entrada NHWC `(1, 112, 112, 3)` normalizada a `(x-127.5)/128`
  y produce embedding `[1, 512]` que el motor L2-normaliza.
  Fuente probada: `huggingface.co/garavv/arcface-onnx/resolve/main/arc.onnx`
  (ResNet100 ArcFace, ~130 MB). Para descargar:
  `bash scripts/download-models.sh`.

Estos archivos se empaquetan dentro del binario gracias al patrón
`models/*.onnx` declarado en `bundle.resources` de
`tauri.conf.json`. Si los pones aquí antes de `npm run tauri build`,
quedan instalados junto con la app.

En tiempo de ejecución WeHi también busca en
`<app_data>/models/`, así que un usuario puede sustituir los modelos
sin reinstalar (la versión en `app_data` tiene prioridad).

Rutas típicas de `app_data` por sistema:

- **macOS** — `~/Library/Application Support/com.grupods.wehi/models/`
- **Windows** — `%APPDATA%\com.grupods.wehi\models\`
- **Linux** — `~/.local/share/com.grupods.wehi/models/`

Verifica que la licencia de cada modelo permita uso comercial antes
de empaquetar y distribuir.
