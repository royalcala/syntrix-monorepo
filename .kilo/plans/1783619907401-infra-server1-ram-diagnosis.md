# Infra — Diagnóstico y reparación de RAM defectuosa en server-1

> Sesión futura. `server-1` (Lenovo ThinkCentre M720s) crashea de forma aleatoria al compilar
> Rust en paralelo. Diagnóstico de la sesión 2026-07-09: **RAM defectuosa**. Este plan confirma
> el fallo con `memtest86+` y guía la reparación (reasentar/reemplazar/aislar el módulo), más la
> consolidación de los workarounds ya aplicados. Ejecutar con acceso físico a server-1 y/o consola
> (el `memtest` requiere reinicio y ~1-2h sin la máquina en servicio).

## Contexto verificado (evidencia de la sesión 2026-07-09)

- **Síntoma**: `rustc`/LLVM aborta con **SIGSEGV/SIGILL** en puntos **aleatorios** del compilador
  (LLVM `InlinerPass`/`DevirtSCCRepeatedPass`, ThinLTO `spawn_thin_lto_work`, `rustc_privacy`
  `TypePrivacyVisitor`) durante compilación **paralela** (`cargo build`/`test` con varios jobs).
- **No es software**: ocurre igual con `nixpkgs rustc 1.95.0` (LLVM 21) y con `rustup 1.96.1`
  (LLVM 22). Con `CARGO_BUILD_JOBS=1` **no** crashea (menos presión de memoria concurrente).
- **Máquina gemela sana**: `server-2` es **idéntico** — i5-8500 (Coffee Lake), microcode `0xfa`,
  kernel `6.18.35`, 6 cores, 15 GiB, **RAM non-ECC** — y compila en paralelo sin problemas.
- **Conclusión**: crashes aleatorios bajo carga paralela intensa + hardware/SO idénticos a una
  gemela sana + **RAM non-ECC** (sin corrección → corrupción silenciosa de páginas del compilador)
  ⇒ **módulo de RAM defectuoso en server-1**.
- `EDAC ie31200: No ECC support` confirma que no hay reporte/corrección de errores de memoria por HW.
- No hay MCE en `dmesg` (esperable con non-ECC: los bit-flips no se reportan).

## Workarounds YA aplicados (esta sesión) — consolidar o revertir según resultado

1. **Bridge `bin/cargo`** (commit `0427f3d`): detecta cargo/rustc de sistema; en server-1 usa
   `CARGO_BUILD_JOBS=1` + `RUST_MIN_STACK=33554432` (evita el crash paralelo) y añade
   `nixpkgs#glibc.dev`. Es una **mitigación**, no un fix — server-1 sigue lento y con RAM sospechosa.
2. **GC root de rustup**: el toolchain de rustup en server-1 tenía su intérprete glibc
   **GC'd** por `nix gc` (binarios ENOENT). Se reinstaló (`rustup install stable`) y se creó
   `/nix/var/nix/gcroots/per-user/root/rustup-glibc` → symlink al `ld-linux-x86-64.so.2` del
   glibc del toolchain, para que el GC semanal no lo borre.

## Objetivo

1. **Confirmar** el fallo de RAM con `memtest86+` (idealmente aislar el módulo/slot).
2. **Reparar**: reasentar, reordenar, o reemplazar el módulo defectuoso.
3. **Revalidar** con una compilación paralela completa que hoy crashea.
4. **Consolidar infra** en `infra-core` (GC root permanente del toolchain; decidir política del bridge).

## Compilación / entorno

- Repo infra: `~/Documents/github/infra-core` (flake nixos, colmena). server-1 = `192.168.1.10`.
- El bridge y las apps NO se necesitan para este plan (es hardware/SO), salvo la revalidación final.

---

## Tareas (orden de ejecución)

### H0 — Preparación (sin downtime)

1. Avisar/agendar downtime de server-1 (`memtest` deja la máquina fuera de servicio ~1-2h).
2. Anotar servicios críticos que corren en server-1 (ver `infra-core/nixos/hosts/server-1/`):
   forgejo, ollama/litellm/openwebui, ocis, stalwart, matrix-tuwunel, hyperswitch, postgres,
   erpnext, docker. Confirmar que su caída temporal es aceptable o migrarlos a `gateway-vps`/server-2.
3. Registrar baseline: `sudo dmidecode -t memory` (fabricante, part number, slots, velocidad) para
   saber cuántos módulos hay y en qué slots.

### H1 — Ejecutar memtest86+

4. Opción A (NixOS boot menu): NixOS incluye `memtest86+` en el menú de systemd-boot si está
   habilitado. Verificar/añadir en la config de server-1:
   ```nix
   boot.loader.systemd-boot.memtest86.enable = true;
   ```
   (rebuild + reboot, elegir la entrada MemTest86+ en el arranque).
5. Opción B (USB): flashear `memtest86+` a un USB y bootear desde él (si no hay acceso al boot menu
   remoto, requiere presencia física / IPMI / teclado+monitor).
6. Correr **al menos 2 pasadas completas** (idealmente toda la noche). Anotar:
   - ¿Errores? ¿en qué rango de direcciones? ¿test # (moving inversions, etc.)?
   - Si hay 2 módulos: los errores suelen mapear a un slot/módulo específico.

### H2 — Aislar y reparar

7. Si **hay errores**:
   - Apagar, **reasentar** ambos módulos (limpiar contactos). Reboot + memtest 1 pasada rápida.
   - Si persiste: probar **un módulo a la vez** / **slot a la vez** para identificar el culpable.
   - Reemplazar el módulo defectuoso (mismo spec que server-2: ver `dmidecode` de H0).
8. Si **NO hay errores** en memtest (falso negativo posible con fallos dependientes de temperatura/
   frecuencia):
   - Estrés alternativo: `stress-ng --vm 6 --vm-bytes 90% --timeout 30m` y/o compilación paralela
     en bucle; observar SIGSEGV.
   - Revisar XMP/frecuencia de RAM en BIOS (bajar a JEDEC estándar puede estabilizar).
   - Actualizar BIOS/UEFI del M720s si server-2 tiene versión distinta (`sudo dmidecode -t bios`).

### H3 — Revalidación

9. Con `bin/cargo` **sin** el workaround de jobs=1 (o forzando `CARGO_BUILD_JOBS` alto), compilar
   en server-1 lo que hoy crashea:
   ```bash
   export PATH="$PWD/bin:$PATH"; export REMOTE_HOST=server-1
   cargo test --workspace   # o: cargo build -p syntrix-admin -p syntrix-client
   ```
   Debe terminar **sin SIGSEGV/SIGILL** en ≥3 corridas limpias desde target frío.
10. Si queda estable, **revertir/relajar** el workaround del bridge para server-1 (volver a
    paralelo) — editar la rama del bridge en `bin/cargo` (bloque `if command -v cargo`).

### H4 — Consolidación en infra-core

11. **GC root permanente del toolchain rustup** (para que `nix.gc` semanal no rompa el linker):
    en `infra-core/nixos/hosts/server-1/` añadir un `systemd.tmpfiles.rules` o un
    `system.activationScripts` que cree/renueve el gcroot al glibc del toolchain activo, o
    preferir el `rustc`/`cargo` de nixpkgs pinneado a canal estable en `environment.systemPackages`
    (evita depender de binarios rustup dinámicos contra glibc del store).
12. Documentar en `infra-core` la política de compilación: qué host es primario para builds,
    y por qué (hasta cerrar este ticket, **server-2** es el estable).

---

## Riesgos y mitigación

- **Downtime de server-1**: corre muchos servicios (H0.2). Coordinar ventana o migrar temporalmente.
- **memtest falso negativo**: fallos intermitentes/térmicos pueden no aparecer en 2 pasadas; usar
  H2.8 (stress + compilación en bucle) como confirmación cruzada.
- **Acceso físico**: si server-1 es headless sin IPMI, bootear memtest puede requerir teclado+monitor
  o un USB preparado. Verificar acceso antes de agendar.
- **Si el módulo no es reemplazable a corto plazo**: dejar server-1 en `CARGO_BUILD_JOBS=1` (lento
  pero estable) y mantener server-2 como primario de builds.

## Validación (criterio de éxito)

- `memtest86+` sin errores tras la reparación (o módulo culpable identificado y reemplazado).
- ≥3 compilaciones paralelas completas en server-1 desde target frío **sin** SIGSEGV/SIGILL.
- Workaround de jobs=1 revertido para server-1 (o decisión documentada de mantenerlo).
- GC root del toolchain hecho permanente en `infra-core` (no depende de un fix manual efímero).

## Fuera de alcance

- Cambios en el código de la app Syntrix (este plan es puramente infra/hardware).
- Migración definitiva de servicios de server-1 (solo el downtime temporal para el diagnóstico).

## Referencias

- Sesión origen: `.kilo/plans/1783528011185-finalize-tests-tcp-fix.md` (sección "⚠️ Infra: server-1").
- Bridge: `bin/cargo` (bloque de detección de cargo de sistema, commit `0427f3d`).
- Infra: `infra-core/nixos/hosts/server-1/` (`base.nix`, `configuration.nix`), `flake.nix`.
- GC root aplicado: `server-1:/nix/var/nix/gcroots/per-user/root/rustup-glibc`.
