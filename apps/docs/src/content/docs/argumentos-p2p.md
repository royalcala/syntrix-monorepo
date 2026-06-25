---
title: "Syntrix — Complemento P2P y Iroh (Guía Técnica y Comercial)"
---

Este documento sirve como complemento al **[02-pitch.md](file:///root/Documents/github/syntrix-related-repos/syntrix-monorepo/syntrix-docs/02-pitch.md)**. Detalla el funcionamiento técnico detrás de nuestro "Espacio Operativo Autónomo" y proporciona los argumentos de venta clave para posicionar la tecnología Peer-to-Peer (P2P) y el framework **Iroh** frente a clientes de escala empresarial.

---

## 1. Fundamentos P2P para Ventas: ¿Por qué no usamos la nube tradicional?

En el modelo cliente-servidor tradicional (SaaS clásico), todos los datos viajan a un servidor centralizado (ej. AWS). Esto crea dependencia de conexión, altos costos de hosting y riesgos de privacidad.

En Syntrix, utilizamos una arquitectura **Peer-to-Peer (P2P)**:

```mermaid
graph TD
    subgraph Cliente-Servidor (SaaS Tradicional)
        A[Dispositivo 1] --> S((Servidor Central / AWS))
        B[Dispositivo 2] --> S
    end

    subgraph Peer-to-Peer (Syntrix + Iroh)
        P1((Dispositivo 1)) <--> P2((Dispositivo 2))
        P2((Dispositivo 2)) <--> P3((Dispositivo 3))
        P3((Dispositivo 3)) <--> P1((Dispositivo 1))
    end
```

### Argumentos de Negocio:
1. **Soberanía y Seguridad**: Los datos de la empresa viven exclusivamente en sus propios dispositivos. No hay un "servidor central" en la nube que pueda ser hackeado o secuestrado.
2. **Cero Caídas de Sistema**: Si el servidor central de un SaaS se cae, la operación del cliente se detiene. En Syntrix, al no haber servidor central, el sistema es inquebrantable. Si un nodo falla, los demás siguen transaccionando y sincronizando directamente.
3. **Cero Costos de Infraestructura Escala**: Para Syntrix como proveedor, el costo de mantener servidores es mínimo, lo que nos permite ofrecer precios más competitivos sin rentas de infraestructura abusivas.

---

## 2. La Pila Tecnológica: ¿Cómo funciona Iroh?

**Iroh** es un motor P2P moderno escrito en Rust, diseñado específicamente para ser rápido y eficiente en dispositivos móviles y de escritorio. Se divide en tres pilares fundamentales que resuelven los retos de red, sincronización y manejo de archivos:

```
┌─────────────────────────────────────────────────────────┐
│                       Syntrix UI                        │
├─────────────────────────────────────────────────────────┤
│    Iroh Docs (Sincronización de Base de Datos / K-V)    │
├─────────────────────────────────────────────────────────┤
│      Iroh Blobs (Transferencia de Archivos Grandes)     │
├─────────────────────────────────────────────────────────┤
│  Iroh Net / Magic Endpoint (QUIC, Hole Punching, DERP)  │
└─────────────────────────────────────────────────────────┘
```

### A. Conectividad Segura (Iroh Net)
* **Identidad por NodeID**: Cada dispositivo en Syntrix se identifica mediante una llave pública criptográfica (Ed25519). Las conexiones no se hacen a IPs cambiantes, sino a identidades inmutables.
* **Hole Punching (Perforación de Puertos)**: Iroh intenta establecer conexiones directas UDP entre dispositivos (incluso detrás de firewalls y NATs caseros) para evitar intermediarios.
* **Servidores Relay (DERP)**: Si ambos dispositivos están detrás de firewalls corporativos muy estrictos que impiden la conexión directa, el tráfico se enruta cifrado a través de un servidor de retransmisión (DERP).
  > **Nota de Seguridad**: El servidor Relay solo ve tráfico cifrado de extremo a extremo mediante QUIC (TLS 1.3). No puede leer, modificar ni interceptar los datos.

### B. Sincronización en Tiempo Real (Iroh Docs)
* **Gossip Protocol**: Cuando un vendedor registra un pedido offline y vuelve a conectarse, el sistema difunde el cambio a los demás peers de forma inmediata a través de un protocolo de "chisme".
* **Set Reconciliation**: Iroh compara bases de datos usando rangos matemáticos para transmitir únicamente los registros modificados. Esto ahorra hasta un 99% de ancho de banda en comparación con replicaciones SQL tradicionales.

### C. Almacenamiento Eficiente (Iroh Blobs)
* **Content-Addressed Storage**: Archivos grandes (como PDFs de facturas o imágenes de inventario) se identifican por el hash de su contenido (BLAKE3).
* **Descarga Verificada**: Iroh descarga archivos en fragmentos verificando la integridad de cada uno en tiempo real. Si un fragmento se corrompe en la transmisión, solo se descarga esa parte, no todo el archivo.

---

## 3. Resolución de Conflictos y Operación Offline

Una objeción común en ERPs offline-first es la colisión de datos. Syntrix lo resuelve mediante dos estrategias:

### A. Edición Concurrente: Last-Write-Wins (LWW) vía HLC
Si dos usuarios editan el mismo campo (ej. el teléfono de un cliente) estando desconectados, usamos **Hybrid Logical Clocks (HLC)**. El cambio con el HLC más reciente prevalece de manera automática y determinista en todos los nodos una vez que se restablece la conexión.

### B. Folios de Facturas: Prefijo + Contador Local
Para evitar que dos dispositivos offline generen el mismo folio de factura (ej. `INV-1001`), implementamos la estructura:
`[Prefijo_Sucursal]-[Contador_Local]-[Sufijo_Dispositivo]`
*(Ejemplo: `C-0142-DA1`)*

Esto garantiza **cero colisiones**, cumple con requerimientos de auditoría y permite un ordenamiento secuencial inmediato sin necesidad de consultar un servidor central.

---

## 4. Matriz de Manejo de Objeciones (Playbook de Ventas)

| Objeción del Cliente | Respuesta Técnica | Traducción Comercial (Valor) |
|---|---|---|
| **"¿Cómo se respaldan mis datos si no hay nube?"** | Cada nodo autorizado contiene una copia de la base de datos (redundancia). Además, se puede configurar un "nodo de backup pasivo" en una PC dedicada u oficina central. | **Respaldo automático e inmune**: Si una computadora se daña, conectas otra y en minutos descarga todo el historial desde los otros dispositivos de tu red local. |
| **"Si despido a alguien, ¿se lleva los datos?"** | Se revoca la llave criptográfica del nodo del empleado despedido en el control de acceso del espacio. | **Seguridad instantánea**: Los demás dispositivos bloquean y rechazan cualquier intento de sincronización de ese nodo inmediatamente. |
| **"Mi red de oficina tiene firewalls estrictos."** | Iroh conmuta automáticamente a servidores Relay (DERP) usando el puerto HTTPS estándar (443). | **Funciona sin configuración**: No necesitas que tu equipo de sistemas abra puertos peligrosos ni configure VPNs complejas. |
| **"¿Cómo facturo ante el SAT de forma offline?"** | El registro comercial es inmediato y local. El timbrado XML ante el PAC autorizado se procesa en segundo plano en cuanto se detecta conexión a internet. | **Venta fluida**: El cliente recibe su ticket al instante y la factura legal se timbra automáticamente sin interrumpir tu caja. |