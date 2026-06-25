---
title: "Esquemas y Tipos"
description: "Modelos de datos definidos en Zod y TypeScript para las entidades de Syntrix."
---

# Esquemas y Tipos

Syntrix valida todos los datos tanto en el frontend (usando **Zod**) como en el backend (usando structs y enums de **Rust** con validación estricta).

## Entidad Contacto (Contact)

Esquema de validación Zod usado para los formularios y la base de datos TanStack DB:

```typescript
import { z } from 'zod';

export const ContactSchema = z.object({
  id: z.string().uuid(),
  name: z.string().min(2, "El nombre debe tener al menos 2 caracteres"),
  email: z.string().email("Correo electrónico inválido").optional(),
  phone: z.string().optional(),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type Contact = z.infer<typeof ContactSchema>;
```
