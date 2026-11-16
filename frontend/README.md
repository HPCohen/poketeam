# Poketeam frontend

React, TypeScript, React Router, TanStack Query, Vite, and plain CSS. Requires Node 22.12+ and npm. The generated API types and npm lockfile are committed.

```sh
cd frontend
npm ci
npm run dev
```

Open **http://127.0.0.1:5173**. Run the backend with `PUBLIC_ORIGIN=http://127.0.0.1:5173` (exact scheme, host and port; no trailing slash) and `LISTEN_ADDR=127.0.0.1:3000`. Vite proxies `/api`, `/health`, and `/openapi.json` there; set `API_TARGET` when the backend listens elsewhere. Using `localhost` in the browser requires `PUBLIC_ORIGIN=http://localhost:5173` instead. Restart the backend after changing its environment. With Compose, run `PUBLIC_ORIGIN=http://127.0.0.1:5173 docker compose up -d api worker` after migrations.

```sh
npm run generate:api  # after intentionally changing backend/openapi.json
npm run check:api     # reproducible generation drift check
npm run typecheck
npm test
npx playwright install chromium
npm run test:e2e      # mocked API, desktop and mobile Chromium
npm run build        # static output: frontend/dist
```

The browser suites start Vite automatically. Mocked API tests are deterministic and need no database or upstream access. For a real-backend check, run both services, create a browser session, save a roster, reload it, and generate a counter after catalog initialization. `node scripts/smoke.mjs` checks real session cookies, Origin rejection, catalog shapes, revisions, and empty DELETE responses through the running Vite proxy. It creates and deletes only its own uniquely named team. The API and Vite must both be running; an uninitialized catalog is reported as a verification limitation.

Production serves `dist` under the same origin as `/api`; route unknown frontend paths back to `index.html` for direct links. Existing AWS scripts support this output. Do not use the Vite development server in production. Deployment is separate from this delivery.

Identity lives only in an HttpOnly backend cookie. Public exploration does not create a session. Explicit Get started initializes it. Drafts live in React state and do not survive reloads; the editor warns before leaving. Save submits the loaded revision. A conflict preserves the draft until an explicit discard/reload. A 401 clears private server data and drafts without replaying mutations. Counter generation returns an unsaved preview; its separate save form creates a new team. Notifications replace the current page on each refresh and deduplicate by event ID.
