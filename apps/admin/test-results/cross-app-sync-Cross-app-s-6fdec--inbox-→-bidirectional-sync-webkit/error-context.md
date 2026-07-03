# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: cross-app-sync.spec.ts >> Cross-app sync (admin + 2 clients) >> full flow: org → role → add device → invite → inbox → bidirectional sync
- Location: src/__tests__/e2e-cross-app/flows/cross-app-sync.spec.ts:34:3

# Error details

```
Error: browserType.connect: WebSocket error: connect ECONNREFUSED ::1:9222
Call log:
  - <ws connecting> ws://localhost:9222/
  - <ws error> ws://localhost:9222/ error connect ECONNREFUSED ::1:9222
  - <ws connect error> ws://localhost:9222/ connect ECONNREFUSED ::1:9222
  - <ws disconnected> ws://localhost:9222/ code=1006 reason=

```