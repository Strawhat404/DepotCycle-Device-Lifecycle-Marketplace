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

Admin authorization is endpoint-specific:
- `Administrator` only: credentials/templates/announcements creation-media upload and announcement delivery.
- `Administrator` or `Manager`: dashboard metrics, feature flags, cohort assignment/read.
- `Administrator`, `Support Agent`, or `Manager`: ratings review and appeals.

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
- `POST /admin/announcements/:announcement_id/deliveries`
  - Deliver an announcement to specific users, a cohort, or all users.
- `GET /admin/announcements/:announcement_id/deliveries`
  - Review delivery/read state per user.
- `GET /admin/cohorts`
- `POST /admin/cohorts`
- `GET /admin/cohort-assignments`
- `POST /admin/cohort-assignments`
  - Manage local cohort assignment state for A/B operations.
- `GET /admin/dashboard/metrics`
  - Aggregates counts from local event logs, users, announcements, templates, uploads, shipments, and flags.
- `POST /admin/media/upload`
  - Multipart upload endpoint for photos/videos stored on local disk with MIME validation and SHA-256 fingerprinting.

## After-sales and Announcements

- `POST /after-sales/cases/:case_id/evidence/upload`
  - Authenticated multipart evidence upload scoped to the caller's authorized after-sales case; media is attached automatically.
- `POST /after-sales/cases/:case_id/evidence`
  - Attach an existing media id to an authorized after-sales case.
- `GET /announcements/inbox`
  - Returns announcements delivered to the current authenticated user.
- `POST /announcements/:announcement_id/read`
  - Marks the caller's delivery for an announcement as read.

## Response conventions

- Success responses return JSON.
- Authenticated endpoints rely on the `depotcycle_session` cookie.
- Error responses return:

```json
{
  "error": "message"
}
```
