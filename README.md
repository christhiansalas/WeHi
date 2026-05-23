# WeHi

App de escritorio para fotografía **en masa**. Tiene dos mitades:

- **Procesar** — redimensiona y aplica marca de agua a un lote completo.
- **Curar** — analiza, depura y recomienda fotos por red social.

Flujo de uso: Cargar → (Depurar → Recomendar) → Procesar.
La curación es opcional; se puede ir de Cargar directo a Procesar.

## Stack

- **Shell de escritorio:** Tauri 2.
- **Frontend:** Vite + React 18/19 + TypeScript + Tailwind CSS + Zustand.
- **Motor de proceso:** crate Rust `imgcore` — `image`, `fast_image_resize`,
  `rawler`, `kamadak-exif`, `webp`.
- **Motor de curación:** crate Rust `imganalyze` — `blake3`, `tract`, `rayon`,
  `chrono`.
- **Caché embebido:** `redb` en `app_data/cache/`.

## Arquitectura

```
WeHi/
├── Cargo.toml                  workspace Cargo (src-tauri + 2 crates)
├── tauri.conf.json             config del bundle (en src-tauri/)
├── package.json                deps de npm
├── index.html · vite.config.ts
├── tailwind.config.js · postcss.config.js
├── prompts/                    18 prompts del plan de construcción
├── DISTRIBUCION.md             empaquetado y firma
├── CLAUDE.md                   reglas permanentes del proyecto
│
├── crates/
│   ├── imgcore/                Rust puro — procesamiento (sin Tauri)
│   │   └── src/
│   │       ├── config.rs       Preset, ResizeConfig, LogoConfig, …
│   │       ├── error.rs        ImgError (thiserror)
│   │       ├── exif.rs         lectura y aplicación de orientación
│   │       ├── raw.rs          extracción de JPEG embebido (CR2/CR3/NEF/NRW)
│   │       ├── decode.rs       decode(path) — estándar + RAW
│   │       ├── resize.rs       fast_image_resize (Lanczos3)
│   │       ├── overlay.rs      compose_logo con fondo opcional
│   │       ├── encode.rs       JPEG / PNG / WebP
│   │       └── pipeline.rs     process_image / process_file
│   │
│   └── imganalyze/             Rust puro — curación (depende de imgcore)
│       └── src/
│           ├── record.rs       AnalysisRecord, QualityMetrics, …
│           ├── hashing.rs      BLAKE3 + dHash + Hamming
│           ├── dedup.rs        union-find por hash perceptual + ventana
│           │                   temporal EXIF
│           ├── quality.rs      Laplaciano + histograma de luminancia
│           ├── ai.rs           AiEngine ONNX enchufable (tract)
│           ├── scoring.rs      sub-scores + composite + ganadora de grupo
│           ├── social.rs       NetworkSpec + format_fit + recommendation
│           └── analyze.rs      orquestación (par_iter con rayon)
│
├── src-tauri/                  capa Tauri (comandos + estado)
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/default.json
│   ├── icons/                  iconos del bundle (placeholders)
│   ├── models/                 modelos ONNX (bundleados; ver README)
│   └── src/
│       ├── main.rs
│       ├── lib.rs              registro de comandos
│       ├── state.rs            AppState · AnalysisState
│       ├── commands.rs         escaneo, procesamiento, logos, presets,
│       │                       miniaturas, data-url helpers
│       ├── queue.rs            run_batch con eventos
│       ├── cache.rs            redb (clave BLAKE3, schema+model version)
│       └── analysis.rs         analyze_batch + caché + dedup + scoring
│
└── src/                        frontend React
    ├── main.tsx · App.tsx · index.css · vite-env.d.ts
    ├── types.ts                réplica TS de los structs Rust
    ├── store.ts                Zustand
    ├── hooks/useBatchEvents.ts suscripción a eventos del lote
    └── components/
        ├── DropZone.tsx        elegir carpeta + scan_folder
        ├── FileQueue.tsx       cola virtualizada (@tanstack/react-virtual)
        ├── LogoManager.tsx     biblioteca de logos
        ├── ResizeConfigForm.tsx
        ├── LogoConfigForm.tsx  cuadrícula 3×3 de posiciones rápidas
        ├── OutputConfigForm.tsx
        ├── PresetEditor.tsx    guarda/carga/elimina presets
        ├── PreviewPanel.tsx    process_preview + arrastre del logo
        ├── ProcessControls.tsx start/cancel del lote
        ├── ProgressPanel.tsx   progreso + resultados virtualizados
        ├── AnalyzePanel.tsx    etapa Depurar (revisión + pesos)
        └── RecommendPanel.tsx  etapa Recomendar (recorte por red)
```

## Reglas permanentes del diseño

(Espejo de [`CLAUDE.md`](CLAUDE.md) — leer antes de modificar.)

1. `imgcore` e `imganalyze` NO dependen de Tauri. `imganalyze` depende de
   `imgcore`; nunca al revés.
2. Nunca se carga el lote completo en memoria. Cada imagen se procesa de
   principio a fin en su hilo y libera su buffer antes de la siguiente.
3. El logo y los modelos de IA se cargan UNA vez por lote, no por archivo.
4. La posición del logo se guarda en coordenadas normalizadas (0.0–1.0)
   del CENTRO.
5. Un archivo corrupto NO detiene el lote ni el análisis. Los errores por
   archivo se acumulan y se reportan al final.
6. NUNCA borrado automático de fotos. WeHi propone; el usuario confirma.
   Los descartes se MUEVEN a una subcarpeta, no se eliminan.
7. Los puntajes son señales, no veredictos. La interfaz muestra los
   sub-puntajes; el usuario decide.
8. Las listas soportan miles de archivos sin trabarse (virtualizadas).
9. Los structs de configuración de Rust se replican en `src/types.ts`.
10. Sin formularios HTML `<form>` en React. Usar `onClick` / `onChange`.

## Comandos Tauri

Registrados en `src-tauri/src/lib.rs`:

**Procesamiento**
- `scan_folder(path) -> string[]` — escanea recursivamente
  jpg/jpeg/png/webp/cr2/cr3/nef/nrw.
- `process_batch(files, preset)` — lote en background. Eventos:
  `progress`, `file_done`, `batch_done`.
- `process_preview(file, preset) -> { base64, width, height, format }` —
  previsualización.
- `cancel_batch()` · `set_max_threads(max)`.

**Biblioteca de logos**
- `list_logos()` · `add_logo(sourcePath)` · `delete_logo(id)` ·
  `get_logo_data_url(id)`.

**Presets**
- `list_presets()` · `save_preset(preset)` · `delete_preset(name)`.

**Análisis / curación**
- `analyze_batch(files, useAi)` — análisis con caché redb. Eventos:
  `analysis_progress`, `analysis_done`.
- `cancel_analysis()` · `get_analysis_results()` ·
  `set_cull_status(path, status)`.
- `list_networks()` · `get_recommendations(network)`.
- `move_rejects(targetSubfolder) -> { movidos, errores }`.

**Helpers de imagen**
- `get_file_data_url(file)` — original (RAW = JPEG embebido).
- `get_thumbnail_data_url(file, size)` — miniatura JPEG q70.

## Desarrollo

Requisitos:
- **Rust** ≥ 1.75 estable (`rustup install stable`)
- **Node** ≥ 18 (`brew install node` o `nvm install --lts`)
- **ffmpeg** en el PATH — necesario para procesar y previsualizar videos.
  Sin él, las funciones de video fallan silenciosamente. En macOS:
  `brew install ffmpeg` o `pip install static-ffmpeg`.
- **macOS:** Xcode Command Line Tools (`xcode-select --install`)
- **Linux:** `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libssl-dev libayatana-appindicator3-dev`

```bash
npm install
bash scripts/download-models.sh   # opcional: baja arcface.onnx para personas
npm run tauri dev                 # arranca la app de escritorio
```

### Verificación

```bash
cargo build                                          # workspace completo
cargo test --workspace                               # 75+ unit tests
cargo clippy --workspace --all-targets -- -D warnings
npx tsc --noEmit                                     # frontend
npm run build                                        # bundle vite
```

### Modelos de IA (opcional)

Coloca los `.onnx` en `src-tauri/models/`:
- `aesthetic.onnx` — puntaje estético tipo NIMA.
- `faces.onnx` — detección de caras.
- `arcface.onnx` — embeddings para reconocer y agrupar personas en el lote.
  Bajar con `bash scripts/download-models.sh`.

Quedan empaquetados con el bundle. El usuario también puede sustituirlos
en `<app_data>/models/` sin reinstalar (esa ruta tiene prioridad). Ver
[`src-tauri/models/README.md`](src-tauri/models/README.md).

Sin modelos, la opción "Incluir análisis de IA" en Depurar produce
registros con `aesthetic`, `faces` y `person_id` en `null` (las métricas
objetivas siguen funcionando).

### Distribución

Releases automáticos en GitHub cuando se empuja un tag `v*`:

```bash
git tag v0.2.0 && git push origin v0.2.0
```

CI compila los 3 OS, firma + notariza el `.dmg` de macOS con tu
Developer ID (vía secrets `APPLE_*`) y publica el Release con URLs
estables `https://github.com/<user>/<repo>/releases/latest/download/WeHi_<OS>.<ext>`.
Ver [`DISTRIBUCION.md`](DISTRIBUCION.md).

### Licencia

Por definir. Cualquier modelo ONNX agregado en `src-tauri/models/`
hereda la licencia de su origen — verifica antes de redistribuir.

## Pipeline en cifras

- Pre-marca automática como Reject:
  - `sub_scores.sharpness < 0.20`, o
  - `sub_scores.exposure < 0.25`, o
  - `duplicate_group is Some` y no es la ganadora.
- Pesos por defecto del composite (editables en vivo):
  - `sharpness 0.30` · `exposure 0.22` · `aesthetic 0.38` · `resolution 0.10`
  - `face_bonus_max 0.10` (siempre suma, nunca penaliza).
- Caché `redb` versionado por separado:
  - `schema_version` (métricas objetivas).
  - `model_version` (hash de los archivos ONNX).
  - Si cambia solo `model_version`, se reusan las métricas y se
    recalcula SOLO la parte de IA.

## Estado de los 18 prompts

| # | Etapa | Estado |
|---|---|---|
| 01–04 | `imgcore` (config, decode, RAW, EXIF, resize, marca de agua, pipeline) | ✅ |
| 05–09 | `imganalyze` (record, hashing/dedup, calidad, IA, scoring/social/analyze) | ✅ |
| 10 | Tauri: comandos de procesamiento + cola + eventos | ✅ |
| 11 | Tauri: análisis con caché redb | ✅ |
| 12 | Frontend: shell + cola virtualizada | ✅ |
| 13 | Frontend: biblioteca de logos + editor de presets | ✅ |
| 14 | Frontend: previsualización + arrastre del logo | ✅ |
| 15 | Frontend: etapa Depurar | ✅ |
| 16 | Frontend: etapa Recomendar | ✅ |
| 17 | Frontend: procesamiento masivo + progreso | ✅ |
| 18 | Empaquetado y distribución (ver [`DISTRIBUCION.md`](DISTRIBUCION.md)) | ✅ |

## Verificación actual

- `cargo build --workspace` — ✅
- `cargo test --workspace` — **75 tests pasan** (43 imganalyze + 32 imgcore).
- `cargo clippy --workspace --all-targets -- -D warnings` — limpio.
- `npx tsc --noEmit` — sin errores de tipos.
- `npm run build` — bundle de 254 KB / **gzip 77 KB**, 55 módulos.
- Sin TODOs/FIXMEs/console.log en código del proyecto.
