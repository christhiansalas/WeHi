#!/usr/bin/env bash
# Post-build cross-platform:
# - macOS: limpia xattrs y re-firma ad-hoc el .app que dejó Tauri.
#   Sin esto macOS lo rechaza con "code has no resources but signature
#   indicates they must be present".
# - Linux/Windows: no-op (los bundlers de esas plataformas no tienen
#   el bug). El script se llama incondicionalmente desde npm.
#
# Uso (directo):
#   bash scripts/postbuild-mac.sh
# Uso (via npm):
#   npm run tauri:build

set -euo pipefail

# Solo aplica en macOS. En otros sistemas sale limpio.
if [ "$(uname -s)" != "Darwin" ]; then
  exit 0
fi

# Si CI ya firmó con un Developer ID real (APPLE_SIGNING_IDENTITY presente),
# NO sobreescribir con firma ad-hoc — eso invalidaría la firma de Apple
# y rompería la notarización.
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "→ APPLE_SIGNING_IDENTITY presente, dejando la firma de Apple intacta"
  exit 0
fi

APP_PATH="${1:-target/release/bundle/macos/WeHi.app}"

if [ ! -d "$APP_PATH" ]; then
  # No hay .app que firmar (build sin bundle de macos). Salida limpia.
  exit 0
fi

echo "→ Limpiando xattrs en $APP_PATH"
# dot_clean -m es lo único que limpia algunos xattrs raros que ni
# xattr -cr ni find+xattr quitan; sin esto codesign falla con
# "resource fork, Finder information, or similar detritus".
dot_clean -m "$APP_PATH" 2>/dev/null || true
find "$APP_PATH" -exec xattr -c {} \; 2>/dev/null || true

echo "→ Re-firmando ad-hoc el bundle completo"
codesign --force --deep --sign - "$APP_PATH"

echo "→ Verificando"
codesign --verify --verbose "$APP_PATH"

echo "✅ $APP_PATH listo."
