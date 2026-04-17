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
        let invalid_body = json_body(invalid_transition).await;
        assert!(invalid_body["error"].as_str().unwrap().contains("transition"), "invalid transition error should mention transition");

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
        let invalid_case_body = json_body(invalid_case_transition).await;
        assert!(invalid_case_body["error"].as_str().unwrap().contains("transition"), "invalid case transition error should mention transition");
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
        let unauth_body = json_body(response).await;
        assert!(unauth_body["error"].as_str().is_some(), "401 response should include error message");

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
        let unauth_body2 = json_body(response2).await;
        assert!(unauth_body2["error"].as_str().is_some());
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
        // Should be rejected with 403 Forbidden (user lacks case access)
        assert_eq!(
            evidence_response.status(),
            StatusCode::FORBIDDEN,
            "shopper should be denied with 403, got {}",
            evidence_response.status()
        );
        let evidence_body = json_body(evidence_response).await;
        assert!(evidence_body["error"].as_str().is_some(), "error response should have 'error' field");
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
        let escalation_body = json_body(response).await;
        assert!(escalation_body["error"].as_str().is_some(), "role escalation rejection should include error message");
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
        let oversell_body = json_body(oversell_resp).await;
        assert!(oversell_body["error"].as_str().unwrap().contains("inventory"), "oversell error should mention inventory");
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
        let bad_price_body = json_body(bad_price_resp).await;
        assert!(bad_price_body["error"].as_str().unwrap().contains("price"), "negative price error should mention price");

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
        let bad_score_body = json_body(bad_score_resp).await;
        assert!(bad_score_body["error"].as_str().unwrap().contains("score"), "bad score error should mention score");

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
        let bad_checksum_body = json_body(bad_checksum_resp).await;
        assert!(bad_checksum_body["error"].as_str().unwrap().contains("checksum"), "checksum failure should mention checksum");

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

    // ── Phase 1: Public / system endpoint tests ─────────────────────────────

    #[tokio::test]
    async fn health_returns_ok_with_expected_shape() {
        let app = setup_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert_eq!(body["status"].as_str().unwrap(), "ok");
        assert_eq!(body["mode"].as_str().unwrap(), "offline-local");
        assert!(body["timestamp_utc"].as_str().is_some());
    }

    #[tokio::test]
    async fn workspaces_returns_all_roles() {
        let app = setup_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/workspaces")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        let arr = body.as_array().unwrap();
        assert!(arr.len() >= 5, "expected at least 5 workspace roles");
        let role_names: Vec<&str> = arr
            .iter()
            .filter_map(|w| w["role_name"].as_str())
            .collect();
        assert!(role_names.contains(&"Shopper"));
        assert!(role_names.contains(&"Administrator"));
        for workspace in arr {
            assert!(workspace["capabilities"].as_array().unwrap().len() >= 1);
        }
    }

    #[tokio::test]
    async fn campuses_returns_seeded_campuses() {
        let app = setup_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/campuses")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        let arr = body.as_array().unwrap();
        assert!(arr.len() >= 3, "expected at least 3 seeded campuses");
        for campus in arr {
            assert!(campus["id"].as_str().is_some());
            assert!(campus["label"].as_str().is_some());
        }
    }

    // ── Auth lifecycle: logout ──────────────────────────────────────────────

    #[tokio::test]
    async fn logout_invalidates_session() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // /auth/me works while logged in
        let me_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(me_resp.status(), StatusCode::OK);

        // Logout
        let logout_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/logout")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logout_resp.status(), StatusCode::OK);
        let logout_body = json_body(logout_resp).await;
        assert_eq!(logout_body["status"].as_str().unwrap(), "logged_out");

        // /auth/me should now return 401
        let me_resp2 = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(me_resp2.status(), StatusCode::UNAUTHORIZED);
    }

    // ── Search / discovery tests ────────────────────────────────────────────

    #[tokio::test]
    async fn listing_detail_and_view_tracking_and_favorites() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // Get a seeded listing ID via search
        let search_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/listings/search?sort=relevance")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(search_resp.status(), StatusCode::OK);
        let listings = json_body(search_resp).await;
        let listing_id = listings[0]["id"].as_str().unwrap();

        // GET /api/v1/listings/:id
        let detail_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/listings/{listing_id}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(detail_resp.status(), StatusCode::OK);
        let detail = json_body(detail_resp).await;
        assert_eq!(detail["listing"]["id"].as_str().unwrap(), listing_id);
        assert!(detail["listing"]["title"].as_str().is_some());
        assert!(detail["popularity_score"].is_number());
        assert!(detail["inventory_on_hand"].is_number());

        // POST /api/v1/listings/:id/view
        let view_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/listings/{listing_id}/view"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(view_resp.status(), StatusCode::OK);
        let view_body = json_body(view_resp).await;
        assert_eq!(view_body["status"].as_str().unwrap(), "recorded");

        // POST /api/v1/favorites/:id  — toggle on
        let fav_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/favorites/{listing_id}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(fav_resp.status(), StatusCode::OK);
        let fav_body = json_body(fav_resp).await;
        assert_eq!(fav_body["favorited"].as_bool().unwrap(), true);

        // POST /api/v1/favorites/:id  — toggle off
        let unfav_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/favorites/{listing_id}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unfav_resp.status(), StatusCode::OK);
        let unfav_body = json_body(unfav_resp).await;
        assert_eq!(unfav_body["favorited"].as_bool().unwrap(), false);
    }

    #[tokio::test]
    async fn search_suggestions_returns_expected_shape() {
        let app = setup_app().await;

        // Suggestions endpoint is public (no auth required by handler, but session middleware runs)
        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/search/suggestions?q=Think")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert!(body["suggestions"].as_array().is_some());
    }

    // ── Search history CRUD ─────────────────────────────────────────────────

    #[tokio::test]
    async fn search_history_crud_lifecycle() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // List initial history (should be empty or only from seeded search)
        let list1_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/search/history")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list1_resp.status(), StatusCode::OK);
        let list1 = json_body(list1_resp).await;
        let initial_count = list1.as_array().unwrap().len();

        // Create a history item
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/search/history")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(
                        json!({"query_text": "test laptop search", "filters_json": {"sort": "price"}}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::OK);
        let create_body = json_body(create_resp).await;
        assert_eq!(create_body["status"].as_str().unwrap(), "stored");

        // List again — should have one more item
        let list2_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/search/history")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list2_resp.status(), StatusCode::OK);
        let list2 = json_body(list2_resp).await;
        assert!(list2.as_array().unwrap().len() > initial_count);
        assert!(list2.as_array().unwrap().iter().any(|item| {
            item["query_text"].as_str() == Some("test laptop search")
        }));

        // Clear history
        let clear_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri("/api/v1/search/history")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(clear_resp.status(), StatusCode::OK);
        let clear_body = json_body(clear_resp).await;
        assert_eq!(clear_body["status"].as_str().unwrap(), "cleared");

        // List again — should be empty
        let list3_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/search/history")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list3_resp.status(), StatusCode::OK);
        let list3 = json_body(list3_resp).await;
        assert_eq!(list3.as_array().unwrap().len(), 0);
    }

    // ── Recommendation settings read path ───────────────────────────────────

    #[tokio::test]
    async fn recommendation_settings_read_and_persist() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // Read current settings
        let read1_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/settings/recommendations")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(read1_resp.status(), StatusCode::OK);
        let read1 = json_body(read1_resp).await;
        assert!(read1["recommendations_enabled"].is_boolean());

        // Update to disabled
        let update_resp = app
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
        assert_eq!(update_resp.status(), StatusCode::OK);

        // Read again — should reflect update
        let read2_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/settings/recommendations")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(read2_resp.status(), StatusCode::OK);
        let read2 = json_body(read2_resp).await;
        assert_eq!(read2["recommendations_enabled"].as_bool().unwrap(), false);
    }

    // ── Phase 2: Inventory document listing ─────────────────────────────────

    #[tokio::test]
    async fn inventory_document_listing_shows_created_documents() {
        let app = setup_app().await;
        let clerk_cookie = login_cookie(&app, "clerk", "DepotCycleDemo123!").await;

        // Get a device ID
        let devices_resp = app
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
        let devices = json_body(devices_resp).await;
        let device_id = devices[0]["id"].as_str().unwrap();

        // Create a document
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/inventory/documents")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::from(
                        json!({
                            "doc_type": "receiving",
                            "reference_no": "RCV-LIST-TEST",
                            "source_campus_id": null,
                            "target_campus_id": null,
                            "notes": "listing test",
                            "lines": [{
                                "device_id": device_id,
                                "quantity": 1,
                                "unit_value_cents": 1000,
                                "target_campus_id": null,
                                "notes": null
                            }]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::OK);
        let create_json = json_body(create_resp).await;
        let doc_id = create_json["document_id"].as_str().unwrap();

        // List documents
        let list_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/inventory/documents")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let list_json = json_body(list_resp).await;
        let docs = list_json.as_array().unwrap();
        assert!(docs.iter().any(|d| d["id"].as_str() == Some(doc_id)));
        // Verify shape
        let found = docs.iter().find(|d| d["id"].as_str() == Some(doc_id)).unwrap();
        assert!(found["doc_type"].as_str().is_some());
        assert!(found["reference_no"].as_str().is_some());
        assert!(found["workflow_status"].as_str().is_some());
    }

    // ── Shipment read / history ─────────────────────────────────────────────

    #[tokio::test]
    async fn shipment_list_and_history_after_transitions() {
        let app = setup_app().await;
        let clerk_cookie = login_cookie(&app, "clerk", "DepotCycleDemo123!").await;

        // Create shipment
        let create_resp = app
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
                            "carrier_name": "TestCarrier",
                            "tracking_number": "TRACK-HIST-1"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::OK);
        let shipment = json_body(create_resp).await;
        let shipment_id = shipment["id"].as_str().unwrap();

        // Transition: created -> packed
        let transition_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/shipments/{shipment_id}/transition"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::from(json!({"next_status": "packed"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(transition_resp.status(), StatusCode::OK);

        // List shipments
        let list_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/shipments")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let list_json = json_body(list_resp).await;
        let shipments = list_json.as_array().unwrap();
        assert!(shipments.iter().any(|s| s["id"].as_str() == Some(shipment_id)));

        // Fetch history
        let history_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/shipments/{shipment_id}/history"))
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(history_resp.status(), StatusCode::OK);
        let history = json_body(history_resp).await;
        let entries = history.as_array().unwrap();
        assert!(entries.len() >= 2, "expected at least creation + transition entries");
        // First entry should be creation (null -> created)
        assert_eq!(entries[0]["to_status"].as_str().unwrap(), "created");
        // Second entry should be transition (created -> packed)
        assert_eq!(entries[1]["from_status"].as_str().unwrap(), "created");
        assert_eq!(entries[1]["to_status"].as_str().unwrap(), "packed");
    }

    // ── After-sales read / history ──────────────────────────────────────────

    #[tokio::test]
    async fn after_sales_case_list_and_history() {
        let app = setup_app().await;
        let support_cookie = login_cookie(&app, "support", "DepotCycleDemo123!").await;

        // Create case
        let case_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/after-sales/cases")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &support_cookie)
                    .body(Body::from(
                        json!({"case_type": "return", "reason": "History test"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(case_resp.status(), StatusCode::OK);
        let case_json = json_body(case_resp).await;
        let case_id = case_json["id"].as_str().unwrap();

        // Transition: requested -> under_review
        let trans_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/after-sales/cases/{case_id}/transition"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &support_cookie)
                    .body(Body::from(json!({"next_status": "under_review"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(trans_resp.status(), StatusCode::OK);

        // List cases
        let list_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/after-sales/cases")
                    .header(header::COOKIE, &support_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let list_json = json_body(list_resp).await;
        assert!(list_json.as_array().unwrap().iter().any(|c| c["id"].as_str() == Some(case_id)));

        // Fetch history
        let history_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/after-sales/cases/{case_id}/history"))
                    .header(header::COOKIE, &support_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(history_resp.status(), StatusCode::OK);
        let history = json_body(history_resp).await;
        let entries = history.as_array().unwrap();
        assert!(entries.len() >= 2);
        assert_eq!(entries[0]["to_status"].as_str().unwrap(), "requested");
        assert_eq!(entries[1]["to_status"].as_str().unwrap(), "under_review");
    }

    // ── Taxonomy: create node + list keywords ───────────────────────────────

    #[tokio::test]
    async fn taxonomy_node_creation_and_keyword_listing() {
        let app = setup_app().await;
        let manager_cookie = login_cookie(&app, "manager", "DepotCycleDemo123!").await;

        // POST /api/v1/taxonomy — create a new node
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/taxonomy")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::from(
                        json!({
                            "parent_id": null,
                            "name": "Tablets",
                            "slug": "tablets",
                            "level": 1,
                            "seo_title": "Tablet Devices",
                            "seo_description": "Tablet category",
                            "seo_keywords": "tablet,ipad",
                            "topic_page_path": "/topics/tablets"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::OK);
        let node = json_body(create_resp).await;
        assert_eq!(node["name"].as_str().unwrap(), "Tablets");
        assert_eq!(node["slug"].as_str().unwrap(), "tablets");

        // Create a keyword and verify it shows in the list
        let kw_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/taxonomy/keywords")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::from(json!({"keyword": "android-tablet"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(kw_resp.status(), StatusCode::OK);

        // GET /api/v1/taxonomy/keywords
        let list_kw_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/taxonomy/keywords")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_kw_resp.status(), StatusCode::OK);
        let keywords = json_body(list_kw_resp).await;
        assert!(keywords.as_array().unwrap().iter().any(|k| {
            k["keyword"].as_str() == Some("android-tablet")
        }));
    }

    // ── Phase 3: Admin cohort read tests ────────────────────────────────────

    #[tokio::test]
    async fn admin_cohort_and_assignment_listing() {
        let app = setup_app().await;
        let admin_cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // Get shopper user ID
        let me_resp = app
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
        let me_json = json_body(me_resp).await;
        let shopper_id = me_json["user_id"].as_str().unwrap();

        // Create cohort
        let cohort_name = format!("test-cohort-{}", Uuid::new_v4().simple());
        let cohort_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/cohorts")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({"name": cohort_name, "description": "test"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cohort_resp.status(), StatusCode::OK);
        let cohort_json = json_body(cohort_resp).await;
        let cohort_id = cohort_json["id"].as_str().unwrap();

        // Assign user to cohort
        let assign_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/cohort-assignments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({"cohort_id": cohort_id, "user_id": shopper_id}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(assign_resp.status(), StatusCode::OK);

        // GET /api/v1/admin/cohorts
        let list_cohorts_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/cohorts")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_cohorts_resp.status(), StatusCode::OK);
        let cohorts = json_body(list_cohorts_resp).await;
        assert!(cohorts.as_array().unwrap().iter().any(|c| c["id"].as_str() == Some(cohort_id)));

        // GET /api/v1/admin/cohort-assignments
        let list_assigns_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/cohort-assignments")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_assigns_resp.status(), StatusCode::OK);
        let assigns = json_body(list_assigns_resp).await;
        let assignment = assigns
            .as_array()
            .unwrap()
            .iter()
            .find(|a| {
                a["cohort_id"].as_str() == Some(cohort_id)
                    && a["user_id"].as_str() == Some(shopper_id)
            })
            .expect("cohort assignments should include the assignment created in this test");
        assert!(assignment["id"].as_str().is_some(), "assignment should expose an id");
        assert_eq!(assignment["cohort_id"].as_str().unwrap(), cohort_id);
        assert_eq!(assignment["user_id"].as_str().unwrap(), shopper_id);
        assert!(
            assignment["created_at"].as_str().is_some(),
            "assignment should expose created_at timestamp"
        );
    }

    // ── Ratings review + dashboard metrics ──────────────────────────────────

    #[tokio::test]
    async fn admin_ratings_review_and_dashboard_metrics() {
        let app = setup_app().await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;
        let admin_cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;

        // Shopper creates a rating on a seeded listing
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
        let listings = json_body(listings_resp).await;
        let listing_id = listings[0]["id"].as_str().unwrap();

        let rating_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/ratings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &shopper_cookie)
                    .body(Body::from(
                        json!({"listing_id": listing_id, "score": 5, "comments": "Admin review test"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rating_resp.status(), StatusCode::OK);

        // GET /api/v1/admin/ratings-review
        let review_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/ratings-review")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(review_resp.status(), StatusCode::OK);
        let reviews = json_body(review_resp).await;
        let arr = reviews.as_array().unwrap();
        assert!(!arr.is_empty());
        let created_review = arr
            .iter()
            .find(|review| review["comments"].as_str() == Some("Admin review test"))
            .expect("ratings review should include the rating created in this test");
        assert!(created_review["rating_id"].as_str().is_some());
        assert_eq!(created_review["score"].as_i64().unwrap(), 5);
        assert_eq!(created_review["comments"].as_str().unwrap(), "Admin review test");
        assert!(
            created_review.get("review_status").is_some(),
            "ratings review payload should expose review_status field"
        );

        // GET /api/v1/admin/dashboard/metrics
        let metrics_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/dashboard/metrics")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(metrics_resp.status(), StatusCode::OK);
        let metrics = json_body(metrics_resp).await;
        assert!(metrics["total_users"].is_number());
        assert!(metrics["total_announcements"].is_number());
        assert!(metrics["total_templates"].is_number());
        assert!(metrics["total_feature_flags"].is_number());
        assert!(metrics["active_users_last_30_days"].is_number());
        assert!(metrics["conversion_rate_percent"].is_number());
        assert!(metrics["average_rating"].is_number());
        assert!(metrics["open_support_cases"].is_number());
    }

    // ── Credentials CRUD-lite ───────────────────────────────────────────────

    #[tokio::test]
    async fn admin_credentials_crud() {
        let app = setup_app().await;
        let admin_cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;

        // POST /api/v1/admin/local-credentials
        let create_local_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/local-credentials")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({
                            "label": "Test DB Cred",
                            "username": "db_user",
                            "secret": "s3cret_password",
                            "notes": "integration test"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_local_resp.status(), StatusCode::OK);
        let local_cred = json_body(create_local_resp).await;
        assert_eq!(local_cred["label"].as_str().unwrap(), "Test DB Cred");
        assert_eq!(local_cred["username"].as_str().unwrap(), "db_user");
        let local_cred_id = local_cred["id"].as_str().unwrap();

        // GET /api/v1/admin/local-credentials
        let list_local_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/local-credentials")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_local_resp.status(), StatusCode::OK);
        let local_creds = json_body(list_local_resp).await;
        assert!(local_creds.as_array().unwrap().iter().any(|c| c["id"].as_str() == Some(local_cred_id)));

        // POST /api/v1/admin/companion-credentials
        let create_companion_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/companion-credentials")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({
                            "label": "Companion API",
                            "provider": "ExternalSvc",
                            "endpoint": "https://api.example.com",
                            "username": "api_user",
                            "secret": "api_s3cret",
                            "notes": "integration test"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_companion_resp.status(), StatusCode::OK);
        let companion_cred = json_body(create_companion_resp).await;
        assert_eq!(companion_cred["provider"].as_str().unwrap(), "ExternalSvc");
        let companion_cred_id = companion_cred["id"].as_str().unwrap();

        // GET /api/v1/admin/companion-credentials
        let list_companion_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/companion-credentials")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_companion_resp.status(), StatusCode::OK);
        let companion_creds = json_body(list_companion_resp).await;
        assert!(companion_creds.as_array().unwrap().iter().any(|c| c["id"].as_str() == Some(companion_cred_id)));
    }

    // ── Non-admin is denied credential access ───────────────────────────────

    #[tokio::test]
    async fn non_admin_cannot_access_credentials() {
        let app = setup_app().await;
        let manager_cookie = login_cookie(&app, "manager", "DepotCycleDemo123!").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/local-credentials")
                    .header(header::COOKIE, &manager_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    // ── Templates CRUD ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn admin_templates_crud_and_versioning() {
        let app = setup_app().await;
        let admin_cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;

        // Create template
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/templates")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({
                            "kind": "email",
                            "key": "welcome_email",
                            "title": "Welcome Email v1",
                            "content": "Hello {{name}}!",
                            "is_active": true
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::OK);
        let template = json_body(create_resp).await;
        let template_id = template["id"].as_str().unwrap();
        assert_eq!(template["kind"].as_str().unwrap(), "email");
        assert_eq!(template["version"].as_i64().unwrap(), 1);

        // List templates
        let list_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/templates")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let templates = json_body(list_resp).await;
        assert!(templates.as_array().unwrap().iter().any(|t| t["id"].as_str() == Some(template_id)));

        // Update template
        let update_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(&format!("/api/v1/admin/templates/{template_id}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({
                            "title": "Welcome Email v2",
                            "content": "Hi {{name}}, welcome!",
                            "is_active": true
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update_resp.status(), StatusCode::OK);
        let updated = json_body(update_resp).await;
        assert_eq!(updated["title"].as_str().unwrap(), "Welcome Email v2");
        assert_eq!(updated["version"].as_i64().unwrap(), 2);

        // Update non-existent template returns 404
        let bad_update_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/v1/admin/templates/nonexistent-id")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({"title": "x", "content": "y"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad_update_resp.status(), StatusCode::NOT_FOUND);
    }

    // ── Announcements admin read ────────────────────────────────────────────

    #[tokio::test]
    async fn admin_announcement_listing_and_deliveries() {
        let app = setup_app().await;
        let admin_cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;
        let shopper_cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // Get shopper user ID
        let me_resp = app
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
        let shopper_id = json_body(me_resp).await["user_id"].as_str().unwrap().to_string();

        // Create announcement
        let ann_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/announcements")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({"title": "List Test", "body": "body", "severity": "warning"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ann_resp.status(), StatusCode::OK);
        let ann = json_body(ann_resp).await;
        let ann_id = ann["id"].as_str().unwrap();

        // Create delivery to specific user
        let delivery_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/admin/announcements/{ann_id}/deliveries"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::from(
                        json!({"user_ids": [shopper_id]}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(delivery_resp.status(), StatusCode::OK);
        let delivery_body = json_body(delivery_resp).await;
        assert_eq!(delivery_body["delivered_count"].as_i64().unwrap(), 1);

        // GET /api/v1/admin/announcements
        let list_ann_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/admin/announcements")
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_ann_resp.status(), StatusCode::OK);
        let anns = json_body(list_ann_resp).await;
        assert!(anns.as_array().unwrap().iter().any(|a| a["id"].as_str() == Some(ann_id)));

        // GET /api/v1/admin/announcements/:id/deliveries
        let list_del_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/admin/announcements/{ann_id}/deliveries"))
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_del_resp.status(), StatusCode::OK);
        let deliveries = json_body(list_del_resp).await;
        let del_arr = deliveries.as_array().unwrap();
        assert!(!del_arr.is_empty(), "deliveries array must contain the delivery we just created");

        // Deep field-level assertions on every delivery record
        let shopper_delivery = del_arr
            .iter()
            .find(|d| d["user_id"].as_str() == Some(&shopper_id))
            .expect("should contain a delivery for the target shopper");
        assert_eq!(
            shopper_delivery["announcement_id"].as_str().unwrap(),
            ann_id,
            "delivery must reference the correct announcement"
        );
        assert!(
            shopper_delivery["delivered_at"].as_str().is_some(),
            "delivered_at timestamp must be present"
        );
        assert!(!shopper_delivery["delivered_at"].as_str().unwrap().is_empty());
        // read_at should be null since the shopper hasn't read it yet
        assert!(
            shopper_delivery["read_at"].is_null(),
            "read_at should be null before the user reads the announcement"
        );

        // Verify all records have the required shape
        for record in del_arr {
            assert!(record["announcement_id"].as_str().is_some(), "every delivery must have announcement_id");
            assert!(record["user_id"].as_str().is_some(), "every delivery must have user_id");
            assert!(record["delivered_at"].as_str().is_some(), "every delivery must have delivered_at");
            // read_at is nullable — just verify the key exists
            assert!(record.get("read_at").is_some(), "every delivery must have read_at key");
        }
    }

    // ── Handler negative-path and edge-case tests ───────────────────────────

    #[tokio::test]
    async fn malformed_json_body_returns_422_or_400() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // Completely invalid JSON
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/orders")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from("not json at all"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::UNPROCESSABLE_ENTITY,
            "malformed JSON should be rejected, got {}",
            resp.status()
        );
    }

    #[tokio::test]
    async fn missing_required_fields_returns_error() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        // Order without listing_id
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/orders")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(json!({"quantity": 1}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            resp.status().is_client_error(),
            "missing listing_id should be rejected, got {}",
            resp.status()
        );

        // Rating without listing_id
        let resp2 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/ratings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(json!({"score": 3}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(resp2.status().is_client_error());

        // Shipment transition without next_status
        let clerk_cookie = login_cookie(&app, "clerk", "DepotCycleDemo123!").await;
        let ship_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/shipments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::from(json!({"carrier_name": "Test"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let ship = json_body(ship_resp).await;
        let ship_id = ship["id"].as_str().unwrap();

        let trans_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&format!("/api/v1/shipments/{ship_id}/transition"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &clerk_cookie)
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(trans_resp.status().is_client_error());
    }

    #[tokio::test]
    async fn empty_body_on_post_returns_error() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(resp.status().is_client_error(), "empty login body should fail");

        let resp2 = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/orders")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(resp2.status().is_client_error(), "empty order body should fail");
    }

    #[tokio::test]
    async fn zero_quantity_order_rejected() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        let listings_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/listings/search?sort=relevance")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let listings = json_body(listings_resp).await;
        let listing_id = listings[0]["id"].as_str().unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/orders")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(
                        json!({"listing_id": listing_id, "quantity": 0}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = json_body(resp).await;
        assert!(body["error"].as_str().unwrap().contains("quantity"));
    }

    #[tokio::test]
    async fn nonexistent_listing_detail_returns_404() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "shopper", "DepotCycleDemo123!").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/listings/nonexistent-id-12345")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = json_body(resp).await;
        assert!(body["error"].is_string());
    }

    #[tokio::test]
    async fn nonexistent_shipment_transition_returns_404() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "clerk", "DepotCycleDemo123!").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/shipments/fake-ship-id/transition")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(json!({"next_status": "packed"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn nonexistent_case_transition_returns_404() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "support", "DepotCycleDemo123!").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/after-sales/cases/fake-case-id/transition")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(json!({"next_status": "under_review"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn inventory_document_with_nonexistent_device_returns_404() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "clerk", "DepotCycleDemo123!").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/inventory/documents")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(
                        json!({
                            "doc_type": "receiving",
                            "reference_no": "BAD-DEV-001",
                            "source_campus_id": null,
                            "target_campus_id": null,
                            "notes": null,
                            "lines": [{"device_id": "nonexistent-device", "quantity": 1, "unit_value_cents": 100, "target_campus_id": null, "notes": null}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn inventory_document_with_empty_lines_returns_400() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "clerk", "DepotCycleDemo123!").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/inventory/documents")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(
                        json!({
                            "doc_type": "receiving",
                            "reference_no": "EMPTY-001",
                            "source_campus_id": null,
                            "target_campus_id": null,
                            "notes": null,
                            "lines": []
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn update_nonexistent_template_returns_404() {
        let app = setup_app().await;
        let cookie = login_cookie(&app, "admin", "DepotCycleAdmin123!").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/v1/admin/templates/fake-template-id")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, &cookie)
                    .body(Body::from(
                        json!({"title": "x", "content": "y"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn password_too_short_on_register_returns_400() {
        let app = setup_app().await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/register")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "username": format!("short_pw_{}", Uuid::new_v4().simple()),
                            "password": "short",
                            "role_name": "Shopper"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = json_body(resp).await;
        assert!(body["error"].as_str().unwrap().contains("password"));
    }

    #[tokio::test]
    async fn wrong_credentials_login_returns_401_with_error_body() {
        let app = setup_app().await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"username": "admin", "password": "WrongPassword!!"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = json_body(resp).await;
        assert!(body["error"].is_string());
        assert!(body["error"].as_str().unwrap().contains("credentials"));
    }

    #[tokio::test]
    async fn login_with_nonexistent_user_returns_401() {
        let app = setup_app().await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"username": "no_such_user_xyz", "password": "AnyPassword123!"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
