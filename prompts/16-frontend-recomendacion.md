# Prompt 16 — Frontend: recomendación por red social

**Objetivo:** mostrar las mejores fotos para cada red social, con vista del recorte.
**Requisito previo:** Prompt 15 terminado y compilando.
**Entregables:** vista de recomendación por red social.

---

▼ INICIO DEL PROMPT

```
Implementa la vista de recomendación por red social en WeHi.

- Selector de red social (Instagram feed, Stories/Reels/TikTok, Facebook
  feed, miniatura YouTube; la lista de redes viene del backend).
- Al elegir una red, invoca el comando `get_recommendations(network)` y
  muestra las fotos ordenadas por su puntaje de recomendación.
- Cada foto muestra una vista previa del recorte a la relación de aspecto de
  esa red, para que se vea cómo quedaría publicada.
- Permite marcar una selección de fotos recomendadas y exportar esa lista
  (por ejemplo, para usarla luego en la etapa de procesamiento).
- Deja visible en la interfaz que la recomendación se basa en calidad
  técnica y encaje de formato, NO en predicción de alcance o viralidad.

Verifica que al cambiar de red social, las recomendaciones y las vistas de
recorte se actualicen correctamente.
```

▲ FIN DEL PROMPT
