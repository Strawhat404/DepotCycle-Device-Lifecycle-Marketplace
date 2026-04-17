# DepotCycle Device Lifecycle Marketplace

**Project type: Fullstack (Rust backend + Leptos WASM frontend)**

Part 1 + Part 2 implementation for the offline-first DepotCycle marketplace. This repo contains:

- `backend/`: Axum API, SQLite schema/migrations, auth/session/security foundation, and admin API shell.
- `frontend/`: Leptos WASM client-side rendered UI for all user roles.
- `unit_tests/`: Unit-level Rust test crate (auth, security, workflows, models, config, error, db helpers, session middleware, router composition, bootstrap/seed wiring).
- `API_tests/`: API functional Rust test crate covering the backend route surface (positive flows, negative paths, authorization, edge cases).
- `frontend_tests/`: Frontend contract test crate (FE type deserialization against real BE responses, role-access denial, field-level validation).
- `frontend/` (WASM tests): Component-level and reactive-state tests inside the Leptos crate (`wasm_bindgen_test`; run via `wasm-pack test --headless`).
- `e2e_tests/`: Router-level end-to-end workflow test crate simulating complete multi-step user journeys for every role.

## Startup

Run the full stack with:

```bash
docker-compose up --build
```

## Service addresses

- Frontend: `http://localhost:8080`
- Backend API: `http://localhost:3000`
- Health check: `http://localhost:3000/api/v1/health`

## Demo credentials

| Username  | Password              | Role              |
|-----------|-----------------------|-------------------|
| `admin`   | `DepotCycleAdmin123!` | Administrator     |
| `shopper` | `DepotCycleDemo123!`  | Shopper           |
| `clerk`   | `DepotCycleDemo123!`  | Inventory Clerk   |
| `manager` | `DepotCycleDemo123!`  | Manager           |
| `support` | `DepotCycleDemo123!`  | Support Agent     |

These are created automatically on first backend startup and can be changed via the admin credential endpoints.

## Verification steps

1. Start the stack:
   ```bash
   docker-compose up --build
   ```

2. Confirm health endpoint:
   ```bash
   curl -s http://localhost:3000/api/v1/health
   ```
   Expected: `{"status":"ok","mode":"offline-local","timestamp_utc":"..."}`

3. Confirm frontend loads:
   Open `http://localhost:8080` in a browser. Expected: login form with role quick-select buttons.

4. Login as admin:
   ```bash
   curl -s -c cookies.txt -X POST http://localhost:3000/api/v1/auth/login \
     -H 'Content-Type: application/json' \
     -d '{"username":"admin","password":"DepotCycleAdmin123!"}'
   ```
   Expected: `{"user_id":"...","username":"admin","role_name":"Administrator",...}`

5. Verify dashboard metrics:
   ```bash
   curl -s -b cookies.txt http://localhost:3000/api/v1/admin/dashboard/metrics
   ```
   Expected: JSON with `total_users >= 5`, `total_feature_flags >= 2`, numeric `conversion_rate_percent` and `average_rating`.

6. Login as shopper in the Leptos UI. Search for "ThinkPad". Expected: at least one listing result with title, price, and campus info.

7. Open a listing detail. Expected: popularity score, inventory on hand count, and recommendation cards.

8. Create an order for 1 unit. Expected: order status "placed" and total matching the listing price.

9. Toggle recommendation settings off. Expected: recommendations list becomes empty.

10. Login as clerk. Create a scrap inventory document with value > $2500. Expected: status "pending_approval".

11. Login as manager. Approve the pending document. Expected: status "executed".

12. Login as support. Create an after-sales return case. Transition to "under_review", then "approved". Expected: each transition succeeds with updated status.

13. Run the unified test suite:
    ```bash
    ./run_tests.sh
    ```
    Expected: all test crates pass (unit_tests, API_tests, frontend_tests, e2e_tests).

14. Run the browser E2E coverage:
    ```bash
    docker-compose up --build
    ```
    Expected: the browser test artifact in `playwright-tests/` covers shopper login, search, detail view, and purchase through the rendered UI. Execute it only in a containerized/CI environment where Playwright dependencies are pre-baked; do not install runtime dependencies on the host.

## Test strategy

- `unit_tests/`: backend unit tests for auth, security, workflows, models, config, DB helpers, middleware, router composition, and bootstrap wiring.
- `API_tests/`: real router-level HTTP tests against the backend with migrations and seeded data; no visible mocking in the request path.
- `frontend_tests/`: frontend contract coverage verifying backend responses deserialize into frontend-facing shapes.
- `frontend/` wasm tests: helper, render, and UI-shell assertions for the actual Leptos application.
- `playwright-tests/`: browser-driven fullstack verification against the rendered UI and live Docker stack.

## Offline notes

Runtime has no external SaaS or network service dependency. SQLite persists to the local `sqlite_data` Docker volume, and uploaded media persists to `media_data`.
