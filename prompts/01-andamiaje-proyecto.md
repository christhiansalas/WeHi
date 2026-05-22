# Prompt 01 — Andamiaje del proyecto

**Objetivo:** crear la estructura base del proyecto y dejarla arrancando.
**Requisito previo:** ninguno. El `CLAUDE.md` ya debe estar en la raíz del repo.
**Entregables:** proyecto Tauri 2 funcional, workspace de Cargo con `src-tauri` y `crates/imgcore`.

---

▼ INICIO DEL PROMPT

```
Crea el andamiaje del proyecto WeHi, una app de escritorio con Tauri 2.

Estructura requerida:
- Proyecto Tauri 2 con frontend Vite + React 18 + TypeScript.
- Tailwind CSS configurado.
- Un workspace de Cargo en la raíz con dos miembros:
  - `src-tauri` (la app Tauri)
  - `crates/imgcore` (el motor de imágenes, crate de librería)
- `src-tauri` declara `imgcore` como dependencia de ruta.

Configuración:
- En tauri.conf.json: productName "WeHi", identifier "com.grupods.wehi"
  (placeholder editable), ventana inicial 1100x720, redimensionable.
- Instala los plugins tauri-plugin-dialog y tauri-plugin-fs.

El crate imgcore por ahora solo necesita un lib.rs con una función dummy
y su test. Verifica que `cargo build` y `npm run tauri dev` arranquen sin error.
```

▲ FIN DEL PROMPT
