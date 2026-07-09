default:
    @just --list

DEPS := "nixpkgs#glib.dev nixpkgs#gtk3.dev nixpkgs#webkitgtk_4_1.dev nixpkgs#libsoup_3.dev nixpkgs#openssl.dev nixpkgs#cairo.dev nixpkgs#pango.dev nixpkgs#gdk-pixbuf.dev nixpkgs#at-spi2-core.dev nixpkgs#harfbuzz.dev nixpkgs#freetype.dev nixpkgs#fontconfig.dev nixpkgs#libxkbcommon.dev nixpkgs#libepoxy.dev nixpkgs#graphene.dev nixpkgs#libdrm.dev nixpkgs#zlib.dev nixpkgs#libpng.dev nixpkgs#libjpeg.dev nixpkgs#pkg-config nixpkgs#cargo nixpkgs#rustc nixpkgs#cmake nixpkgs#perl nixpkgs#nodejs_22 nixpkgs#mold nixpkgs#clang"

PKG_SETUP := 'export PKG_CONFIG_PATH=""; for d in /nix/store/*/lib/pkgconfig /nix/store/*/share/pkgconfig; do [ -d "$d" ] && ls "$d"/*.pc >/dev/null 2>&1 && PKG_CONFIG_PATH="$PKG_CONFIG_PATH:$d"; done; export PKG_CONFIG_PATH; export WEBKIT_DISABLE_COMPOSITING_MODE=1; export WEBKIT_DISABLE_DMABUF_RENDERER=1'


# =========================================================================
# 1. DESARROLLO ESTÁNDAR LOCAL (Compila y ejecuta en la laptop)
# =========================================================================

# Compila y ejecuta la app de administración en local
admin:
    pkill -f "apps/a[d]min/.*vite" || true
    pkill -x syntrix-admin || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; cd apps/admin/src-tauri; cargo tauri dev'

# Compila y ejecuta una segunda app de administración en local para pruebas en paralelo
admin-2:
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-admin-2"; cd apps/admin/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://localhost:1426\", \"beforeDevCommand\": \"pnpm dev --port 1426\"}}"'

# Compila y ejecuta la app del cliente en local
client:
    pkill -f "apps/c[l]ient/.*vite" || true
    pkill -x syntrix-client || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; cd apps/client/src-tauri; cargo tauri dev'

# Compila y ejecuta una segunda app del cliente en local para pruebas en paralelo
client-2:
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-2"; cd apps/client/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://localhost:1425\", \"beforeDevCommand\": \"pnpm dev --port 1425\"}}"'

# =========================================================================
# 2. COMPILACIÓN REMOTA (Estrategia B: Compila en servidor, ejecuta en laptop)
# =========================================================================

# Compila remotamente en server-1 y ejecuta la app de admin localmente sin usar CPU local
remote-compile-admin:
    pkill -f "apps/a[d]min/.*vite" || true
    pkill -x syntrix-admin || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-1"; export PATH="$PWD/bin:$PATH"; cd apps/admin/src-tauri; cargo tauri dev'

# Compila remotamente en server-1 y ejecuta la segunda app de admin localmente en paralelo
remote-compile-admin-2:
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-1"; export PATH="$PWD/bin:$PATH"; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-admin-2"; cd apps/admin/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://localhost:1426\", \"beforeDevCommand\": \"pnpm dev --port 1426\"}}"'

# Compila remotamente en server-2 y ejecuta la app del cliente localmente sin usar CPU local
remote-compile-client:
    pkill -f "apps/c[l]ient/.*vite" || true
    pkill -x syntrix-client || true
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-2"; export PATH="$PWD/bin:$PATH"; cd apps/client/src-tauri; cargo tauri dev'

# Compila remotamente en server-2 y ejecuta la segunda app del cliente localmente en paralelo
remote-compile-client-2:
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-2"; export PATH="$PWD/bin:$PATH"; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-2"; cd apps/client/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://localhost:1425\", \"beforeDevCommand\": \"pnpm dev --port 1425\"}}"'

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
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-admin-2"; cd apps/admin/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://{{server_ip}}:1421\", \"beforeDevCommand\": \"\"}}"'

# [Ejecutar en Laptop] Abre la ventana local de cliente conectada a un servidor Vite remoto (ej. just remote-client 100.64.0.2)
remote-client server_ip="100.64.0.2":
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; cd apps/client/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://{{server_ip}}:1420\", \"beforeDevCommand\": \"\"}}"'

# [Ejecutar en Laptop] Abre la segunda ventana local de cliente conectada a un servidor Vite remoto para pruebas
remote-client-2 server_ip="100.64.0.2":
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export SYNTRIX_DATA_DIR="$HOME/.local/share/syntrix-2"; cd apps/client/src-tauri; cargo tauri dev --config "{\"build\": {\"devUrl\": \"http://{{server_ip}}:1420\", \"beforeDevCommand\": \"\"}}"'

# =========================================================================
# 4. GESTIÓN DE PROCESOS
# =========================================================================

# Mata todas las apps Tauri y compilaciones locales (admin, client, vite)
kill-local:
    @echo "=== Matando procesos Tauri locales ==="
    pkill -x syntrix-admin 2>/dev/null || true
    pkill -x syntrix-client 2>/dev/null || true
    pkill -f "[v]ite.*apps/admin" 2>/dev/null || true
    pkill -f "[v]ite.*apps/client" 2>/dev/null || true
    pkill -f "[b]in/cargo.*tauri" 2>/dev/null || true
    @echo "✅ Procesos locales eliminados."

# Mata compilaciones Rust y procesos Tauri en los servidores remotos
kill-remote:
    @echo "=== Matando procesos en server-1 ==="
    ssh -o ConnectTimeout=3 server-1 "pkill -f 'cargo build| cargo check| cargo clippy| cargo run| cargo test| cargo doc| syntrix-admin| syntrix-client| nix.*shell.*cargo' 2>/dev/null; echo 'ok'" 2>/dev/null || echo "inaccesible"
    @echo "=== Matando procesos en server-2 ==="
    ssh -o ConnectTimeout=3 server-2 "pkill -f 'cargo build| cargo check| cargo clippy| cargo run| cargo test| cargo doc| syntrix-admin| syntrix-client| nix.*shell.*cargo' 2>/dev/null; echo 'ok'" 2>/dev/null || echo "inaccesible"
    @echo "✅ Procesos remotos eliminados."

# Limpia targets remotos (target-remote persistente + sesiones target-* residuales) en servidores remotos
clean-remote-targets:
    @echo "=== Limpiando targets remotos en server-1 ==="
    ssh -o ConnectTimeout=3 server-1 "rm -rf /root/remote-builds/syntrix-monorepo/target-remote /root/remote-builds/syntrix-monorepo/target-* 2>/dev/null; echo 'ok'" 2>/dev/null || echo "inaccesible"
    @echo "=== Limpiando targets remotos en server-2 ==="
    ssh -o ConnectTimeout=3 server-2 "rm -rf /root/remote-builds/syntrix-monorepo/target-remote /root/remote-builds/syntrix-monorepo/target-* 2>/dev/null; echo 'ok'" 2>/dev/null || echo "inaccesible"
    @echo "✅ Targets remotos eliminados (caché de compilación borrada)."

# Mata todo: local + servidores remotos
kill-all: kill-local kill-remote clean-remote-targets
    @echo "✅ Todos los procesos y targets de sesión han sido eliminados."

# =========================================================================
# 5. PRUEBAS, CALIDAD Y LIMPIEZA
# =========================================================================

# Tests Rust unitarios (rápidos, sin P2P) — solo #[cfg(test)] inline
test-unit:
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-1"; export PATH="$PWD/bin:$PATH"; cargo test --workspace --lib'

# Tests Rust integración (admin/client, con P2P)
test-integration:
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-1"; export PATH="$PWD/bin:$PATH"; cargo test --workspace --tests'

# Test E2E de binarios reales (procesos separados con dial P2P real)
# Compila ambos binarios remotamente, luego ejecuta el test localmente.
# Los hosts de compilación son parametrizables (default: server-2, el más estable).
# Ej: just test-binary-e2e                          → todo en server-2
#     just test-binary-e2e server-1 server-2        → admin+test en server-1, client en server-2
test-binary-e2e admin_host="server-2" client_host="server-2":
    #!/usr/bin/env bash
    set -e
    echo "=== Compilando admin en {{admin_host}} ==="
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="{{admin_host}}"; export PATH="$PWD/bin:$PATH"; cargo build -p syntrix-admin'
    echo "=== Compilando client en {{client_host}} ==="
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="{{client_host}}"; export PATH="$PWD/bin:$PATH"; cargo build -p syntrix-client'
    echo "=== Compilando test binary en {{admin_host}} ==="
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="{{admin_host}}"; export PATH="$PWD/bin:$PATH"; cargo test -p syntrix-admin --test binary_e2e_test --no-run'
    echo "=== Descargando test binary desde {{admin_host}} ==="
    ssh -o ConnectTimeout=10 {{admin_host}} "ls -t /root/remote-builds/syntrix-monorepo/target-remote/debug/deps/binary_e2e_test-* 2>/dev/null | head -1" > /tmp/binary_e2e_test_remote_path.txt
    REMOTE_BIN_PATH="$(cat /tmp/binary_e2e_test_remote_path.txt)"
    if [ -z "$REMOTE_BIN_PATH" ]; then
      echo "ERROR: No se encontró el binary de test en {{admin_host}}"
      exit 1
    fi
    LOCAL_BIN_DIR="apps/admin/src-tauri/target/debug"
    mkdir -p "$LOCAL_BIN_DIR"
    scp -o ConnectTimeout=10 "{{admin_host}}:$REMOTE_BIN_PATH" "$LOCAL_BIN_DIR/binary_e2e_test"
    chmod +x "$LOCAL_BIN_DIR/binary_e2e_test"
    echo "=== Ejecutando test binario localmente ==="
    CLIENT_BIN="$(pwd)/target/debug/syntrix-client"
    ADMIN_BIN="$(pwd)/target/debug/syntrix-admin"
    TEST_BIN="$LOCAL_BIN_DIR/binary_e2e_test"
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; SYNTRIX_CLIENT_BIN="'"$CLIENT_BIN"'" SYNTRIX_ADMIN_BIN="'"$ADMIN_BIN"'" "'"$TEST_BIN"'" --ignored --nocapture'

# Tests Rust completos (unit + integration)
test-rust:
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-1"; export PATH="$PWD/bin:$PATH"; cargo test --workspace'

# Tests frontend Vitest (admin + client)
test-frontend:
    cd apps/client && pnpm test
    cd apps/admin && pnpm test

# Tests completos sin E2E
test: test-frontend

# Tests E2E con Playwright/WebDriver (requiere app compilada y corriendo)
test-e2e-admin:
    cd apps/admin && npx playwright test --config src/__tests__/e2e/playwright.config.ts

test-e2e-client:
    cd apps/client && npx playwright test --config src/__tests__/e2e/playwright.config.ts

# Tests E2E cross-app (admin + 2 clientes, lanza apps automáticamente)
test-e2e-cross-app:
    #!/usr/bin/env bash
    set -e
    ROOT="$(pwd)"
    E2E_DIR="/tmp/syntrix-e2e"
    echo "=== Lanzando 3 apps Tauri (admin + 2 clientes) ==="
    rm -rf "$E2E_DIR"
    mkdir -p "$E2E_DIR/admin" "$E2E_DIR/client1" "$E2E_DIR/client2"
    # Admin (server-1)
    cd "$ROOT/apps/admin/src-tauri"
    SYNTRIX_DATA_DIR="$E2E_DIR/admin" REMOTE_HOST="server-1" cargo tauri dev \
      --config '{"build": {"devUrl": "http://localhost:1421"}}' \
      -- -- --remote-debugging-port=9222 > "$E2E_DIR/admin.log" 2>&1 &
    ADMIN_PID=$!
    # Client 1 (server-2)
    cd "$ROOT/apps/client/src-tauri"
    SYNTRIX_DATA_DIR="$E2E_DIR/client1" REMOTE_HOST="server-2" cargo tauri dev \
      --config '{"build": {"devUrl": "http://localhost:1420"}}' \
      -- -- --remote-debugging-port=9223 > "$E2E_DIR/client1.log" 2>&1 &
    CLIENT1_PID=$!
    # Client 2 (server-2)
    cd "$ROOT/apps/client/src-tauri"
    SYNTRIX_DATA_DIR="$E2E_DIR/client2" REMOTE_HOST="server-2" cargo tauri dev \
      --config '{"build": {"devUrl": "http://localhost:1425"}}' \
      -- -- --remote-debugging-port=9224 > "$E2E_DIR/client2.log" 2>&1 &
    CLIENT2_PID=$!
    cd "$ROOT"
    echo "Waiting for WebDriver endpoints..."
    for port in 9222 9223 9224; do
      echo -n "  port $port ..."
      for i in $(seq 1 30); do
        if curl -s "http://localhost:$port/json/version" > /dev/null 2>&1; then echo " ready"; break; fi
        sleep 1
      done
    done
    echo "=== Running Playwright test ==="
    cd apps/admin && npx playwright test --config src/__tests__/e2e-cross-app/playwright.config.ts; TEST_EXIT=$?
    echo "=== Killing apps ==="
    kill $ADMIN_PID $CLIENT1_PID $CLIENT2_PID 2>/dev/null || true
    wait $ADMIN_PID $CLIENT1_PID $CLIENT2_PID 2>/dev/null || true
    exit $TEST_EXIT

# Tests completos (todo incluyendo E2E)
test-all: test test-e2e-admin test-e2e-client test-e2e-cross-app

# Setup WebDriver para E2E
e2e-setup:
    @echo "=== Instalando WebKitWebDriver ==="
    @nix --extra-experimental-features "nix-command flakes" shell nixpkgs#webkitgtk_4_1 --command bash -c 'which WebKitWebDriver && echo "WebDriver OK"'
    cd apps/admin && npx playwright install webkit
    cd apps/client && npx playwright install webkit

# Valida sintaxis y formato (linter) de ambas aplicaciones
lint:
    cd apps/client && pnpm lint
    cd apps/admin && pnpm lint

# Genera migraciones SQL desde los schemas de Drizzle para admin y client
drizzle-gen:
    cd apps/admin && pnpm drizzle-kit generate
    cd apps/client && pnpm drizzle-kit generate
    cd apps/admin && pnpm export-schema
    cd apps/client && pnpm export-schema

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
# 6. DOCUMENTACIÓN
# =========================================================================

# Levanta el servidor de desarrollo para la documentación de Astro Starlight
docs:
    pkill -f "[a]stro dev" || true
    pkill -f "[n]ode.*astro" || true
    pnpm docs:dev

# Mata el servidor de desarrollo de la documentación de Astro Starlight
docs-kill:
    pkill -f "[a]stro dev" || true
    pkill -f "[n]ode.*astro" || true


# Compila el binario relay (self-hosted NAT relay server)
build-relay:
    @echo "=== Compilando syntrix-relay ==="
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; export REMOTE_HOST="server-2"; export PATH="$PWD/bin:$PATH"; cargo build -p syntrix-relay'

# Ejecuta el relay server localmente
run-relay listen="/ip4/0.0.0.0/udp/0/quic-v1":
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c '{{PKG_SETUP}}; cd apps/syntrix-relay && RUST_LOG=syntrix_relay=info cargo run -- --listen {{listen}}'

# Compila el sitio estático de la documentación (incluyendo rustdoc remoto y Astro Starlight)
docs-build:
    @echo "=== Compilando Rustdoc remotamente en el servidor ==="
    nix --extra-experimental-features "nix-command flakes" shell {{DEPS}} --command bash -c ' \
        {{PKG_SETUP}}; \
        export REMOTE_HOST="server-1"; \
        export PATH="$PWD/bin:$PATH"; \
        cd apps/admin/src-tauri && cargo doc --no-deps -p syntrix-admin -p syntrix-core; \
        cd ../../client/src-tauri && cargo doc --no-deps -p syntrix-client \
    '
    @echo "=== Copiando Rustdoc generado a la carpeta pública de Astro ==="
    rm -rf apps/docs/public/rustdoc
    mkdir -p apps/docs/public/rustdoc
    cp -r apps/admin/src-tauri/target/doc/* apps/docs/public/rustdoc/
    cp -r apps/client/src-tauri/target/doc/* apps/docs/public/rustdoc/ || true
    @echo "=== Compilando el sitio de Astro Starlight ==="
    pnpm docs:build

