# DepotCycle API Spec

Base path: `/api/v1`

## Health

- `GET /health`
  - Returns service status, storage mode, and UTC timestamp.

## Authentication

- `POST /auth/register`
  - Creates a user with username, password, role, encrypted display name, and encrypted phone.
  - Password must be at least 12 characters.
- `POST /auth/login`
  - Validates credentials and sets the `depotcycle_session` HTTP-only cookie.
  - Enforces lockout after 5 failed attempts for 15 minutes.
- `POST /auth/logout`
  - Invalidates the active session.
- `GET /auth/me`
  - Returns the authenticated user context and session metadata.

## Admin Console Preparation

All admin endpoints require an authenticated `Administrator` session.

- `GET /admin/local-credentials`
- `POST /admin/local-credentials`
  - Create local credentials with encrypted secrets and append-only audit logging.
- `GET /admin/companion-credentials`
- `POST /admin/companion-credentials`
  - Store companion credentials and related endpoint/provider metadata.
- `GET /admin/templates`
- `POST /admin/templates`
- `PUT /admin/templates/:id`
  - Manage editable templates and content configuration.
- `GET /admin/announcements`
- `POST /admin/announcements`
  - Create in-app announcements only. No external push integration is used.
- `GET /admin/dashboard/metrics`
  - Aggregates counts from local event logs, users, announcements, templates, uploads, shipments, and flags.
- `POST /admin/media/upload`
  - Multipart upload endpoint for photos/videos stored on local disk with MIME validation and SHA-256 fingerprinting.

## Response conventions

- Success responses return JSON.
- Authenticated endpoints rely on the `depotcycle_session` cookie.
- Error responses return:

```json
{
  "error": "message"
}
```

