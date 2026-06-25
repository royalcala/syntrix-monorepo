---
title: "Estructura de Namespaces"
description: "Modelado de datos en Syntrix: los 4 namespaces core, su propósito y seguridad."
---

# Estructura de Namespaces

En Syntrix, los datos no viven en tablas tradicionales de bases de datos relacionales, sino en **Namespaces** (espacios de nombres criptográficos). Un namespace es un espacio compartido de almacenamiento clave-valor controlado por llaves públicas/privadas.

## Los 4 Namespaces Core

El modelado de datos de Syntrix se estructura en 4 namespaces con propósitos y permisos bien delimitados:

| Namespace | Nombre Criptográfico | Propósito | Permiso de Escritura |
|---|---|---|---|
| **Identity** | `sys.identity` | Contiene los datos del nodo, perfil de usuario, llaves criptográficas y llaves de otros namespaces. | Solo el nodo creador (Local) |
| **Workspace** | `sys.workspace` | Datos de la operación diaria del equipo (clientes, contactos, tareas, configuraciones del espacio). | Miembros autorizados del equipo |
| **Audit Log** | `sys.audit` | Log inmutable e irreversible de acciones realizadas en el espacio de trabajo. | Todo nodo (solo añadir) |
| **Plugins** | `sys.plugins` | Datos y estados particulares utilizados por complementos y workflows externos. | Nodos con plugin instalado |

---

## Seguridad y Criptografía

> [!IMPORTANT]
> Cada namespace es cifrado de extremo a extremo (E2EE) con una llave simétrica compartida únicamente entre los nodos que han sido invitados explícitamente a colaborar en ese espacio de trabajo.
> 
> Un atacante que intercepte el tráfico P2P o un servidor DERP intermedio solo verá bytes aleatorios.
