# Prompt 12 — Frontend: shell, estado y cola de archivos

**Objetivo:** base de la interfaz y la cola de entrada, capaz de manejar miles de archivos.
**Requisito previo:** Prompt 11 terminado y compilando.
**Entregables:** `types.ts`, store de Zustand, `DropZone`, lista virtualizada de la cola.

---

▼ INICIO DEL PROMPT

```
Construye la base del frontend de WeHi en React.

- Crea `src/types.ts` replicando exactamente los structs de configuración
  de `imgcore` (Preset, ResizeConfig, LogoConfig, LogoPosition,
  LogoBackground, OutputConfig y sus enums).
- Crea un store con Zustand para: archivos escaneados, archivo seleccionado
  para previsualizar, preset activo, biblioteca de logos, estado de la cola,
  progreso del lote.
- Componente `DropZone`: botón que abre el selector de carpetas con
  `@tauri-apps/plugin-dialog`, llama al comando `scan_folder` y guarda los
  archivos en el store.
- Componente `FileQueue`: muestra la lista de imágenes escaneadas y el
  conteo total. IMPORTANTE: la lista debe estar virtualizada para soportar
  miles de archivos sin trabarse. Cada fila muestra el nombre del archivo
  y su estado; al hacer clic en una fila, ese archivo queda seleccionado
  como muestra para la previsualización.
- Layout general: panel izquierdo (selección de carpeta + cola), panel
  central (previsualización, aún vacío), panel derecho (configuración del
  preset, aún vacío). Estilo limpio con Tailwind, modo claro, tipografía
  sobria.

No uses formularios HTML <form>. Verifica que al seleccionar una carpeta
con muchas fotos se muestre el conteo correcto y la lista se desplace
con fluidez.
```

▲ FIN DEL PROMPT
