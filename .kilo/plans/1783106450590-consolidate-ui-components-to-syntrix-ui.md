# Plan: Consolidate UI Components into `@syntrix/ui`

## Problem

`apps/client` has **15 duplicated files** that already exist in `packages/syntrix-ui`:
- 9 shadcn-style UI primitives (`ui/button`, `ui/badge`, `ui/card`, `ui/input`, `ui/select`, `ui/sheet`, `ui/switch`, `ui/scroll-area`, `ui/table`)
- 3 composite components (`ThemeToggle`, `EntityGrid`, `DetailPanel`)
- 1 utility (`lib/utils.ts`)
- 2 theming CSS files (`index.css` copied between both apps)

Client imports all components locally from `./components/` despite having `@syntrix/ui` as a workspace dependency. Admin already imports correctly from `@syntrix/ui`.

Additionally, the **EntityGrid** in syntrix-ui and client have **diverged** — each has features the other lacks. They need merging so both apps get the full feature set.

The **theming CSS** (shadcn color tokens, dark mode, base styles) is duplicated across both apps. Admin has a richer setup (`shadcn/tailwind.css`, `tw-animate-css`, Geist font, chart colors) that client lacks.

## Key Decisions

1. **EntityGrid merge**: Port client's Limbo FTS search (`invoke("search_entity")`) + URL params into syntrix-ui's EntityGrid. FTS search controlled by `enableSearch` prop (default `false`).
2. **Shared CSS**: Put everything in `@syntrix/ui` — both apps import a single `theme.css` from the package.
3. **DetailPanel**: Use syntrix-ui's version (superset: has `customActions`, `multi-select`, better error handling).

## Context / Notes

- `search_entity` Tauri command is **not deprecated**. Backend migrated from standalone Tantivy to Limbo SQL's native FTS (`fts_match`/`fts_score` functions). The comment "Tantivy full-text index" in `apps/client/src/components/EntityGrid.tsx:55` is stale.
- Both apps use Tailwind v4 with `@import "tailwindcss"` and `@theme inline` (no `tailwind.config.*` files).
- Admin's CSS brings extra deps: `shadcn` (npm), `tw-animate-css` (npm), `@fontsource-variable/geist` (npm).
- `syntrix-ui` already depends on `@tauri-apps/api`, `clsx`, `tailwind-merge`, `@tanstack/react-query`, `@tanstack/react-table`.

---

## Tasks

### Phase 1: Shared CSS foundation

#### 1.1 Add CSS dependencies to `@syntrix/ui`
- Add to `packages/syntrix-ui/package.json` dependencies:
  - `"shadcn": "^4.11.0"`
  - `"tw-animate-css": "^1.4.0"`
  - `"@fontsource-variable/geist": "^5.2.9"`

#### 1.2 Create `packages/syntrix-ui/src/theme.css`
- Combine the admin's richer `index.css` setup into one shared file:
  - `@import "tailwindcss"`
  - `@import "tw-animate-css"`
  - `@import "shadcn/tailwind.css"`
  - `@import "@fontsource-variable/geist"`
  - `@custom-variant dark (&:where(.dark, .dark *))`
  - `@theme inline` block (all color, sidebar, radius, font, chart tokens)
  - `:root` and `.dark` CSS custom properties
  - `@layer base` styles (border, body, html)

#### 1.3 Update both apps to use shared CSS
- **`apps/admin/src/index.css`**: replace entire content with:
  - `@import "@syntrix/ui/theme.css"`
  - `@source "../../../packages/syntrix-ui"`
  - Any admin-specific overrides (if needed, otherwise empty beyond the imports)
- **`apps/client/src/index.css`**: replace entire content with same pattern
- Keep `@source "../../../packages/syntrix-ui"` in both so Tailwind scans `@syntrix/ui` for classes

---

### Phase 2: Primitives, utils, and ThemeToggle cleanup

#### 2.1 Delete duplicated files from `apps/client/src/`
Remove these files:
- `apps/client/src/components/ui/button.tsx`
- `apps/client/src/components/ui/badge.tsx`
- `apps/client/src/components/ui/card.tsx`
- `apps/client/src/components/ui/input.tsx`
- `apps/client/src/components/ui/select.tsx`
- `apps/client/src/components/ui/sheet.tsx`
- `apps/client/src/components/ui/switch.tsx`
- `apps/client/src/components/ui/scroll-area.tsx`
- `apps/client/src/components/ui/table.tsx`
- `apps/client/src/components/ThemeToggle.tsx`
- `apps/client/src/lib/utils.ts`

#### 2.2 Update client imports to use `@syntrix/ui`
Replace all local `./components/ui/*` imports with `@syntrix/ui/components/ui/*`:
- `apps/client/src/screens/Inbox.tsx`: `../components/ui/card` → `@syntrix/ui/components/ui/card`, ditto for button/badge
- `apps/client/src/screens/Setup.tsx`: `../components/ui/input` → `@syntrix/ui/components/ui/input`, ditto for button
- `apps/client/src/App.tsx`: Already imports from `@syntrix/ui` for some components. Ensure ThemeToggle is imported from `@syntrix/ui/components/ThemeToggle`

For `cn` utility:
- Check if any client file imports `cn` from `./lib/utils`. If so, update to `@syntrix/ui/lib/utils`.
- Check `apps/admin` too — admin has its own `src/lib/utils.ts`. Update admin imports to use `@syntrix/ui/lib/utils` instead, then delete `apps/admin/src/lib/utils.ts`.

---

### Phase 3: EntityGrid merge into `@syntrix/ui`

#### 3.1 Port client features into `packages/syntrix-ui/src/components/EntityGrid.tsx`

Port these features from the client's version:

1. **Debounced Limbo FTS search**: Add `invoke("search_entity", { query, entities, limit })` with debounce (150ms). Controlled by new `enableSearch?: boolean` prop (default `false`). Results are used to filter/sort displayed rows by score.

2. **URL-based row selection**: Add `useSearchParams` integration — when URL has `?id=xxx`, auto-open the DetailPanel for that record. Import from `react-router-dom` (already a dep).

3. **`onCreateRecord` callback prop**: Add optional prop so parent can customize row creation. Keep existing internal default behavior as fallback.

4. **Fix `setRefreshKey` bug**: In the mobile bottom sheet's backdrop onClick (line ~304), `setRefreshKey` is referenced but never defined. Replace with a proper mechanism (e.g., close detail and trigger `onCreateRecord` if in create mode, or just close).

5. **Update stale comment**: Change "Reactive search results from Tantivy full-text index" to "Reactive search results via Limbo FTS".

The merge must preserve all existing syntrix-ui features:
- `entity_changed` event listener (`listen("entity_changed", ...)`)
- `customActions` prop
- `onSaveUpdate` callback prop
- `multi-select` field display in grid cells
- Relative date formatting ("Hoy", "Ayer", "Hace N días")
- `loadData(orgId)` taking orgId parameter
- Mobile bottom sheet detail panel

#### 3.2 Update EntityGrid props interface

Ensure the merged interface has all needed props:

```ts
interface EntityGridProps {
  entity: EntityDefinition & { loadData?: (orgId?: string) => Promise<Array<Record<string, unknown>>> };
  activeView?: string;
  role?: string;
  orgId?: string;
  onSaveCreate?: (row: Row) => Promise<Row>;
  onSaveUpdate?: (recordId: string, changes: Record<string, unknown>) => Promise<void>;
  customActions?: (row: Record<string, unknown>) => React.ReactNode;
  enableSearch?: boolean;  // NEW: enables Limbo FTS search
  onCreateRecord?: () => Promise<Row>;  // NEW: optional custom create handler
}
```

#### 3.3 Update `packages/syntrix-ui/src/index.ts`

- Add `SearchResult` type export if the search feature exposes it
- Ensure `EntityGrid` is exported with the updated interface

#### 3.4 Delete client's local EntityGrid and update imports

- Delete `apps/client/src/components/EntityGrid.tsx`
- In `apps/client/src/App.tsx`: change `import { EntityGrid } from "./components/EntityGrid"` to `import { EntityGrid } from "@syntrix/ui/components/EntityGrid"`
- Pass `enableSearch={true}` to all EntityGrid usages in App.tsx (lines 200, 210, 220, 230)

#### 3.5 Update client App.tsx EntityGrid callbacks

The client's App.tsx sets `EntityGrid` callbacks inline for `onSaveCreate` and `onSaveUpdate`. Verify these patterns work with the merged syntrix-ui EntityGrid interface (they should, since the syntrix-ui version already has `onSaveCreate` and `onSaveUpdate` props). Adjust if needed.

---

### Phase 4: DetailPanel consolidation

#### 4.1 Delete client's local DetailPanel and update imports

- Delete `apps/client/src/components/DetailPanel.tsx`
- The import is inside `EntityGrid.tsx`, which is being deleted in Phase 3. No direct imports to update in client source code.
- However, check if any other client file directly imports `DetailPanel` from `./components/DetailPanel` (besides EntityGrid). Based on analysis, none do.
- Check `apps/client/src/__tests__/components/DetailPanel.test.tsx` — it imports from `../components/DetailPanel`. Update to `@syntrix/ui/components/DetailPanel`.

#### 4.2 Verify syntrix-ui DetailPanel is a complete superset

Compare both versions:
- syntrix-ui has: `customActions` prop, `multi-select` field support, `onSaveUpdate` takes `(recordId, changes)`, better error handling with `error.message`
- client has: simpler `onSaveUpdate` takes `(id, val)`, no `customActions`, no `multi-select`

syntrix-ui version is complete superset. No changes needed.

---

### Phase 5: Tests and verification

#### 5.1 Update test imports

- `apps/client/src/__tests__/components/EntityGrid.test.tsx`: change `import { EntityGrid } from "../components/EntityGrid"` to `import { EntityGrid } from "@syntrix/ui/components/EntityGrid"`
- `apps/client/src/__tests__/components/DetailPanel.test.tsx`: change `import { DetailPanel } from "../components/DetailPanel"` to `import { DetailPanel } from "@syntrix/ui/components/DetailPanel"`

#### 5.2 Verify no remaining stale imports

Search for any remaining local imports in client that should now come from `@syntrix/ui`:
- `from "./components/ui/"` — should be zero matches
- `from "../components/ui/"` — should be zero matches
- `from "./components/EntityGrid"` — should be zero matches (outside tests)
- `from "./components/DetailPanel"` — should be zero matches (outside tests)
- `from "./components/ThemeToggle"` — should be zero matches

#### 5.3 Run type checking and tests

```bash
# In packages/syntrix-ui
npx tsc --noEmit

# In apps/client
npx tsc --noEmit && npx vitest run

# In apps/admin  
npx tsc --noEmit && npx vitest run
```

---

### Phase 6: Package.json cleanup

#### 6.1 `@syntrix/ui` — already done in Phase 1.1

#### 6.2 `apps/client` — optional cleanup

Review if these deps can be removed (they're now sourced via `@syntrix/ui`):
- `@radix-ui/react-select` — only used in `DetailPanel` (now in syntrix-ui). Keep if used elsewhere.
- `class-variance-authority` — not directly imported in client code. Can remove.
- `clsx` — only used in `lib/utils.ts` (deleted). Keep if used elsewhere.
- `tailwind-merge` — only used in `lib/utils.ts` (deleted). Keep if used elsewhere.

Check: `drizzle-orm` is used client-side for something. Do not touch.

#### 6.3 `apps/admin` — optional cleanup

Same `clsx`, `tailwind-merge`, `class-variance-authority` review as client.

---

## File Manifest

### Created
| File | Purpose |
|---|---|
| `packages/syntrix-ui/src/theme.css` | Shared CSS — shadcn tokens, dark mode, base styles, fonts |

### Modified
| File | Changes |
|---|---|
| `packages/syntrix-ui/package.json` | Add `shadcn`, `tw-animate-css`, `@fontsource-variable/geist` deps |
| `packages/syntrix-ui/src/components/EntityGrid.tsx` | Merge FTS search, URL params, fix setRefreshKey, update comment |
| `packages/syntrix-ui/src/index.ts` | Add exports for new types if needed |
| `apps/client/src/index.css` | Replace with `@import "@syntrix/ui/theme.css"` |
| `apps/admin/src/index.css` | Replace with `@import "@syntrix/ui/theme.css"` |
| `apps/client/src/App.tsx` | Update EntityGrid import path, add `enableSearch` prop |
| `apps/client/src/screens/Inbox.tsx` | Update ui/ imports to `@syntrix/ui/...` |
| `apps/client/src/screens/Setup.tsx` | Update ui/ imports to `@syntrix/ui/...` |
| `apps/client/src/__tests__/components/EntityGrid.test.tsx` | Update import to `@syntrix/ui` |
| `apps/client/src/__tests__/components/DetailPanel.test.tsx` | Update import to `@syntrix/ui` |
| `apps/client/package.json` | Optional: remove unused deps (`class-variance-authority`, maybe `clsx`/`tailwind-merge`) |

### Deleted
| File | Reason |
|---|---|
| `apps/client/src/components/ui/button.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/ui/badge.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/ui/card.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/ui/input.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/ui/select.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/ui/sheet.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/ui/switch.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/ui/scroll-area.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/ui/table.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/ThemeToggle.tsx` | Duplicate — exists in syntrix-ui |
| `apps/client/src/components/EntityGrid.tsx` | Duplicate — merged into syntrix-ui |
| `apps/client/src/components/DetailPanel.tsx` | Duplicate — syntrix-ui superset |
| `apps/client/src/lib/utils.ts` | Duplicate — exists in syntrix-ui |
| `apps/admin/src/lib/utils.ts` | Duplicate — exists in syntrix-ui |

---

## Risks

1. **EntityGrid merge complexity**: Two diverged versions with different search strategies. FTS search is guarded by a prop; if the merge introduces bugs, both apps are affected.
   - *Mitigation*: Run both apps' test suites after merge. The `enableSearch` prop defaults to false so admin is unaffected by default.

2. **CSS import order**: `shadcn/tailwind.css` provides base shadcn theme, our `:root` + `.dark` customizes it. Import order matters — `shadcn/tailwind.css` before custom properties.
   - *Mitigation*: Test in both apps' dev mode. Mirror admin's working import order.

3. **Missing `@fontsource-variable/geist` in syntrix-ui**: Adding a font dep to the shared UI package means it's always loaded even if an app doesn't use it.
   - *Mitigation*: Low impact — tree-shaking and both apps were going to use it anyway.

4. **Client `onSaveCreate`/`onSaveUpdate` callback signatures**: Client's EntityGrid inline callbacks do `await queryClient.refetchQueries(...)` and `queryClient.setQueryData(...)`. If the merged syntrix-ui version's `onSaveUpdate` takes `(recordId, changes)` (2 args) but client expects `(id, val)`, the interface may differ.
   - *Mitigation*: The syntrix-ui DetailPanel already calls `onSaveUpdate(recordId, finalValue)`. Client's App.tsx inline callbacks should match this signature. Verify and adjust if client was using different arity.

## Validation

1. Both apps compile without TypeScript errors (`tsc --noEmit`)
2. Both apps pass existing tests (`vitest run`)
3. No remaining imports to `./components/ui/` or `./components/EntityGrid` etc. in client
4. CSS themes render correctly in both apps in light and dark modes
5. EntityGrid search works in client (with `enableSearch={true}`)
6. `CommandPalette` global search still works (uses same `search_entity` in App.tsx)
