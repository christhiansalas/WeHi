# Modelos ONNX

Coloca aquí los modelos ONNX que la capa de IA carga al activar
"Incluir análisis de IA" en la etapa Depurar:

- `aesthetic.onnx` — modelo de puntaje estético tipo NIMA.
- `faces.onnx` — modelo de detección de caras.

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
