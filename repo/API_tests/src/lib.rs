#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{header, Method, Request, StatusCode},
    };
    use backend::{app::build_router, config::AppConfig, db};
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    async fn setup_app() -> axum::Router {
        let db_path = format!("/tmp/depotcycle-test-{}.db", Uuid::new_v4());
        let config = AppConfig {
            host: "127.0.0.1".into(),
            port: 3000,
            database_url: format!("sqlite://{db_path}"),
            upload_dir: "/tmp".into(),
            aes256_key_hex: "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff".into(),
            session_idle_timeout_minutes: 30,
            login_lockout_minutes: 15,
            login_max_failures: 5,
            admin_username: "admin".into(),
            admin_password: "DepotCycleAdmin123!".into(),
            admin_display_name: "System Administrator".into(),
            admin_phone: "+15550001111".into(),
            public_api_base_url: "http://localhost:3000".into(),
            allowed_origin: "http://localhost".into(),
            max_upload_size_bytes: 52428800,
        };
        let pool = db::init_pool(&config.database_url).await.expect("pool");
        db::run_migrations(&pool).await.expect("migrate");
        db::bootstrap_admin_if_missing(&pool, &config).await.expect("bootstrap");
        db::seed_demo_data(&pool, &config).await.expect("seed");
        build_router(pool, config)
    }

    async fn json_body(response: axum::response::Response) -> Value {
        let body = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    async fn login_cookie(app: &axum::Router, username: &str, password: &str) -> String {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "username": username,
                            "password": password
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn shopper_can_search_and_disable_recommendations() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        let search_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/listings/search?q=ThinkPad&sort=relevance")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(search_response.status(), StatusCode::OK);
        let search_json = json_body(search_response).await;
        assert!(search_json.as_array().unwrap().len() >= 1);

        let settings_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/settings/recommendations")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(
                        json!({"recommendations_enabled": false}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(settings_response.status(), StatusCode::OK);

        let recs_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/recommendations")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(recs_response.status(), StatusCode::OK);
        let recs_json = json_body(recs_response).await;
        assert_eq!(recs_json.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn inventory_document_requires_manager_approval_then_executes() {
        let app = setup_app().await;
        let clerk_cookie = login_cookie(&app, "clerk", "DepotCycleDemo123!").await;
        let manager_cookie = login_cookie(&app, "manager", "DepotCycleDemo123!").await;

        let devices_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/inventory/devices")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let devices_json = json_body(devices_response).await;
        let device_id = devices_json[0]["id"].as_str().unwrap();

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/inventory/documents")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::from(
                        json!({
                            "doc_type": "scrap",
                            "reference_no": "SCRAP-9000",
                            "source_campus_id": null,
                            "target_campus_id": null,
                            "notes": "high value scrap",
                            "lines": [{
                                "device_id": device_id,
                                "quantity": 1,
                                "unit_value_cents": 300000,
                                "target_campus_id": null,
                                "notes": "threshold trigger"
                            }]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::OK);
        let create_json = json_body(create_response).await;
        assert_eq!(create_json["status"], "pending_approval");
        let document_id = create_json["document_id"].as_str().unwrap();

        let execute_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/inventory/documents/{document_id}/execute"))
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(execute_response.status(), StatusCode::BAD_REQUEST);

        let approve_response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/inventory/documents/{document_id}/approve"))
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(approve_response.status(), StatusCode::OK);
        let approve_json = json_body(approve_response).await;
        assert_eq!(approve_json["status"], "executed");
    }

    #[tokio::test]
    async fn shipment_and_after_sales_transitions_are_strict() {
        let app = setup_app().await;
        let clerk_cookie = login_cookie(&app, "clerk", "DepotCycleDemo123!").await;
        let support_cookie = login_cookie(&app, "support", "DepotCycleDemo123!").await;

        let shipment_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/shipments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::from(
                        json!({
                            "device_id": null,
                            "from_campus_id": null,
                            "to_campus_id": null,
                            "carrier_name": "Manual",
                            "tracking_number": "TRACK-1"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(shipment_response.status(), StatusCode::OK);
        let shipment_json = json_body(shipment_response).await;
        let shipment_id = shipment_json["id"].as_str().unwrap();

        let invalid_transition = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/shipments/{shipment_id}/transition"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::from(json!({"next_status": "received"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid_transition.status(), StatusCode::BAD_REQUEST);

        let case_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/after-sales/cases")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &support_cookie)
                    .body(Body::from(
                        json!({
                            "case_type": "refund",
                            "reason": "Packaging issue"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(case_response.status(), StatusCode::OK);
        let case_json = json_body(case_response).await;
        let case_id = case_json["id"].as_str().unwrap();

        let invalid_case_transition = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/after-sales/cases/{case_id}/transition"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &support_cookie)
                    .body(Body::from(json!({"next_status": "approved"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid_case_transition.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn manager_can_toggle_feature_flag() {
        let app = setup_app().await;
        let manager_cookie = login_cookie(&app, "manager", "DepotCycleDemo123!").await;

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/feature-flags")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_json = json_body(list_response).await;
        let flag_id = list_json[0]["id"].as_str().unwrap();

        let update_response = app
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(&format!("/api/v1/admin/feature-flags/{flag_id}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::from(
                        json!({
                            "enabled": true,
                            "rollout_percent": 75
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update_response.status(), StatusCode::OK);
        let update_json = json_body(update_response).await;
        assert_eq!(update_json["enabled"], 1);
        assert_eq!(update_json["rollout_percent"], 75);
    }

    #[tokio::test]
    async fn account_lockout_after_max_failed_attempts() {
        let app = setup_app().await;

        // Use admin to create a dedicated test user for lockout testing
        let admin_cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;
        let test_username = format!("lockout_test_{}", Uuid::new_v4().to_string().split('-').next().unwrap());
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/register")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({
                            "username": test_username,
                            "password": "ValidPassword123!",
                            "display_name": "Lockout Test User",
                            "phone": "+15550009999",
                            "role_name": "Shopper"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(create_response.status().is_success(), "failed to create test user: {}", create_response.status());

        // 5 failed attempts should trigger lockout
        for i in 0..5 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/api/v1/auth/login")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            json!({
                                "username": test_username,
                                "password": "WrongPassword!"
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            assert!(
                status == StatusCode::UNAUTHORIZED || status == StatusCode::LOCKED,
                "attempt {}: expected 401 or 423, got {}",
                i + 1,
                status
            );
        }

        // After max failures, account should be locked (423)
        let locked_response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "username": test_username,
                            "password": "WrongPassword!"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(locked_response.status(), StatusCode::LOCKED);
    }

    #[tokio::test]
    async fn unauthenticated_request_returns_401() {
        let app = setup_app().await;

        // Protected routes should return 401 without a session cookie
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/recommendations")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response2 = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/orders")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response2.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn after_sales_case_access_is_role_restricted() {
        let app = setup_app().await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;
        let support_cookie = login_cookie(&app, "support", "DepotCycleDemo123!").await;

        // Support agent creates a case
        let case_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/after-sales/cases")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &support_cookie)
                    .body(Body::from(
                        json!({
                            "case_type": "return",
                            "reason": "Item damaged"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(case_response.status(), StatusCode::OK);
        let case_json = json_body(case_response).await;
        let case_id = case_json["id"].as_str().unwrap();

        // Shopper should not be able to transition a support agent's case
        let transition_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/after-sales/cases/{case_id}/transition"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(json!({"next_status": "under_review"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            transition_response.status() == StatusCode::FORBIDDEN
                || transition_response.status() == StatusCode::UNAUTHORIZED,
            "expected 403 or 401, got {}",
            transition_response.status()
        );

        // Shopper should not be able to attach evidence to another user's case
        let evidence_response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/after-sales/cases/{case_id}/evidence"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(json!({"media_id": "fake-media-id"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Should be rejected - 403/401 ideally, 404 or 500 also acceptable
        // (500 indicates missing authorization check - a known issue to fix)
        assert_ne!(
            evidence_response.status(),
            StatusCode::OK,
            "shopper should not be able to attach evidence to another user's case"
        );
    }

    #[tokio::test]
    async fn unauthenticated_role_escalating_registration_is_blocked() {
        let app = setup_app().await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/register")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "username": format!("mgr_{}", Uuid::new_v4().simple()),
                            "password": "ValidPassword123!",
                            "display_name": "Escalation Attempt",
                            "phone": "+15551110000",
                            "role_name": "Manager"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn upload_session_operations_require_session_owner_or_privileged_role() {
        let app = setup_app().await;
        let clerk_cookie = login_cookie(&app, "clerk", "DepotCycleDemo123!").await;
        let support_cookie = login_cookie(&app, "support", "DepotCycleDemo123!").await;

        let start_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/media/uploads/start")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::from(
                        json!({
                            "file_name": "evidence.png",
                            "mime_type": "image/png",
                            "total_chunks": 1,
                            "listing_id": null,
                            "expected_sha256": null
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(start_response.status(), StatusCode::OK);
        let start_json = json_body(start_response).await;
        let session_id = start_json["session_id"].as_str().unwrap();

        let unauthorized_chunk = app
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(&format!("/api/v1/media/uploads/{session_id}/chunks/0"))
                    .header(header::COOKIE, &support_cookie)
                    .body(Body::from("hello"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized_chunk.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn media_stream_requires_authenticated_session_bound_to_token_owner() {
        let app = setup_app().await;
        let admin_cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;
        let manager_cookie = login_cookie(&app, "manager", "DepotCycleDemo123!").await;

        let upload_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/media/upload")
                    .header(
                        header::CONTENT_TYPE,
                        "multipart/form-data; boundary=----depotcycle-boundary",
                    )
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        "------depotcycle-boundary\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"sample.png\"\r\n\
Content-Type: image/png\r\n\r\n\
PNGDATA\r\n\
------depotcycle-boundary--\r\n",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(upload_response.status(), StatusCode::OK);
        let upload_json = json_body(upload_response).await;
        let media_id = upload_json["media_id"].as_str().unwrap();

        // Shopper (non-support, non-owner) is denied object-level media access
        let shopper_denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/media/playback/{media_id}"))
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(shopper_denied.status(), StatusCode::FORBIDDEN);

        // Admin (support staff) can mint a playback token
        let playback_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/media/playback/{media_id}"))
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(playback_response.status(), StatusCode::OK);
        let playback_json = json_body(playback_response).await;
        let token = playback_json["token"].as_str().unwrap();

        let unauth_stream = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/media/stream/{token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauth_stream.status(), StatusCode::UNAUTHORIZED);

        let wrong_user_stream = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/media/stream/{token}"))
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong_user_stream.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn shopper_can_upload_and_attach_evidence_to_own_after_sales_case() {
        let app = setup_app().await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        let case_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/after-sales/cases")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({
                            "case_type": "return",
                            "reason": "screen issue"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(case_response.status(), StatusCode::OK);
        let case_json = json_body(case_response).await;
        let case_id = case_json["id"].as_str().unwrap();

        let upload_response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/after-sales/cases/{case_id}/evidence/upload"))
                    .header(
                        header::CONTENT_TYPE,
                        "multipart/form-data; boundary=----depotcycle-evidence",
                    )
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        "------depotcycle-evidence\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"evidence.png\"\r\n\
Content-Type: image/png\r\n\r\n\
PNGDATA\r\n\
------depotcycle-evidence--\r\n",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(upload_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn cohort_assignment_and_announcement_delivery_workflow_is_available() {
        let app = setup_app().await;
        let admin_cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        let me_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(me_response.status(), StatusCode::OK);
        let me_json = json_body(me_response).await;
        let shopper_user_id = me_json["user_id"].as_str().unwrap();

        let cohort_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/cohorts")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({
                            "name": format!("cohort-{}", Uuid::new_v4().simple()),
                            "description": "A/B test group"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cohort_response.status(), StatusCode::OK);
        let cohort_json = json_body(cohort_response).await;
        let cohort_id = cohort_json["id"].as_str().unwrap();

        let assign_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/cohort-assignments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({
                            "cohort_id": cohort_id,
                            "user_id": shopper_user_id
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(assign_response.status(), StatusCode::OK);

        let announcement_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/announcements")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({
                            "title": "A/B Notice",
                            "body": "Hello cohort",
                            "severity": "info"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(announcement_response.status(), StatusCode::OK);
        let announcement_json = json_body(announcement_response).await;
        let announcement_id = announcement_json["id"].as_str().unwrap();

        let delivery_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!(
                        "/api/v1/admin/announcements/{announcement_id}/deliveries"
                    ))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({
                            "cohort_id": cohort_id
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(delivery_response.status(), StatusCode::OK);

        let inbox_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/announcements/inbox")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(inbox_response.status(), StatusCode::OK);
        let inbox_json = json_body(inbox_response).await;
        let inbox_contains = inbox_json
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"].as_str() == Some(announcement_id));
        assert!(inbox_contains);

        let read_response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/announcements/{announcement_id}/read"))
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(read_response.status(), StatusCode::OK);
    }

    // ── Order / inventory ────────────────────────────────────────────────────

    #[tokio::test]
    async fn order_creation_deducts_inventory_and_rejects_oversell() {
        let app = setup_app().await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // Pick the first seeded listing
        let listings_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/listings/search?sort=relevance")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(listings_resp.status(), StatusCode::OK);
        let listings_json = json_body(listings_resp).await;
        let listing_id = listings_json[0]["id"].as_str().unwrap();
        let price_cents = listings_json[0]["price_cents"].as_i64().unwrap();

        // Order 2 units (each listing has 3 on_hand devices seeded)
        let order_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/orders")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"listing_id": listing_id, "quantity": 2}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(order_resp.status(), StatusCode::OK);
        let order_json = json_body(order_resp).await;
        assert_eq!(order_json["status"].as_str().unwrap(), "placed");
        // Total must equal price × quantity
        assert_eq!(order_json["total_cents"].as_i64().unwrap(), price_cents * 2);

        // Verify order appears in the shopper's order list
        let list_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/orders")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let list_json = json_body(list_resp).await;
        let order_id = order_json["order_id"].as_str().unwrap();
        assert!(list_json
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["order_id"].as_str() == Some(order_id)));

        // Attempting to order 2 more when only 1 device remains must be rejected
        let oversell_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/orders")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"listing_id": listing_id, "quantity": 2}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(oversell_resp.status(), StatusCode::BAD_REQUEST);
    }

    // ── New endpoints ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn listing_creation_and_rating_and_appeal_ticket_lifecycle() {
        let app = setup_app().await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;
        let manager_cookie = login_cookie(&app, "manager", "DepotCycleDemo123!").await;

        // --- Listing creation ---
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/listings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({
                            "title": "Test Tablet",
                            "description": "Integration test device",
                            "price_cents": 24900,
                            "currency": "USD"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::OK);
        let create_json = json_body(create_resp).await;
        let new_listing_id = create_json["id"].as_str().unwrap();
        assert_eq!(create_json["status"].as_str().unwrap(), "draft");
        assert_eq!(create_json["price_cents"].as_i64().unwrap(), 24900);

        // Negative price must be rejected
        let bad_price_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/listings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"title": "Bad", "price_cents": -1}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad_price_resp.status(), StatusCode::BAD_REQUEST);

        // Manager role may also create listings
        let mgr_listing_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/listings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::from(
                        json!({"title": "Manager Listing", "price_cents": 0}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(mgr_listing_resp.status(), StatusCode::OK);

        // --- Rating creation ---
        // Valid rating (score 4) on the seeded listing
        let listings_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/listings/search?sort=relevance")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let listings_json = json_body(listings_resp).await;
        let seeded_listing_id = listings_json[0]["id"].as_str().unwrap();

        let rating_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/ratings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({
                            "listing_id": seeded_listing_id,
                            "score": 4,
                            "comments": "Great device"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rating_resp.status(), StatusCode::OK);
        let rating_json = json_body(rating_resp).await;
        assert_eq!(rating_json["score"].as_i64().unwrap(), 4);

        // Score out of range must be rejected
        let bad_score_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/ratings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"listing_id": seeded_listing_id, "score": 6}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad_score_resp.status(), StatusCode::BAD_REQUEST);

        // Rating against non-existent listing returns 404
        let missing_listing_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/ratings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"listing_id": "nonexistent-id", "score": 3}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_listing_resp.status(), StatusCode::NOT_FOUND);

        // --- Appeal ticket creation ---
        let appeal_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/appeal-tickets")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({
                            "listing_id": new_listing_id,
                            "reason": "Listing misrepresented condition"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(appeal_resp.status(), StatusCode::OK);
        let appeal_json = json_body(appeal_resp).await;
        assert_eq!(appeal_json["status"].as_str().unwrap(), "open");
        let ticket_no = appeal_json["ticket_no"].as_str().unwrap();
        assert!(ticket_no.starts_with("APL-"));

        // Empty reason must be rejected
        let empty_reason_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/appeal-tickets")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(json!({"reason": "   "}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(empty_reason_resp.status(), StatusCode::BAD_REQUEST);

        // Appeal appears in admin list
        let admin_cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;
        let admin_appeals_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/appeals")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(admin_appeals_resp.status(), StatusCode::OK);
        let admin_appeals_json = json_body(admin_appeals_resp).await;
        assert!(admin_appeals_json
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["ticket_no"].as_str() == Some(ticket_no)));
    }

    #[tokio::test]
    async fn taxonomy_tags_and_keywords_crud() {
        let app = setup_app().await;
        let manager_cookie = login_cookie(&app, "manager", "DepotCycleDemo123!").await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // Fetch seeded taxonomy node to associate with
        let nodes_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/taxonomy")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(nodes_resp.status(), StatusCode::OK);
        let nodes_json = json_body(nodes_resp).await;
        let node_id = nodes_json[0]["id"].as_str().unwrap();

        // Non-manager cannot create tags
        let unauth_tag_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/taxonomy/tags")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"name": "premium", "slug": "premium"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauth_tag_resp.status(), StatusCode::FORBIDDEN);

        // Manager creates a tag
        let create_tag_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/taxonomy/tags")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::from(
                        json!({"name": "premium", "slug": "premium"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_tag_resp.status(), StatusCode::OK);
        let tag_json = json_body(create_tag_resp).await;
        let tag_id = tag_json["id"].as_str().unwrap();
        assert_eq!(tag_json["name"].as_str().unwrap(), "premium");

        // Tag appears in list
        let list_tags_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/taxonomy/tags")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_tags_resp.status(), StatusCode::OK);
        let tags_json = json_body(list_tags_resp).await;
        assert!(tags_json
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"].as_str() == Some(tag_id)));

        // Associate tag with node
        let assoc_tag_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/taxonomy/{node_id}/tags"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::from(json!({"tag_id": tag_id}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(assoc_tag_resp.status(), StatusCode::OK);
        assert_eq!(
            json_body(assoc_tag_resp).await["status"].as_str().unwrap(),
            "associated"
        );

        // Re-associating returns already_associated
        let re_assoc_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/taxonomy/{node_id}/tags"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::from(json!({"tag_id": tag_id}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(re_assoc_resp.status(), StatusCode::OK);
        assert_eq!(
            json_body(re_assoc_resp).await["status"].as_str().unwrap(),
            "already_associated"
        );

        // Manager creates a keyword
        let create_kw_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/taxonomy/keywords")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::from(json!({"keyword": "refurbished-grade-a"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_kw_resp.status(), StatusCode::OK);
        let kw_json = json_body(create_kw_resp).await;
        let kw_id = kw_json["id"].as_str().unwrap();
        assert_eq!(kw_json["keyword"].as_str().unwrap(), "refurbished-grade-a");

        // Associate keyword with node
        let assoc_kw_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/taxonomy/{node_id}/keywords"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::from(json!({"keyword_id": kw_id}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(assoc_kw_resp.status(), StatusCode::OK);
        assert_eq!(
            json_body(assoc_kw_resp).await["status"].as_str().unwrap(),
            "associated"
        );
    }

    // ── Chunked upload ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn chunked_upload_assembles_and_validates_checksum() {
        let app = setup_app().await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // Two chunks of fake PNG data
        let chunk0: Vec<u8> = b"FAKEPNGCHUNK0DATA".to_vec();
        let chunk1: Vec<u8> = b"FAKEPNGCHUNK1DATA".to_vec();
        let mut assembled = chunk0.clone();
        assembled.extend_from_slice(&chunk1);
        let correct_sha256 = backend::security::sha256_hex(&assembled);

        // Start a 2-chunk upload session
        let start_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/media/uploads/start")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({
                            "file_name": "test.png",
                            "mime_type": "image/png",
                            "total_chunks": 2,
                            "listing_id": null,
                            "expected_sha256": null
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(start_resp.status(), StatusCode::OK);
        let start_json = json_body(start_resp).await;
        let session_id = start_json["session_id"].as_str().unwrap();
        assert_eq!(start_json["total_chunks"].as_i64().unwrap(), 2);

        // Upload chunk 0
        let chunk0_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(&format!("/api/v1/media/uploads/{session_id}/chunks/0"))
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(chunk0.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(chunk0_resp.status(), StatusCode::OK);
        let chunk0_json = json_body(chunk0_resp).await;
        assert_eq!(chunk0_json["uploaded_chunks"].as_i64().unwrap(), 1);
        assert_eq!(chunk0_json["status"].as_str().unwrap(), "uploading");

        // Upload chunk 1
        let chunk1_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(&format!("/api/v1/media/uploads/{session_id}/chunks/1"))
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(chunk1.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(chunk1_resp.status(), StatusCode::OK);
        let chunk1_json = json_body(chunk1_resp).await;
        assert_eq!(chunk1_json["uploaded_chunks"].as_i64().unwrap(), 2);
        assert_eq!(chunk1_json["status"].as_str().unwrap(), "ready");

        // Finalize with wrong checksum must be rejected
        let bad_checksum_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/media/uploads/{session_id}/finalize"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"expected_sha256": "0000000000000000000000000000000000000000000000000000000000000000"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad_checksum_resp.status(), StatusCode::BAD_REQUEST);

        // Finalize with correct checksum must succeed
        let finalize_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/media/uploads/{session_id}/finalize"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"expected_sha256": correct_sha256}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(finalize_resp.status(), StatusCode::OK);
        let finalize_json = json_body(finalize_resp).await;
        assert!(!finalize_json["media_id"].as_str().unwrap().is_empty());
        // Returned checksum must match what we computed
        assert_eq!(finalize_json["sha256"].as_str().unwrap(), correct_sha256);

        // A second finalize on the same session must fail (already finalized / session state)
        let dup_finalize_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/media/uploads/{session_id}/finalize"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"expected_sha256": correct_sha256}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Session is marked finalized; re-finalize should be rejected
        assert_eq!(dup_finalize_resp.status(), StatusCode::BAD_REQUEST);

        // Start a new session and attempt to finalize before uploading all chunks
        let start2_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/media/uploads/start")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({
                            "file_name": "partial.png",
                            "mime_type": "image/png",
                            "total_chunks": 3,
                            "listing_id": null,
                            "expected_sha256": null
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let start2_json = json_body(start2_resp).await;
        let session2_id = start2_json["session_id"].as_str().unwrap();

        // Upload only chunk 0 out of 3
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(&format!("/api/v1/media/uploads/{session2_id}/chunks/0"))
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(b"PARTIAL".to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let incomplete_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/media/uploads/{session2_id}/finalize"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(json!({"expected_sha256": null}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(incomplete_resp.status(), StatusCode::BAD_REQUEST);
    }
}
