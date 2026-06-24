# Syntrix monorepo development commands

# Run admin app locally
admin:
    cd apps/admin && pnpm install && cd src-tauri && cargo tauri dev

# Run client app locally
client:
    cd apps/client && pnpm install && cd src-tauri && cargo tauri dev

# Run all tests
test:
    cd apps/client && pnpm test
    cd apps/admin && pnpm test

# Lint both apps
lint:
    cd apps/client && pnpm lint
    cd apps/admin && pnpm lint

# --- ENFOQUE HÍBRIDO (Vite Remoto en server-1 + Ventana Tauri Local en laptop-rao) ---

# [En server-1] Corre el servidor de desarrollo de admin escuchando en toda la red local (LAN)
host-admin:
    cd apps/admin && pnpm dev --host

# [En server-1] Corre el servidor de desarrollo de client escuchando en toda la red local (LAN)
host-client:
    cd apps/client && pnpm dev --host

# [En laptop-rao] Compila y ejecuta la ventana local de admin conectándose a server-1 (ej. just remote-admin 100.64.0.2:1421)
remote-admin server_ip="100.64.0.2:1421":
    cd apps/admin && cd src-tauri && cargo tauri dev --config '{"build": {"devUrl": "http://{{server_ip}}:1421", "beforeDevCommand": ""}}'

# [En laptop-rao] Compila y ejecuta la ventana local de client conectándose a server-1 (ej. just remote-client 100.64.0.2:1421)
remote-client server_ip="100.64.0.2:1421":
    cd apps/client && cd src-tauri && cargo tauri dev --config '{"build": {"devUrl": "http://{{server_ip}}:1420", "beforeDevCommand": ""}}'

# --- RESET DE DATOS (borra keypair, docs, blobs, orgs) ---

# Borra todos los datos persistidos del admin (keypair, docs, blobs, orgs)
# PRECAUCIÓN: esto eliminará la identidad y organización del admin. Ejecutar antes de iniciar de nuevo.
clean-admin:
    rm -rf ~/.local/share/syntrix-admin

# Borra todos los datos persistidos del client
clean-client:
    rm -rf ~/.local/share/syntrix

# Borra datos de AMBAS apps — empezar completamente de cero
clean-all: clean-admin clean-client
    @echo "✅  Datos de admin y client eliminados. Puedes reiniciar ambas apps."

