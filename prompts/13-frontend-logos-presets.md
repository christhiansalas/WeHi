# Prompt 13 — Frontend: biblioteca de logos y editor de presets

**Objetivo:** gestionar varios logos y configurar el preset, con accesos rápidos de posición.
**Requisito previo:** Prompt 12 terminado y compilando.
**Entregables:** `LogoManager`, `ResizeConfigForm`, `LogoConfigForm`, `OutputConfigForm`, gestión de presets.

---

▼ INICIO DEL PROMPT

```
Implementa la biblioteca de logos y el editor de presets en el panel
derecho de WeHi.

Componente `LogoManager`:
- Botón para importar un logo (selector de archivo PNG -> comando
  `add_logo`).
- Cuadrícula visual de los logos guardados (`list_logos`), cada uno con
  su miniatura y un botón para eliminar (`delete_logo`).
- Permite tener varios logos guardados; el usuario elige cuál usar en
  cada preset.

Editor de presets, con estos componentes:
- `ResizeConfigForm`: selector de modo (MaxDimension / Exact / Percentage)
  e inputs según el modo, más toggles de keep_aspect_ratio y allow_upscale.
- `LogoConfigForm` (toda esta sección es opcional, con un toggle que la
  activa/desactiva):
  - Selección del logo a usar, tomado de la biblioteca de logos.
  - ACCESOS RÁPIDOS DE POSICIÓN: una cuadrícula visual 3x3. Cada celda
    coloca el logo en esa zona (esquinas, centros de borde, centro)
    escribiendo el valor normalizado correspondiente en `position`.
    El posicionamiento libre por arrastre se implementa sobre la
    previsualización (Prompt 14); aquí la cuadrícula son solo atajos.
  - Sliders de `scale_pct` (tamaño del logo) y `opacity`.
  - Fondo opcional de la marca de agua: un toggle que activa LogoBackground,
    con selector de color, slider de opacidad del fondo y slider de
    `padding_pct`.
  - Al activar el logo por primera vez, usa una posición por defecto
    razonable (esquina inferior derecha).
- `OutputConfigForm`: selector de formato, slider de calidad, selector de
  carpeta de salida, input del patrón de nombre de archivo y toggle de
  preserve_structure.

Barra superior del editor: nombre del preset, botón Guardar (`save_preset`),
selector para cargar un preset existente (`list_presets`) y botón eliminar
(`delete_preset`).

Todo el estado vive en el store de Zustand. Verifica que guardar y volver
a cargar un preset conserve todos los valores, incluidos el logo elegido,
su posición y el fondo.
```

▲ FIN DEL PROMPT
