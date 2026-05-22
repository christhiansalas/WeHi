# Prompt 08 — `imganalyze`: capa de IA (estética y caras)

**Objetivo:** ejecutar modelos ONNX para puntaje estético y detección de caras.
**Requisito previo:** Prompt 07 terminado y compilando.
**Entregables:** módulo `ai.rs` con la estructura preparada para los modelos.

> **Antes de este prompt necesitas conseguir tú:** un modelo de estética tipo
> NIMA y un modelo de detección de caras, ambos en formato ONNX, con licencia
> apta para uso comercial. El código los trata como recurso enchufable.

---

▼ INICIO DEL PROMPT

```
En `imganalyze`, implementa la capa de IA en el módulo `ai.rs`, usando el
crate `tract` para ejecutar modelos ONNX en CPU.

La capa usa dos modelos ONNX cargados como archivos externos; sus rutas son
configurables. Trátalos como recurso enchufable.
- Un modelo de estética tipo NIMA.
- Un modelo de detección de caras.

Estructura:
- `AiEngine`: carga ambos modelos UNA sola vez y construye sus planes
  ejecutables (inmutables). Si un archivo de modelo no está disponible, la
  creación falla de forma controlada, de modo que el análisis pueda continuar
  sin IA (los campos aesthetic y faces quedan en None).
- Estética: preprocesa la imagen al tensor de entrada del modelo (RGB
  normalizado, al tamaño que el modelo espera, p. ej. 224x224), ejecuta la
  inferencia, e interpreta la salida como una distribución de probabilidad
  sobre 10 categorías (calificación 1..10). Calcula:
    mean    = suma de i * p_i  para i = 1..10
    std_dev = dispersión de esa distribución
  Devuelve un AestheticScore { mean, std_dev }.
- Caras: ejecuta el detector, interpreta las cajas delimitadoras y su
  confianza, y deriva un FaceSummary { count, largest_face_frac,
  main_face_centered }.

Documenta en comentarios qué forma de tensor de entrada y de salida espera
cada modelo, para que sustituir un modelo sea sencillo.

Agrega tests estructurados para correr si hay un modelo de prueba disponible,
y que no fallen si no lo hay. Verifica `cargo build`.
```

▲ FIN DEL PROMPT
