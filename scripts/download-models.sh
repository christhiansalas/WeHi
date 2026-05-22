#!/usr/bin/env bash
# Descarga los modelos ONNX que WeHi usa para análisis IA.
#
# Los modelos NO se commitean al repo (van en .gitignore). Corre este
# script una vez después de clonar o si quieres re-bajarlos:
#
#   bash scripts/download-models.sh
#
# Cada modelo es opcional: si falla la descarga, el motor de IA omite
# esa funcionalidad y el resto del análisis sigue corriendo.

set -euo pipefail

cd "$(dirname "$0")/.."
DEST="src-tauri/models"
mkdir -p "$DEST"

download() {
  local name="$1"; local url="$2"
  local out="$DEST/$name"
  if [ -f "$out" ]; then
    echo "✓ $name ya existe ($(du -h "$out" | cut -f1))"
    return 0
  fi
  echo "→ Descargando $name ..."
  if curl -fL --progress-bar -o "$out.partial" "$url"; then
    mv "$out.partial" "$out"
    echo "✓ $name listo ($(du -h "$out" | cut -f1))"
  else
    rm -f "$out.partial"
    echo "⚠ $name falló — la app seguirá funcionando sin esa capacidad"
  fi
}

# ArcFace ResNet100 (NHWC, 130 MB). Reconocimiento facial → person_id.
download "arcface.onnx" \
  "https://huggingface.co/garavv/arcface-onnx/resolve/main/arc.onnx"

# aesthetic.onnx y faces.onnx — el usuario los provee según sus
# preferencias de licencia. Cualquier modelo NIMA y BlazeFace/UltraFace
# en formato ONNX (NCHW) compatible con tract 0.21 sirve.

echo ""
echo "Listo. Para verificar la carga:"
echo "  cargo run -p imganalyze --example smoke_arcface"
