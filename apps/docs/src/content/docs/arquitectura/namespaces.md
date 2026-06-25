---
title: "Estructura de Namespaces"
description: "Modelado de datos en Syntrix: los 4 namespaces core, su propósito y seguridad."
---

# Estructura de Namespaces

En Syntrix, los datos no viven en tablas tradicionales de bases de datos relacionales, sino en **Namespaces** (espacios de nombres criptográficos). Un namespace es un espacio compartido de almacenamiento clave-valor controlado por firmas criptográficas basadas en llaves públicas y privadas.

## Los 4 Namespaces Core

El modelado de datos y aislamiento en Syntrix se estructura en 4 namespaces con propósitos y niveles de acceso específicos:

| Namespace | Identificador / Prefijo | Propósito | Permiso de Escritura / Acceso |
|---|---|---|---|
| **Control** | `control_id` | Contiene metadatos de la organización, registro de dispositivos miembro (`members/`), roles y políticas de permisos (`roles/`). | Administradores de la Organización |
| **Catalogs** | `catalogs_id` | Almacena datos operativos maestros comunes (catálogo de productos, catálogo de clientes, proveedores). | Roles autorizados (ej: Admin, Sales) |
| **Operational** | `operational_id` | Registra transacciones diarias, facturas creadas, órdenes y movimientos de inventario. | Roles con permisos de escritura operativa |
| **Payroll** | `payroll_id` | Datos altamente sensibles correspondientes a nóminas, salarios y transacciones internas del personal. | Restringido (ej: Solo Admin o Contabilidad) |

---

## Seguridad, Autorización y Aislamiento

### Cifrado de Extremo a Extremo (E2EE)
Cada uno de estos 4 namespaces está protegido con llaves simétricas distribuidas únicamente a los dispositivos autenticados. La red de transporte (incluso los servidores DERP/Relay intermedios) solo transmite datos cifrados.

### Roles y Validación Activa (`accept_cb`)
Syntrix no se limita a un control de acceso superficial en la interfaz. Cuando un nodo intenta sincronizar un namespace, el motor de red ejecuta el callback `accept_cb` definido en la librería `iroh-syntrix-docs`:

1. **Lectura de Políticas**: Se consulta la lista de miembros y roles configurada en el namespace **Control**.
2. **Rechazo Criptográfico**: Si el dispositivo solicitante no tiene un rol válido con permisos de acceso para el namespace solicitado (por ejemplo, un vendedor intentando sincronizar el namespace de **Payroll**), el handshake de Iroh se aborta inmediatamente a nivel de protocolo de red.
3. **Heartbeats Activos**: Los nodos reportan periódicamente su estado de presencia y dirección de red mediante entradas con formato `heartbeat/<node_id_hex>` dentro del namespace de **Control**, permitiendo un monitoreo dinámico de los peers activos.
