default:
    @just --list

DEPS := "nixpkgs#glib.dev nixpkgs#gtk3.dev nixpkgs#webkitgtk_4_1.dev nixpkgs#libsoup_3.dev nixpkgs#openssl.dev nixpkgs#cairo.dev nixpkgs#pango.dev nixpkgs#gdk-pixbuf.dev nixpkgs#at-spi2-core.dev nixpkgs#harfbuzz.dev nixpkgs#freetype.dev nixpkgs#fontconfig.dev nixpkgs#libxkbcommon.dev nixpkgs#libepoxy.dev nixpkgs#graphene.dev nixpkgs#libdrm.dev nixpkgs#zlib.dev nixpkgs#libpng.dev nixpkgs#libjpeg.dev nixpkgs#pkg-config nixpkgs#cargo nixpkgs#rustc nixpkgs#cmake nixpkgs#perl nixpkgs#nodejs_22 nixpkgs#mold nixpkgs#clang"

PKG_SETUP := 'export PKG_CONFIG_PATH=""; for d in /nix/store/*/lib/pkgconfig /nix/store/*/share/pkgconfig; do [ -d "$d" ] && ls "$d"/*.pc >/dev/null 2>&1 && PKG_CONFIG_PATH="$PKG_CONFIG_PATH:$d"; done; export PKG_CONFIG_PATH'

# =========================================================================
# 1. DESARROLLO ESTÁNDAR LOCAL (Compila y ejecuta en la laptop)
# =========================================================================

# Compila y ejecuta la app de administración en local
admin:
    pkill -f "apps/a[d]min/.*vite" || true
    pkill -x syntrix-admin || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; cd apps/admin/src-tauri; cargo tauri dev'

# Compila y ejecuta una segunda app de administración en local para pruebas
admin-2:
    pkill -f "apps/a[d]min/.*vite" || true
    pkill -x syntrix-admin || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-admin-2"; export IROH_DATA_DIR="$HOME/.local/share/iroh-admin-2"; cd apps/admin/src-tauri; cargo tauri dev'

# Compila y ejecuta la app del cliente en local
client:
    pkill -f "apps/c[l]ient/.*vite" || true
    pkill -x syntrix-client || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; cd apps/client/src-tauri; cargo tauri dev'

# Compila y ejecuta una segunda app del cliente en local para pruebas
client-2:
    pkill -f "apps/c[l]ient/.*vite" || true
    pkill -x syntrix-client || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-2"; export IROH_DATA_DIR="$HOME/.local/share/iroh-2"; cd apps/client/src-tauri; cargo tauri dev'

# =========================================================================
# 2. COMPILACIÓN REMOTA (Estrategia B: Compila en servidor, ejecuta en laptop)
# =========================================================================

# Compila remotamente en server-1 y ejecuta la app de admin localmente sin usar CPU local
remote-compile-admin:
    pkill -f "apps/a[d]min/.*vite" || true
    pkill -x syntrix-admin || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-1"; export PATH="$PWD/bin:$PATH"; cd apps/admin/src-tauri; cargo tauri dev'

# Compila remotamente en server-2 y ejecuta la app del cliente localmente sin usar CPU local
remote-compile-client:
    pkill -f "apps/c[l]ient/.*vite" || true
    pkill -x syntrix-client || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-2"; export PATH="$PWD/bin:$PATH"; cd apps/client/src-tauri; cargo tauri dev'

# =========================================================================
# 3. ENFOQUE HÍBRIDO (Vite Remoto en Servidor + Ventana Tauri Local en Laptop)
# =========================================================================

# [Ejecutar en Servidor] Levanta el servidor Vite de administración en la red local (LAN)
host-admin:
    cd apps/admin && pnpm dev --host

# [Ejecutar en Servidor] Levanta el servidor Vite de cliente en la red local (LAN)
host-client:
    cd apps/client && pnpm dev --host

# [Ejecutar en Laptop] Abre la ventana local de admin conectada a un servidor Vite remoto (ej. just remote-admin 100.64.0.2)
remote-admin server_ip="100.64.0.2":
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; cd apps/admin/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://{{server_ip}}:1421\", \"beforeDevCommand\": \"\"}}"'

# [Ejecutar en Laptop] Abre la segunda ventana local de admin conectada a un servidor Vite remoto para pruebas
remote-admin-2 server_ip="100.64.0.2":
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-admin-2"; export IROH_DATA_DIR="$HOME/.local/share/iroh-admin-2"; cd apps/admin/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://{{server_ip}}:1421\", \"beforeDevCommand\": \"\"}}"'

# [Ejecutar en Laptop] Abre la ventana local de cliente conectada a un servidor Vite remoto (ej. just remote-client 100.64.0.2)
remote-client server_ip="100.64.0.2":
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; cd apps/client/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://{{server_ip}}:1420\", \"beforeDevCommand\": \"\"}}"'

# [Ejecutar en Laptop] Abre la segunda ventana local de cliente conectada a un servidor Vite remoto para pruebas
remote-client-2 server_ip="100.64.0.2":
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-2"; export IROH_DATA_DIR="$HOME/.local/share/iroh-2"; cd apps/client/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://{{server_ip}}:1420\", \"beforeDevCommand\": \"\"}}"'

# =========================================================================
# 4. PRUEBAS, CALIDAD Y LIMPIEZA
# =========================================================================

# Corre todas las pruebas unitarias e integración de ambas aplicaciones
test:
    cd apps/client && pnpm test
    cd apps/admin && pnpm test

# Valida sintaxis y formato (linter) de ambas aplicaciones
lint:
    cd apps/client && pnpm lint
    cd apps/admin && pnpm lint

# Limpia los directorios locales cargo target de compilaciones de Rust
clean-builds:
    @echo "=== Limpiando archivos temporales locales y compilaciones ==="
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c 'cd apps/admin/src-tauri && cargo clean'
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c 'cd apps/client/src-tauri && cargo clean'

# Borra datos persistidos del Admin (Keypairs, blobs, etc.)
clean-data-admin:
    rm -rf ~/.local/share/syntrix-admin
    rm -rf ~/.local/share/syntrix-admin-2

# Borra datos persistidos del Cliente (Keypairs, blobs, etc.)
clean-data-client:
    rm -rf ~/.local/share/syntrix
    rm -rf ~/.local/share/syntrix-2

# Borra datos persistidos de AMBAS aplicaciones para empezar de cero
clean-data-all: clean-data-admin clean-data-client
    @echo "✅ Datos persistidos locales eliminados."

# =========================================================================
# 5. DOCUMENTACIÓN
# =========================================================================

# Levanta el servidor de desarrollo para la documentación de Astro Starlight
docs:
    pkill -f "[a]stro dev" || true
    pnpm docs:dev

# Compila el sitio estático de la documentación
docs-build:
    pnpm docs:build

