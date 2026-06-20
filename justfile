# Syntrix monorepo development commands

# Run admin app
admin:
    cd apps/admin && pnpm install && cd src-tauri && cargo tauri dev

# Run client app
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
