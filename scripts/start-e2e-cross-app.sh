#!/usr/bin/env bash
# =============================================================================
# start-e2e-cross-app.sh
#
# Lanza 3 apps Tauri (admin + 2 clientes) con data dirs aislados y
# WebDriver habilitado en puertos distintos para tests E2E cross-app.
#
# Usa el bridge de compilación remota (server-1/server-2) para no saturar
# la laptop.
#
# Puertos:
#   Admin:   CDP/WebDriver en 9222, Vite en 1421
#   Client1: CDP/WebDriver en 9223, Vite en 1420
#   Client2: CDP/WebDriver en 9224, Vite en 1425
#
# Uso:
#   ./scripts/start-e2e-cross-app.sh
#
# Para detener: Ctrl+C (mata los 3 procesos)
# =============================================================================

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
E2E_DIR="/tmp/syntrix-e2e"

# Preparar PATH para usar el bridge de compilación remota
export PATH="$ROOT/bin:$PATH"

# Clean previous data to start fresh
rm -rf "$E2E_DIR"
mkdir -p "$E2E_DIR/admin" "$E2E_DIR/client1" "$E2E_DIR/client2"

echo "=== Starting Syntrix E2E cross-app environment ==="
echo "Data dirs: $E2E_DIR"
echo "Bridge: enabled (compilación remota)"
echo ""

cleanup() {
  echo ""
  echo "=== Shutting down all apps ==="
  kill $ADMIN_PID $CLIENT1_PID $CLIENT2_PID 2>/dev/null
  wait $ADMIN_PID $CLIENT1_PID $CLIENT2_PID 2>/dev/null
  echo "Done."
}
trap cleanup EXIT INT TERM

# ----- Admin (port 9222) en server-1 -----
echo "[admin] compiling on server-1, starting on ws://localhost:9222 ..."
cd "$ROOT/apps/admin/src-tauri"
SYNTRIX_DATA_DIR="$E2E_DIR/admin" \
  REMOTE_HOST="server-1" \
  cargo tauri dev \
    --config '{"build": {"devUrl": "http://localhost:1421"}}' \
    -- -- --remote-debugging-port=9222 \
    > "$E2E_DIR/admin.log" 2>&1 &
ADMIN_PID=$!

# ----- Client 1 (port 9223) en server-2 -----
echo "[client1] compiling on server-2, starting on ws://localhost:9223 ..."
cd "$ROOT/apps/client/src-tauri"
SYNTRIX_DATA_DIR="$E2E_DIR/client1" \
  REMOTE_HOST="server-2" \
  cargo tauri dev \
    --config '{"build": {"devUrl": "http://localhost:1420"}}' \
    -- -- --remote-debugging-port=9223 \
    > "$E2E_DIR/client1.log" 2>&1 &
CLIENT1_PID=$!

# ----- Client 2 (port 9224) en server-2 -----
echo "[client2] compiling on server-2, starting on ws://localhost:9224 ..."
cd "$ROOT/apps/client/src-tauri"
SYNTRIX_DATA_DIR="$E2E_DIR/client2" \
  REMOTE_HOST="server-2" \
  cargo tauri dev \
    --config '{"build": {"devUrl": "http://localhost:1425"}}' \
    -- -- --remote-debugging-port=9224 \
    > "$E2E_DIR/client2.log" 2>&1 &
CLIENT2_PID=$!

cd "$ROOT"

echo ""
echo "All apps launched. Waiting for readiness..."
echo "  Admin:   PID=$ADMIN_PID   ws://localhost:9222 (log: $E2E_DIR/admin.log)"
echo "  Client1: PID=$CLIENT1_PID ws://localhost:9223 (log: $E2E_DIR/client1.log)"
echo "  Client2: PID=$CLIENT2_PID ws://localhost:9224 (log: $E2E_DIR/client2.log)"
echo ""

# Wait for all WebDriver endpoints to be ready
for port in 9222 9223 9224; do
  echo -n "  Waiting for port $port ..."
  for i in $(seq 1 30); do
    if curl -s "http://localhost:$port/json/version" > /dev/null 2>&1; then
      echo " ready"
      break
    fi
    sleep 1
  done
done

echo ""
echo "=== All apps ready! ==="
echo "Run the E2E test in another terminal:"
echo "  cd apps/admin && npx playwright test --config src/__tests__/e2e-cross-app/playwright.config.ts"
echo ""
echo "Press Ctrl+C to stop all apps."

# Keep running until Ctrl+C
wait
