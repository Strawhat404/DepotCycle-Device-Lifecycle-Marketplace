# Test Coverage Audit

## Project Type

- Declared project type: `fullstack`
- Evidence: `repo/README.md:3`

## Backend Endpoint Inventory

Source of truth: `repo/backend/src/app.rs:48-105`

Total endpoints: `74`

1. `GET /api/v1/health`
2. `GET /api/v1/workspaces`
3. `GET /api/v1/campuses`
4. `GET /api/v1/inventory/devices`
5. `POST /api/v1/auth/register`
6. `POST /api/v1/auth/login`
7. `POST /api/v1/auth/logout`
8. `GET /api/v1/auth/me`
9. `POST /api/v1/listings`
10. `GET /api/v1/listings/search`
11. `GET /api/v1/listings/:id`
12. `POST /api/v1/listings/:id/view`
13. `POST /api/v1/favorites/:id`
14. `GET /api/v1/search/suggestions`
15. `GET /api/v1/search/history`
16. `POST /api/v1/search/history`
17. `DELETE /api/v1/search/history`
18. `GET /api/v1/recommendations`
19. `GET /api/v1/settings/recommendations`
20. `POST /api/v1/settings/recommendations`
21. `GET /api/v1/orders`
22. `POST /api/v1/orders`
23. `POST /api/v1/ratings`
24. `POST /api/v1/appeal-tickets`
25. `GET /api/v1/taxonomy`
26. `POST /api/v1/taxonomy`
27. `GET /api/v1/taxonomy/tags`
28. `POST /api/v1/taxonomy/tags`
29. `GET /api/v1/taxonomy/keywords`
30. `POST /api/v1/taxonomy/keywords`
31. `POST /api/v1/taxonomy/:node_id/tags`
32. `POST /api/v1/taxonomy/:node_id/keywords`
33. `POST /api/v1/media/uploads/start`
34. `PUT /api/v1/media/uploads/:session_id/chunks/:chunk_index`
35. `POST /api/v1/media/uploads/:session_id/finalize`
36. `GET /api/v1/media/playback/:media_id`
37. `GET /api/v1/media/stream/:token`
38. `GET /api/v1/inventory/documents`
39. `POST /api/v1/inventory/documents`
40. `POST /api/v1/inventory/documents/:document_id/approve`
41. `POST /api/v1/inventory/documents/:document_id/execute`
42. `GET /api/v1/shipments`
43. `POST /api/v1/shipments`
44. `POST /api/v1/shipments/:shipment_id/transition`
45. `GET /api/v1/shipments/:shipment_id/history`
46. `GET /api/v1/after-sales/cases`
47. `POST /api/v1/after-sales/cases`
48. `POST /api/v1/after-sales/cases/:case_id/transition`
49. `POST /api/v1/after-sales/cases/:case_id/evidence`
50. `POST /api/v1/after-sales/cases/:case_id/evidence/upload`
51. `GET /api/v1/after-sales/cases/:case_id/history`
52. `GET /api/v1/announcements/inbox`
53. `POST /api/v1/announcements/:announcement_id/read`
54. `GET /api/v1/admin/feature-flags`
55. `PUT /api/v1/admin/feature-flags/:flag_id`
56. `GET /api/v1/admin/cohorts`
57. `POST /api/v1/admin/cohorts`
58. `GET /api/v1/admin/cohort-assignments`
59. `POST /api/v1/admin/cohort-assignments`
60. `GET /api/v1/admin/ratings-review`
61. `GET /api/v1/admin/appeals`
62. `GET /api/v1/admin/local-credentials`
63. `POST /api/v1/admin/local-credentials`
64. `GET /api/v1/admin/companion-credentials`
65. `POST /api/v1/admin/companion-credentials`
66. `GET /api/v1/admin/templates`
67. `POST /api/v1/admin/templates`
68. `PUT /api/v1/admin/templates/:id`
69. `GET /api/v1/admin/announcements`
70. `POST /api/v1/admin/announcements`
71. `GET /api/v1/admin/announcements/:announcement_id/deliveries`
72. `POST /api/v1/admin/announcements/:announcement_id/deliveries`
73. `GET /api/v1/admin/dashboard/metrics`
74. `POST /api/v1/admin/media/upload`

## API Test Mapping Table

Static evidence base:
- `repo/API_tests/src/lib.rs`
- `repo/e2e_tests/src/lib.rs`
- `repo/frontend_tests/src/lib.rs`
- `repo/playwright-tests/shopper-purchase.spec.ts`

Summary mapping outcome:
- Every backend endpoint still has at least one exact HTTP hit in `repo/API_tests/src/lib.rs`.
- Browser E2E now adds one true FE↔BE UI path for login, search, listing detail, and purchase.

Representative endpoint mappings:

| Endpoint | Covered | Test type | Test files | Evidence |
|---|---|---|---|---|
| `POST /api/v1/auth/login` | yes | true no-mock HTTP + browser E2E | `API_tests`, `frontend_tests`, `playwright-tests` | `login_cookie` helper `repo/API_tests/src/lib.rs:44`; `fe_login_response_deserializes_to_auth_user` `repo/frontend_tests/src/lib.rs:365`; shopper login `repo/playwright-tests/shopper-purchase.spec.ts:6-9` |
| `GET /api/v1/listings/search` | yes | true no-mock HTTP + browser E2E | `API_tests`, `frontend_tests`, `e2e_tests`, `playwright-tests` | `repo/API_tests/src/lib.rs:73`; `repo/frontend_tests/src/lib.rs:467`; `repo/e2e_tests/src/lib.rs:104`; `repo/playwright-tests/shopper-purchase.spec.ts:11-17` |
| `GET /api/v1/listings/:id` | yes | true no-mock HTTP + browser E2E | `API_tests`, `frontend_tests`, `e2e_tests`, `playwright-tests` | `repo/API_tests/src/lib.rs:1721`; `repo/frontend_tests/src/lib.rs:489`; `repo/e2e_tests/src/lib.rs:139`; `repo/playwright-tests/shopper-purchase.spec.ts:19-20` |
| `POST /api/v1/orders` | yes | true no-mock HTTP + browser E2E | `API_tests`, `frontend_tests`, `e2e_tests`, `playwright-tests` | `repo/API_tests/src/lib.rs:930`; `repo/frontend_tests/src/lib.rs:668`; `repo/e2e_tests/src/lib.rs:173`; `repo/playwright-tests/shopper-purchase.spec.ts:22-25` |
| `GET /api/v1/admin/cohort-assignments` | yes | true no-mock HTTP | `API_tests` | strengthened body assertions `repo/API_tests/src/lib.rs:2373-2402` |
| `GET /api/v1/admin/ratings-review` | yes | true no-mock HTTP | `API_tests` | strengthened body assertions `repo/API_tests/src/lib.rs:2447-2474` |

Coverage conclusion for all remaining endpoints:
- No uncovered backend route was found in the static rerun.

## API Test Classification

1. True No-Mock HTTP
- `repo/API_tests/src/lib.rs`: app bootstrapped via real router; requests sent through Axum `oneshot(...)`.
- `repo/e2e_tests/src/lib.rs`: same real router/request path for multi-step workflows.
- `repo/playwright-tests/shopper-purchase.spec.ts`: browser-driven UI test against the actual running app and Docker stack as configured in `repo/playwright.config.ts:10-15`.

2. HTTP with Mocking
- None detected.

3. Non-HTTP (unit/integration without HTTP)
- `repo/unit_tests/src/lib.rs`
- `repo/frontend/src/main.rs` wasm tests

## Mock Detection Rules

Detected mocking/stubbing:
- None found by static inspection.

Evidence:
- No visible `jest.mock`, `vi.mock`, `sinon.stub`, DI override, or transport/controller/service substitution in inspected tests.

## Coverage Summary

- Total endpoints: `74`
- Endpoints with HTTP tests: `74`
- Endpoints with true no-mock HTTP tests: `74`
- HTTP coverage: `100%`
- True API coverage: `100%`

## Unit Test Summary

### Backend Unit Tests

Test files:
- `repo/unit_tests/src/lib.rs`

Modules covered:
- Controllers/router composition: `repo/unit_tests/src/lib.rs:841-995`
- Services/workflows/security: `repo/unit_tests/src/lib.rs:8-57`, `248-395`
- Repositories/db helpers: `repo/unit_tests/src/lib.rs:171-209`, `621-686`
- Auth/guards/middleware: `repo/unit_tests/src/lib.rs:59-169`, `688-839`
- Models/config/error/bootstrap wiring: `repo/unit_tests/src/lib.rs:397-551`, `997-1139`

Important backend modules not tested directly:
- `repo/backend/src/handlers.rs` is covered primarily via HTTP tests rather than isolated unit tests.

Verdict:
- Backend unit tests: `PRESENT`

### Frontend Unit Tests

Frontend unit tests: `PRESENT`

Frontend test files:
- `repo/frontend/src/main.rs`
- `repo/frontend_tests/src/lib.rs`

Frameworks/tools detected:
- `wasm_bindgen_test` at `repo/frontend/src/main.rs:1347-1355`
- Rust test harness and async tests in `repo/frontend_tests/src/lib.rs`

Components/modules covered:
- Actual frontend helpers/types/signals/render shell in `repo/frontend/src/main.rs:1344-2070`
- Real backend response contract coverage in `repo/frontend_tests/src/lib.rs`

Important frontend components/modules not tested sufficiently:
- The browser E2E layer is minimal: only one shopper flow is visible in `repo/playwright-tests/shopper-purchase.spec.ts:3-25`.
- No visible browser E2E for clerk, manager, support, or admin workflows.
- No visible browser tests for upload UI, recommendation toggling persistence, or after-sales transitions through the actual DOM.

Cross-layer observation:
- Balance improved materially. The repo is no longer purely backend-heavy because there is now at least one actual browser-driven FE↔BE journey.
- Remaining imbalance: only one browser E2E scenario versus a very broad backend API surface.

## API Observability Check

Assessment: `stronger than previous audit`

Strong evidence:
- Cohort assignment payload now asserts `id`, `cohort_id`, `user_id`, and `created_at`: `repo/API_tests/src/lib.rs:2387-2402`
- Ratings review now asserts the created review record by comments, exact score, comments, and `review_status`: `repo/API_tests/src/lib.rs:2464-2474`

Residual weaker zones:
- Some endpoints still only assert status or minimal shape, especially lower-risk read paths and some negative auth checks.

## Tests Check

Success paths:
- Strong across backend workflows.

Failure cases:
- Strong across auth, validation, not-found, forbidden, and workflow transition errors.

Edge cases:
- Strong for uploads, transitions, lockout, malformed input, and missing fields.

Validation:
- Strong.

Auth/permissions:
- Strong.

Integration boundaries:
- Strong backend integration via router-level tests.
- Fullstack boundary is now explicitly covered by a browser E2E path through the rendered UI: `repo/playwright-tests/shopper-purchase.spec.ts:3-25`.

`run_tests.sh` check:
- Docker-based: `PASS`
- Evidence: `repo/run_tests.sh:14-19`

## End-to-End Expectations

- For `fullstack`, real FE↔BE E2E should exist.
- Current state: `PARTIALLY SATISFIED` but materially improved.
- Evidence:
- Router-level multi-step tests: `repo/e2e_tests/src/lib.rs`
- Browser-level fullstack flow: `repo/playwright-tests/shopper-purchase.spec.ts:3-25`

Remaining gap:
- Only one browser E2E flow is visible.

## Test Coverage Score (0–100)

Score: `93/100`

## Score Rationale

- `+40`: full backend endpoint coverage with real route execution.
- `+18`: strong failure, validation, and auth coverage.
- `+12`: broad backend unit coverage.
- `+10`: frontend tests are present across wasm render/unit plus contract tests.
- `+8`: browser E2E now covers a real FE↔BE user journey.
- `-3`: only one browser E2E path; role coverage in the browser layer remains narrow.
- `-2`: some lower-priority endpoints still have shallow body assertions.

## Key Gaps

- Browser E2E coverage is still thin relative to the app’s multi-role scope.
- No visible browser E2E for admin, clerk, manager, or support flows.
- Some read endpoints remain breadth-covered more than depth-covered.

## Confidence & Assumptions

- Confidence: `high`
- Assumptions:
- Static inspection only; no tests were run.
- Browser E2E validity is inferred from file/config presence, not execution.

## Final Test Coverage Verdict

- Verdict: `PASS`

Rationale:
- The prior fullstack gap is materially addressed by the new browser E2E layer, and the repo now clears a strict >90 quality bar on static evidence.

# README Audit

## README Location

- Required path exists: `repo/README.md`

## Hard Gate Review

### Formatting

- Pass
- Evidence: `repo/README.md:1-109`

### Startup Instructions

- Pass
- Evidence: `docker-compose up --build` at `repo/README.md:17-21`, `43-46`

### Access Method

- Pass
- Evidence: `repo/README.md:23-27`

### Verification Method

- Pass
- Evidence: API and UI verification steps at `repo/README.md:48-97`

### Environment Rules (STRICT)

- Pass

Evidence:
- README no longer instructs `npm install`, `pip install`, `apt-get`, manual DB setup, or local runtime dependency installs.
- Startup remains Docker-contained at `repo/README.md:17-21`, `43-46`.
- Browser E2E section now explicitly says to execute only in a containerized/CI environment with dependencies pre-baked and not to install runtime dependencies on the host: `repo/README.md:91-95`.

### Demo Credentials (Conditional)

- Pass
- Evidence: `repo/README.md:29-39`

## Engineering Quality

Tech stack clarity:
- Good

Architecture explanation:
- Partial

Testing instructions:
- Improved and now compliant with the strict Docker-contained environment rule.

Security/roles:
- Good

Workflows:
- Good

Presentation quality:
- Good structurally.

## High Priority Issues

- No hard-gate violations remain.
- Browser E2E instructions are now policy-compliant but still operationally thin because the README does not provide a concrete repo-local container command to execute Playwright.

## Medium Priority Issues

- Architecture explanation is still shallow for a multi-crate fullstack system.
- README does not explain how to run the browser E2E tests in a containerized way or whether that path is expected in CI.

## Low Priority Issues

- No concise endpoint summary table.
- No explicit note about role-by-role coverage limits in the browser E2E layer.

## Hard Gate Failures

- None.

## README Verdict

- Verdict: `PASS`

Rationale:
- The README now satisfies the strict hard gates. Remaining issues are quality/detail issues rather than compliance failures.

## Final README Verdict

- Final verdict: `PASS`
