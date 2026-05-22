# WeHi — Reglas del proyecto

WeHi es una app de escritorio (Tauri 2) para fotografía EN MASA. Tiene dos
trabajos: CURAR (analizar, depurar, recomendar) y PROCESAR (redimensionar,
superponer marca de agua).

## Stack
- Shell: Tauri 2.x
- Frontend: Vite + React 18 + TypeScript + Tailwind CSS + Zustand
- Motor de proceso: crate `imgcore` — `image`, `fast_image_resize`, `rawler`
- Motor de curación: crate `imganalyze` — `blake3`, `tract`, `redb`, `rayon`

## Reglas permanentes
1. `imgcore` e `imganalyze` NO dependen de Tauri. Son Rust puro, testeables
   de forma aislada. `imganalyze` depende de `imgcore`; nunca al revés.
2. NUNCA cargar el lote completo en memoria. Cada imagen se procesa de
   principio a fin en su hilo y libera su buffer antes de la siguiente.
3. El logo y los modelos de IA se cargan UNA vez por lote, no por archivo.
4. La posición del logo se guarda en coordenadas normalizadas (0.0–1.0).
5. Un archivo corrupto NO detiene el lote ni el análisis. Los errores por
   archivo se acumulan y se reportan al final.
6. NUNCA borrado automático de fotos. WeHi propone; el usuario confirma. Los
   descartes se MUEVEN a una subcarpeta, no se eliminan.
7. Los puntajes son señales, no veredictos. La interfaz muestra los
   sub-puntajes; el usuario decide.
8. La interfaz soporta listas de miles de archivos sin trabarse (virtualizadas).
9. Los structs de configuración de Rust se replican en `src/types.ts`.
10. Todo módulo de `imgcore` e `imganalyze` lleva unit tests.
11. Antes de terminar cada tarea: `cargo build`, `cargo test` y `cargo clippy`
    deben pasar sin errores.
12. Sin formularios HTML `<form>` en React. Usar onClick / onChange.
