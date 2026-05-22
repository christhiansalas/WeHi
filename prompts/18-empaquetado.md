# Prompt 18 — Empaquetado y distribución

**Objetivo:** generar instaladores para Windows y macOS y documentar la firma.
**Requisito previo:** Prompt 17 terminado y la app funcionando de extremo a extremo.
**Entregables:** configuración de bundle, iconos, `DISTRIBUCION.md`.

---

▼ INICIO DEL PROMPT

```
Prepara WeHi para distribución en Windows y macOS.

- Configura el bundle en tauri.conf.json:
  - Windows: instalador NSIS y/o MSI.
  - macOS: DMG.
- Asegúrate de que los modelos ONNX de la capa de IA se incluyan como
  recursos empaquetados con la app, y que la app los localice tanto en
  desarrollo como ya instalada.
- Genera y enlaza los iconos de la app en todos los tamaños requeridos
  a partir de un PNG fuente (deja el PNG como placeholder).
- Crea un archivo `DISTRIBUCION.md` que documente:
  - Comandos de build (`npm run tauri build`) y dónde quedan los binarios.
  - Pasos para firmar la app en Windows y para notarizar en macOS, y qué
    pasa si se distribuye sin firmar (advertencia de SmartScreen /
    Gatekeeper, y cómo abrirla de todos modos).
- Opcional: integra el plugin `tauri-plugin-updater` y documenta cómo
  publicar actualizaciones.

Verifica que `npm run tauri build` genere los instaladores correctamente.
```

▲ FIN DEL PROMPT
