# Plan: Crear rama syntrix-v02 desde migration-to-limbo-db

**Contexto:** La rama `mercury-motor` completó la migración de iroh-docs → libp2p/Limbo. La rama `syntrix` (main actual) aún usa iroh-docs, lo cual causa errores como `"unknown namespace: customers"` porque el usuario corre accidentalmente el binario de `syntrix` en vez del de `mercury-motor`.  

Se decide crear `syntrix-v02` como nueva rama canónica y renombrar `mercury-motor` a `migration-to-limbo-db` como referencia histórica.

**Ramas involucradas:**

| Rama actual | Nueva rama | Rol |
|---|---|---|
| `mercury-motor` (local) | `migration-to-limbo-db` | Histórica, describe la migración |
| _nueva_ | `syntrix-v02` | Canónica, mismo HEAD que migration-to-limbo-db |
| `syntrix` | _sin cambios_ | Archivo de la versión iroh-docs |

---

## Pasos

### 1. Renombrar rama local y push

```bash
# En el repo principal
cd /home/alcala/Documents/github/syntrix-p2p/syntrix-monorepo

# Renombrar local
git branch -m mercury-motor migration-to-limbo-db

# Push con nuevo nombre y eliminar el viejo en origin
git push origin migration-to-limbo-db
git push origin --delete mercury-motor
```

### 2. Crear syntrix-v02 y push

```bash
# Crear desde migration-to-limbo-db
git branch syntrix-v02 migration-to-limbo-db

# Push
git push origin syntrix-v02
```

### 3. Verificar estructura final

Al terminar, `origin` debe tener:

```
origin/syntrix                  ← archivo iroh-docs (sin tocar)
origin/migration-to-limbo-db    ← histórico (ex mercury-motor)
origin/syntrix-v02              ← nueva canónica
```

`syntrix-v02` y `migration-to-limbo-db` apuntan al mismo commit `2931a17`.

### 4. Limpiar worktree local

```bash
# Eliminar el worktree (los commits ya están en las ramas)
git worktree remove .kilo/worktrees/mercury-motor
```

---

## Riesgos

- **Ninguno.** Las ramas `syntrix` y `origin/mercury-motor` no se pierden. Solo se renombra y se crea una nueva rama.
- El worktree `mercury-motor` se elimina solo después de confirmar que `syntrix-v02` existe en origin.

## Validación

- [ ] `git log --oneline syntrix-v02` muestra los mismos commits que `migration-to-limbo-db`
- [ ] `origin/syntrix-v02` existe en GitHub
- [ ] `origin/syntrix` sigue intacto
- [ ] El worktree `mercury-motor` fue removido sin errores
