# WeHi — Distribución

Guía para compilar, firmar y publicar WeHi en Windows y macOS.

## Antes de empezar

- Coloca los modelos ONNX en `src-tauri/models/`:
  - `aesthetic.onnx`
  - `faces.onnx`
  - Quedan empaquetados automáticamente (`bundle.resources`).
- Verifica el `identifier` en `src-tauri/tauri.conf.json`. El valor
  actual es `com.grupods.wehi` (placeholder); cámbialo al definitivo
  ANTES de firmar la primera versión: el identifier queda amarrado
  al certificado y a las preferencias del sistema operativo.

## Iconos

El scaffold incluye los iconos placeholder en `src-tauri/icons/`.
Para regenerarlos a partir de un PNG fuente cuadrado (mínimo
1024×1024):

```bash
npx @tauri-apps/cli icon ruta/al/icono-fuente.png
```

Sobrescribe los `.png`, `.icns` y `.ico` en `src-tauri/icons/`.

## Compilar

```bash
npm install
npm run tauri build
```

### Dónde quedan los binarios

- **macOS** — `src-tauri/target/release/bundle/`
  - `macos/WeHi.app/` (paquete .app)
  - `dmg/WeHi_<version>_<arch>.dmg` (instalador)
- **Windows** — `src-tauri/target/release/bundle/`
  - `nsis/WeHi_<version>_<arch>-setup.exe`
  - `msi/WeHi_<version>_<arch>_en-US.msi`
- **Linux** (no es objetivo oficial) — `appimage/` / `deb/`.

Tauri compila solo para la plataforma anfitriona: para Windows
compila desde Windows; para macOS desde macOS.

## Firma — macOS

Sin firmar, al primer arranque macOS muestra Gatekeeper diciendo
"WeHi no se puede abrir porque proviene de un desarrollador no
identificado". El usuario puede abrirla con clic derecho ► Abrir,
o ejecutando una vez:

```bash
xattr -dr com.apple.quarantine /Applications/WeHi.app
```

Para distribución pública, firmar y notarizar:

1. **Apple Developer ID** — necesitas una cuenta del programa Apple
   Developer y un certificado "Developer ID Application".
2. **Firmar el .app** durante el bundle:
   - En `tauri.conf.json`, dentro de `bundle.macOS`, define
     `signingIdentity` con el nombre de tu certificado:
     ```json
     "signingIdentity": "Developer ID Application: Nombre (TEAMID)"
     ```
   - O usa variables de entorno:
     ```bash
     export APPLE_SIGNING_IDENTITY="Developer ID Application: Nombre (TEAMID)"
     npm run tauri build
     ```
3. **Notarizar** con `notarytool`. Tauri puede hacerlo si defines:
   ```bash
   export APPLE_ID="tu@apple.id"
   export APPLE_PASSWORD="app-specific-password"
   export APPLE_TEAM_ID="TEAMID"
   ```
   Tauri llama a `xcrun notarytool submit --wait` automáticamente y
   "staplea" el ticket al DMG.
4. **Entitlements**: si la app necesita capacidades de hardened
   runtime (acceso a archivos arbitrarios, JIT, etc.), añade un
   `.entitlements` y referéncialo en `bundle.macOS.entitlements`.

## Firma — Windows

Sin firmar, el SmartScreen de Windows muestra "Windows protegió tu
PC" al ejecutar el instalador. El usuario puede pulsar "Más
información" ► "Ejecutar de todas formas". Tras suficientes
instalaciones, SmartScreen "aprende" la app (lento) — firmar lo
evita.

1. Consigue un certificado Authenticode de un proveedor (DigiCert,
   Sectigo, etc.). Idealmente **EV Code Signing** para evitar la
   ventana de SmartScreen.
2. Importa el certificado en el almacén de Windows y anota su
   **thumbprint** (huella).
3. En `tauri.conf.json`, dentro de `bundle.windows`:
   ```json
   "certificateThumbprint": "AABBCCDD…",
   "digestAlgorithm": "sha256",
   "timestampUrl": "http://timestamp.digicert.com",
   "tsp": false
   ```
4. Compila con la cadena de Windows:
   ```bash
   npm run tauri build
   ```
   Tauri firma el .exe y el instalador con `signtool.exe`.

## Plugin updater (opcional)

Para actualizaciones automáticas, integra `tauri-plugin-updater`:

1. Añade la dependencia al `src-tauri/Cargo.toml`:
   ```toml
   tauri-plugin-updater = "2"
   ```
2. Añade el npm package y regístralo en `src-tauri/src/lib.rs`:
   ```rust
   .plugin(tauri_plugin_updater::Builder::new().build())
   ```
3. En `tauri.conf.json` añade:
   ```json
   "plugins": {
     "updater": {
       "endpoints": ["https://releases.tu-dominio.com/{{target}}/{{current_version}}"],
       "pubkey": "PUBLIC_KEY_BASE64"
     }
   }
   ```
4. Genera el par de claves:
   ```bash
   npx @tauri-apps/cli signer generate -w ~/.tauri/wehi.key
   ```
   Guarda la clave privada con cuidado; la pública va en
   `tauri.conf.json`.
5. Tras cada build, publica los artefactos firmados (.dmg y .nsis)
   junto con un JSON de manifiesto en tu endpoint. Tauri lo lee al
   arrancar la app y descarga la nueva versión si hay una.

## Checklist antes de publicar

- [ ] Identifier definitivo (no `com.grupods.wehi` placeholder).
- [ ] Versión incrementada en `tauri.conf.json` y `package.json`.
- [ ] Modelos ONNX presentes en `src-tauri/models/` y con licencia
      apta para uso comercial.
- [ ] Iconos definitivos en `src-tauri/icons/`.
- [ ] `cargo test` y `cargo clippy --all-targets -- -D warnings`
      pasan sin errores.
- [ ] `npm run tauri build` termina y deja los instaladores en
      `src-tauri/target/release/bundle/`.
- [ ] (Producción) firmados y notarizados según la plataforma.
