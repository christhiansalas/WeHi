# Prompt 02 — `imgcore`: tipos de configuración

**Objetivo:** definir el modelo de datos de los presets, con posición de logo normalizada y fondo opcional.
**Requisito previo:** Prompt 01 terminado y compilando.
**Entregables:** módulos `config.rs` y `error.rs` en `imgcore`, con tests de serialización.

---

▼ INICIO DEL PROMPT

```
En el crate `imgcore`, crea el módulo `config.rs` con el modelo de datos
de los presets. Todos los structs y enums derivan Serialize, Deserialize,
Clone y Debug.

Enums:
- ResizeMode: MaxDimension, Exact, Percentage
- OutputFormat: Jpeg, Png, WebP

Structs:
- ResizeConfig { mode, width: Option<u32>, height: Option<u32>,
  percentage: Option<f32>, keep_aspect_ratio: bool, allow_upscale: bool }
- LogoPosition { x: f32, y: f32 }
  (coordenadas NORMALIZADAS 0.0–1.0 del CENTRO del logo respecto a la
   imagen base; se guardan normalizadas para que la misma posición
   funcione igual en fotos de distintas dimensiones dentro de un lote)
- LogoBackground { color_rgba: [u8; 4], padding_pct: f32 }
  (recuadro opcional detrás del logo; el canal alfa de color_rgba controla
   la translucidez del fondo)
- LogoConfig { logo_id: String, position: LogoPosition, scale_pct: f32,
  opacity: f32, background: Option<LogoBackground> }
  (logo_id referencia un logo guardado en la biblioteca de la app;
   scale_pct = ancho del logo como % del ancho de la imagen base)
- OutputConfig { format: OutputFormat, quality: u8, folder: PathBuf,
  filename_pattern: String, preserve_structure: bool }
- Preset { name: String, resize: ResizeConfig, logo: Option<LogoConfig>,
  output: OutputConfig }

Crea también `error.rs` con un enum `ImgError` usando `thiserror`, cubriendo:
error de E/S, error de decodificación, formato no soportado, logo no
encontrado, fallo al extraer la previsualización RAW.

Implementa Default para Preset con valores sensatos (allow_upscale = false,
quality = 85, format = Jpeg, sin logo).

Agrega unit tests que verifiquen el round-trip de serialización JSON de un
Preset con y sin logo, y con y sin fondo de logo.
```

▲ FIN DEL PROMPT
