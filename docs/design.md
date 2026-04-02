# DepotCycle Part 1 Design

## Architecture

The foundation is split into two Rust services inside `repo/`:

- Axum backend: REST API, SQLite access, session/auth handling, local file uploads, audit trails, and admin console preparation endpoints.
- Leptos frontend: WASM-based shell for the future marketplace and admin console.

SQLite is the only data store. The database file is persisted under `/data/depotcycle.db` inside the backend container and backed by the `sqlite_data` Docker volume. Uploaded media is stored on the local filesystem under `/app/uploads` and backed by the `media_data` Docker volume.

## Security design

- Passwords are stored with Argon2id hashes only.
- Password policy enforces a minimum length of 12 characters.
- Login lockout is persisted in SQLite and enforced after 5 failed attempts for 15 minutes.
- Sessions are stored server-side in SQLite and expire after 30 minutes of inactivity.
- Sensitive fields are encrypted at rest with AES-256-GCM using an application key from `.env`.
- Audit trails are append-only, and SQLite triggers reject update/delete operations against immutable audit tables.
- Media uploads are stored locally after MIME validation, SHA-256 fingerprinting, and metadata recording.

## Database design

### Identity and access

- `roles`
- `users`
- `sessions`
- `local_credentials`
- `companion_credentials`
- `admin_audit_trails`

### Listings and taxonomy

- `campuses`
- `listing_conditions`
- `taxonomy_nodes`
- `taxonomy_tags`
- `taxonomy_keywords`
- `taxonomy_node_tags`
- `taxonomy_node_keywords`
- `listings`
- `listing_media`
- `media_chunks`

### Inventory and operational controls

- `inventory_devices`
- `inventory_documents`
- `ledger_change_records`

### Logistics and after-sales

- `shipment_orders`
- `shipment_status_history`
- `service_level_agreements`
- `ratings`
- `appeal_tickets`

### Feature delivery and analytics

- `event_logs`
- `feature_flags`
- `cohorts`
- `cohort_assignments`

### Admin configuration

- `templates`
- `announcements`
- `announcement_deliveries`

## Migration strategy

Part 1 uses SQLx migrations embedded in the backend binary. On startup the backend:

1. Opens or creates the SQLite database.
2. Applies all pending migrations.
3. Ensures the role seed data exists.
4. Bootstraps the first administrator from `.env` if no users exist.

## API foundation boundaries

Part 1 intentionally stops at foundational infrastructure:

- Auth/session endpoints are active.
- Admin management and metrics endpoints are active.
- Schema exists for listings, inventory, logistics, flags, and analytics.
- Core marketplace workflows, fulfillment workflows, and rich UI flows are deferred to Part 2.

