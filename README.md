# DepotCycle Device Lifecycle Marketplace

Part 1 + Part 2 implementation for the offline-first DepotCycle marketplace. This repo contains:

- `backend/`: Axum API, SQLite schema/migrations, auth/session/security foundation, and admin API shell.
- `frontend/`: Leptos WASM shell for the future admin/client UI.
- `unit_tests/`: unit-level Rust test crate.
- `API_tests/`: API functional Rust test crate.

## Startup

Run the full stack with:

```bash
docker compose up 
```

## Service addresses

- Frontend: `http://localhost:8080`
- Backend API: `http://localhost:3000`
- Health check: `http://localhost:3000/api/v1/health`

## Demo credentials

- Username: `admin`
- Password: `DepotCycleAdmin123!`

Additional seeded users:

- `shopper` / `DepotCycleDemo123!`
- `clerk` / `DepotCycleDemo123!`
- `manager` / `DepotCycleDemo123!`
- `support` / `DepotCycleDemo123!`

These are created automatically on first backend startup and can be changed via the admin credential endpoints.

## Verification steps

1. Start the stack with `docker compose up --build`.
2. Visit `http://localhost:8080` and confirm the shell loads.
3. Request `GET /api/v1/health` and confirm a healthy JSON response.
4. Log in with one of the seeded credentials in the Leptos UI.
5. Verify discovery, recommendations, inventory document processing, shipment transitions, and after-sales timelines from the browser.
6. Call `GET /api/v1/admin/dashboard/metrics` with an admin or manager session.
7. Run the unified test command:

```bash
./run_tests.sh
```

## Offline notes

Runtime has no external SaaS or network service dependency. SQLite persists to the local `sqlite_data` Docker volume, and uploaded media persists to `media_data`.
