---
title: "Sincronización P2P (Iroh)"
description: "Funcionamiento del protocolo de red descentralizado, entrelazado de nodos y persistencia compartida."
---

# Sincronización P2P (Iroh)

La red descentralizada de Syntrix se construye sobre el ecosistema **Iroh**, el cual nos permite entrelazar datos entre múltiples nodos de forma orgánica, invisible y resistente a caídas de internet global.

## Protocolo Iroh Docs & Blobs

Syntrix utiliza **Iroh Docs** como motor de sincronización de documentos clave-valor basado en CRDTs (Conflict-Free Replicated Data Types), emparejado con **Iroh Blobs** para la transferencia optimizada de datos binarios y estructurados de gran tamaño.

### Características del Protocolo:
- **P2P Directo (Hole Punching)**: Los nodos intentan establecer conexiones directas peer-to-peer (usando protocolos de hole punching como STUN/TURN/DERP). Si los firewalls son muy restrictivos, la conexión se triangula a través de un servidor de relevo (Relay/DERP) manteniendo el cifrado de extremo a extremo (E2EE).
- **Gossip Protocol (`iroh-gossip`)**: Utilizado para la difusión rápida y multidifusión de eventos y estados de sincronización en tiempo real dentro del grupo de la organización.
- **Sincronía Eventual Nativa**: Si un nodo pierde conexión (trabajo offline, viajes, etc.), opera localmente sin problemas. En el momento en que detecta conectividad con otros peers, Iroh ejecuta una reconciliación automática (handshake) fusionando el log de cambios de manera determinista.

---

## Resolución de Conflictos y Consistencia

Dado que no existe un servidor centralizado o una noción de tiempo global absoluta, Syntrix e Iroh Docs garantizan la consistencia mediante:

1. **LWW (Last-Write-Wins) a nivel de Entrada**: En la asignación de claves, el cambio con la firma de autor y marca de tiempo criptográfica más reciente prevalece.
2. **Logs Append-Only**: Para registros transaccionales críticos (como operaciones contables, facturación o logs de auditoría), Syntrix modela los datos como logs inmutables ordenados en el tiempo. El estado actual de la entidad es el resultado de procesar secuencialmente este log inmutable.
3. **Validación Autorizada**: Toda sincronización de red corre a través de un callback de aceptación personalizado (`accept_cb` provisto por `iroh-syntrix-docs`) que valida criptográficamente que el peer remoto tenga los permisos correctos antes de aceptar la réplica de datos.
