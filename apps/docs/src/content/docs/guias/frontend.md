---
title: "Desarrollo Frontend (React)"
description: "Cómo desarrollar, probar y extender la interfaz de usuario soberana de Syntrix."
---

# Desarrollo Frontend

Esta sección documenta los estándares, componentes y flujos para el desarrollo del frontend de las aplicaciones Tauri de Syntrix (Client y Admin).

## Tecnologías Utilizadas

- **React 19**: Biblioteca UI principal.
- **Vite 7**: Empaquetador y servidor de desarrollo.
- **Tailwind CSS v4**: Framework de estilos.
- **TanStack Table & Form**: Manejo de tablas de datos y validaciones de formularios.
- **Lucide React**: Biblioteca de iconos premium.

## Estructura del Frontend

El frontend se divide en:
1. `@syntrix/ui` (ubicado en `packages/syntrix-ui/`): Componentes visuales compartidos y reutilizables.
2. Aplicaciones específicas en `apps/admin` y `apps/client`.

---

## Estándares de Diseño y UI

> [!IMPORTANT]
> Todos los nuevos componentes creados deben seguir las directrices estéticas de Syntrix:
> - Usar la paleta de colores corporativa y tipografía Geist.
> - Soporte nativo y sin fisuras para **Modo Oscuro/Claro**.
> - Micro-animaciones para mejorar la retroalimentación al usuario.

### Ejemplo de Creación de Componente

Al añadir un componente a `@syntrix/ui`, recuerda exportarlo en `packages/syntrix-ui/src/index.ts`.
