# Prompt 14 — Frontend: previsualización en vivo y posicionamiento por arrastre

**Objetivo:** mostrar el resultado sobre una muestra y permitir arrastrar el logo libremente.
**Requisito previo:** Prompt 13 terminado y compilando.
**Entregables:** componente `PreviewPanel` con interacción de arrastre.

---

▼ INICIO DEL PROMPT

```
Implementa el panel central de previsualización de WeHi, con
posicionamiento del logo por arrastre.

Componente `PreviewPanel`:
- Toma el archivo seleccionado en la cola como imagen de muestra.
- Llama al comando `process_preview` con esa imagen y el preset activo,
  y muestra la imagen resultante (los bytes base64 que devuelve el
  comando). La previsualización pasa por el mismo motor que el lote:
  lo que se ve es exactamente lo que se obtendrá.
- Se actualiza automáticamente cuando cambia el preset. Aplica un debounce
  de ~300 ms en los cambios de sliders para no recalcular de más.
- Muestra debajo las dimensiones de salida resultantes.
- Incluye un interruptor "Original / Procesada" para comparar la muestra.
- Si no hay archivo seleccionado, muestra un mensaje invitando a elegir
  uno de la cola.

POSICIONAMIENTO DEL LOGO POR ARRASTRE:
- El logo mostrado sobre la previsualización es arrastrable con el mouse.
- Mientras se arrastra NO se llama al backend en cada movimiento. El
  frontend dibuja el logo (su imagen, con el tamaño y la opacidad actuales)
  como una capa superpuesta sobre la previsualización que sigue al cursor,
  para dar respuesta instantánea y fluida.
- Convierte las coordenadas del puntero a coordenadas normalizadas 0–1
  usando el rectángulo en pantalla de la imagen de previsualización.
- Al soltar (pointer up): escribe la nueva `position` (normalizada) en el
  store. Eso dispara una llamada a `process_preview` que refresca el
  resultado exacto, ya con el fondo y el blending reales del motor.
- El arrastre y la cuadrícula 3x3 de accesos rápidos escriben en la misma
  `position`: son dos formas de fijar el mismo dato.
- El logo no puede arrastrarse fuera de los límites de la imagen.

Verifica que al arrastrar el logo, o al usar la cuadrícula 3x3, la
previsualización refleje la nueva posición de la marca de agua.
```

▲ FIN DEL PROMPT
