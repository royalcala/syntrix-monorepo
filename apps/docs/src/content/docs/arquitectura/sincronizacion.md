---
title: "Sincronización P2P (Iroh)"
description: "Funcionamiento del protocolo de red descentralizado, entrelazado de nodos y persistencia compartida."
---

# Sincronización P2P (Iroh)

La red descentralizada de Syntrix se construye sobre el ecosistema **Iroh**, el cual nos permite entrelazar datos entre múltiples nodos de forma orgánica, invisible y resistente a caídas de internet global.

## Protocolo Iroh Docs

Syntrix utiliza **Iroh Docs**, un motor de sincronización de documentos basado en CRDTs (Conflict-Free Replicated Data Types) llave-valor.

### Características del Protocolo:
- **P2P Directo (Hole Punching)**: Los nodos se conectan directamente entre sí siempre que sea posible. Si ambos están detrás de cortafuegos estrictos, el tráfico pasa por un servidor DERP intermedio de forma cifrada de extremo a extremo.
- **Sincronía Eventual**: Si un nodo se desconecta (por ejemplo, viaja en un avión o se corta la electricidad), sigue operando en modo local. Al recuperar la conectividad, Iroh realiza un "handshake" automático de reconciliación y fusiona las ramas de datos de manera determinista.

---

## Resolución de Conflictos

Dado que no existe una base de datos central ni una noción global del tiempo absoluto, los conflictos se resuelven mediante las siguientes políticas:

1. **LWW (Last-Write-Wins)**: En campos simples, el cambio con la marca de tiempo criptográfica más reciente es el que prevalece.
2. **Historial de Modificaciones**: Para registros críticos (como facturación o cambios de inventario), cada acción se registra como una entrada inmutable de tipo append-only en el log de Iroh Docs. El estado actual se calcula recalculando el log.

> [!WARNING]
> Dado que dependemos de relojes locales de los dispositivos para resolver colisiones LWW, Syntrix implementa una tolerancia y sincronización de desvíos de reloj durante el protocolo de saludo (handshake).
