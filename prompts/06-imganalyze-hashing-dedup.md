# Prompt 06 — `imganalyze`: hashing y detección de repetidas

**Objetivo:** detectar duplicados exactos y casi-duplicados, y agruparlos.
**Requisito previo:** Prompt 05 terminado y compilando.
**Entregables:** módulos `hashing.rs` y `dedup.rs` con tests.

---

▼ INICIO DEL PROMPT

```
En `imganalyze`, implementa la detección de repetidas.

Módulo `hashing.rs`:
- `content_hash(path) -> [u8; 32]`: hash BLAKE3 del contenido del archivo.
  Detecta duplicados exactos (copias byte a byte).
- `perceptual_hash(img) -> u64`: dHash de 64 bits. Reduce la imagen a 9x8
  en escala de grises, compara cada píxel con su vecino horizontal (1 bit
  por comparación), produce 64 bits.
- `hamming_distance(a: u64, b: u64) -> u32`: XOR + popcount.

Módulo `dedup.rs`:
- Recibe la lista de AnalysisRecord del lote (cada uno ya con su
  perceptual_hash y su capture_time).
- Construye un grafo: una arista entre dos fotos si su distancia de Hamming
  es <= un umbral configurable (por defecto 10) Y su capture_time está
  dentro de una ventana temporal configurable (si ambas tienen EXIF; si
  falta el dato, solo se usa el umbral de Hamming).
- Calcula los componentes conexos del grafo con union-find (disjoint-set).
  Cada componente con más de un miembro es un grupo de repetidas; asigna
  su id al campo `duplicate_group` de cada AnalysisRecord del grupo.
- NOTA: la elección de la "ganadora" del grupo (is_group_winner) se hará en
  el módulo de puntaje (Prompt 09), porque depende del composite_score.

Agrega unit tests: dos copias idénticas dan el mismo content_hash; dos
imágenes casi iguales caen en el mismo grupo; dos imágenes distintas no.
Verifica `cargo test`.
```

▲ FIN DEL PROMPT
