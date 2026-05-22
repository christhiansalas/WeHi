# WeHi — Índice de la secuencia de construcción

18 prompts para construir WeHi con Claude Code. Cada prompt está en su propio
archivo; ábrelo, copia el bloque entre `▼ INICIO DEL PROMPT` y `▲ FIN DEL
PROMPT`, y pégalo en Claude Code.

## Antes de empezar
Copia `CLAUDE.md` a la raíz del repositorio. Claude Code lo lee de forma
automática y aplica sus reglas en todos los prompts.

## El producto
WeHi tiene dos mitades:
- **Procesar** — redimensionar y poner marca de agua en masa (crate `imgcore`).
- **Curar** — analizar, depurar y recomendar fotos (crate `imganalyze`).

Flujo de uso final: Cargar → Analizar → Depurar → Recomendar → Procesar.
La curación es opcional; se puede ir de Cargar directo a Procesar.

## Orden de los prompts

| # | Archivo | Construye |
|---|---|---|
| 01 | `01-andamiaje-proyecto.md` | Andamiaje Tauri 2 + workspace de Cargo |
| 02 | `02-imgcore-tipos.md` | `imgcore`: tipos de configuración |
| 03 | `03-imgcore-decodificacion.md` | `imgcore`: decodificación (estándar + RAW), EXIF, resize |
| 04 | `04-imgcore-marca-de-agua.md` | `imgcore`: marca de agua, codificación, pipeline |
| 05 | `05-imganalyze-andamiaje.md` | Crate `imganalyze` + modelo de datos del análisis |
| 06 | `06-imganalyze-hashing-dedup.md` | `imganalyze`: hashing y detección de repetidas |
| 07 | `07-imganalyze-calidad.md` | `imganalyze`: métricas de calidad objetivas |
| 08 | `08-imganalyze-ia.md` | `imganalyze`: capa de IA (estética + caras) |
| 09 | `09-imganalyze-puntaje.md` | `imganalyze`: puntaje, ranking, recomendación, orquestación |
| 10 | `10-tauri-procesamiento.md` | Capa Tauri: comandos de procesamiento y cola |
| 11 | `11-tauri-analisis.md` | Capa Tauri: comandos de análisis y caché |
| 12 | `12-frontend-shell.md` | Frontend: shell, estado, cola de archivos |
| 13 | `13-frontend-logos-presets.md` | Frontend: biblioteca de logos y editor de presets |
| 14 | `14-frontend-previsualizacion.md` | Frontend: previsualización y arrastre del logo |
| 15 | `15-frontend-depurar.md` | Frontend: etapa Depurar / Revisar |
| 16 | `16-frontend-recomendacion.md` | Frontend: recomendación por red social |
| 17 | `17-frontend-procesamiento.md` | Frontend: procesamiento masivo y progreso |
| 18 | `18-empaquetado.md` | Empaquetado y distribución |

## Notas de uso
- **Orden estricto.** Ejecuta del 01 al 18 sin saltar. Cada prompt asume el
  estado del anterior.
- **No acumules deuda.** Si un prompt deja algo sin compilar, pide la
  corrección antes de avanzar. Los prompts 02–09 (los dos motores) son los
  más críticos.
- **Identifier de macOS.** Ajusta `com.grupods.wehi` al valor oficial antes
  del prompt 18; queda amarrado a la app al firmarla.
- Los prompts del mismo bloque (p. ej. 13 y 14, o 15 y 16) pueden hacerse en
  una sola sesión si quieres ir más rápido.

## Lo que necesitas conseguir tú
- **Modelos ONNX para la IA** (prompt 08): un modelo de estética tipo NIMA y
  un modelo de detección de caras, en formato ONNX. Consigue los archivos y
  verifica que su licencia permita uso comercial. El código los trata como
  recurso enchufable.
- **Fotos de prueba RAW** de los cuerpos de cámara de tu equipo (Canon/Nikon),
  y fotos variadas (nítidas, borrosas, ráfagas, con y sin caras) para validar
  los prompts 03, 06–09 y 15.
