# Prompt 17 — Frontend: procesamiento masivo y progreso

**Objetivo:** ejecutar el lote completo sobre la selección curada y mostrar el avance.
**Requisito previo:** Prompt 16 terminado y compilando.
**Entregables:** botón de procesar, suscripción a eventos, `ProgressPanel`.

---

▼ INICIO DEL PROMPT

```
Conecta el procesamiento masivo en el frontend de WeHi.

- Botón "Procesar lote": invoca el comando `process_batch` con el preset
  activo. Por defecto opera sobre la selección curada (las fotos con
  CullStatus = Keep, con opción de incluir también las Undecided). Si no se
  hizo la etapa de curación, opera sobre todos los archivos escaneados.
  Se deshabilita si no hay carpeta seleccionada o no hay preset.
- Suscríbete a los eventos de Tauri `progress`, `file_done` y `batch_done`.
- Componente `ProgressPanel`:
  - Barra de progreso general (procesadas / total) con porcentaje.
  - Nombre del archivo en proceso.
  - Lista de resultados por archivo (ok / error), virtualizada para
    soportar miles de filas.
- Botón "Cancelar" que llama a `cancel_batch`, visible solo durante el lote.
- Al recibir `batch_done`: mostrar un resumen final (total, exitosas,
  fallidas) y, si hay errores, una lista expandible con el detalle por
  archivo.
- Durante el procesamiento, la interfaz debe permanecer fluida (el trabajo
  ocurre en el backend; el frontend solo escucha eventos).

Verifica el flujo completo con un lote grande de fotos de prueba,
incluyendo algún archivo RAW.
```

▲ FIN DEL PROMPT
