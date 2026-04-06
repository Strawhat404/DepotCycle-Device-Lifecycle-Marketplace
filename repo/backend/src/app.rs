use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post, put},
    Router,
};
use sqlx::SqlitePool;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{auth, config::AppConfig, handlers};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<AppConfig>,
}

pub fn build_router(pool: SqlitePool, config: AppConfig) -> Router {
    let max_upload = config.max_upload_size_bytes;
    let allowed_origin = config.allowed_origin.clone();
    let state = AppState {
        pool,
        config: Arc::new(config),
    };

    let cors = CorsLayer::new()
        .allow_origin(
            allowed_origin
                .parse::<axum::http::HeaderValue>()
                .unwrap_or_else(|_| "http://localhost".parse().unwrap()),
        )
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::COOKIE,
        ])
        .allow_credentials(true);

    Router::new()
        .route("/api/v1/health", get(handlers::health))
        .route("/api/v1/workspaces", get(handlers::workspaces))
        .route("/api/v1/campuses", get(handlers::list_campuses))
        .route("/api/v1/inventory/devices", get(handlers::list_devices))
        .route("/api/v1/auth/register", post(handlers::register))
        .route("/api/v1/auth/login", post(handlers::login))
        .route("/api/v1/auth/logout", post(handlers::logout))
        .route("/api/v1/auth/me", get(handlers::me))
        .route("/api/v1/listings", post(handlers::create_listing))
        .route("/api/v1/listings/search", get(handlers::search_listings))
        .route("/api/v1/listings/:id", get(handlers::get_listing_detail))
        .route("/api/v1/listings/:id/view", post(handlers::record_listing_view))
        .route("/api/v1/favorites/:id", post(handlers::toggle_favorite))
        .route("/api/v1/search/suggestions", get(handlers::search_suggestions))
        .route("/api/v1/search/history", get(handlers::list_search_history).post(handlers::create_search_history).delete(handlers::clear_search_history))
        .route("/api/v1/recommendations", get(handlers::recommendations))
        .route("/api/v1/settings/recommendations", get(handlers::get_recommendation_settings).post(handlers::update_recommendation_settings))
        .route("/api/v1/orders", get(handlers::list_orders).post(handlers::create_order))
        .route("/api/v1/ratings", post(handlers::create_rating))
        .route("/api/v1/appeal-tickets", post(handlers::create_appeal_ticket))
        .route("/api/v1/taxonomy", get(handlers::list_taxonomy).post(handlers::create_taxonomy))
        .route("/api/v1/taxonomy/tags", get(handlers::list_taxonomy_tags).post(handlers::create_taxonomy_tag))
        .route("/api/v1/taxonomy/keywords", get(handlers::list_taxonomy_keywords).post(handlers::create_taxonomy_keyword))
        .route("/api/v1/taxonomy/:node_id/tags", post(handlers::associate_taxonomy_tag))
        .route("/api/v1/taxonomy/:node_id/keywords", post(handlers::associate_taxonomy_keyword))
        .route("/api/v1/media/uploads/start", post(handlers::create_upload_session))
        .route("/api/v1/media/uploads/:session_id/chunks/:chunk_index", put(handlers::upload_chunk).layer(DefaultBodyLimit::max(max_upload)))
        .route("/api/v1/media/uploads/:session_id/finalize", post(handlers::finalize_upload))
        .route("/api/v1/media/playback/:media_id", get(handlers::playback_link))
        .route("/api/v1/media/stream/:token", get(handlers::stream_media))
        .route("/api/v1/inventory/documents", get(handlers::list_inventory_documents).post(handlers::create_inventory_document))
        .route("/api/v1/inventory/documents/:document_id/approve", post(handlers::approve_inventory_document))
        .route("/api/v1/inventory/documents/:document_id/execute", post(handlers::execute_inventory_document_endpoint))
        .route("/api/v1/shipments", get(handlers::list_shipments).post(handlers::create_shipment))
        .route("/api/v1/shipments/:shipment_id/transition", post(handlers::transition_shipment))
        .route("/api/v1/shipments/:shipment_id/history", get(handlers::shipment_history))
        .route("/api/v1/after-sales/cases", get(handlers::list_after_sales_cases).post(handlers::create_after_sales_case))
        .route("/api/v1/after-sales/cases/:case_id/transition", post(handlers::transition_after_sales_case))
        .route("/api/v1/after-sales/cases/:case_id/evidence", post(handlers::attach_after_sales_evidence))
        .route("/api/v1/after-sales/cases/:case_id/evidence/upload", post(handlers::upload_after_sales_evidence).layer(DefaultBodyLimit::max(max_upload)))
        .route("/api/v1/after-sales/cases/:case_id/history", get(handlers::after_sales_history))
        .route("/api/v1/announcements/inbox", get(handlers::list_my_announcement_deliveries))
        .route("/api/v1/announcements/:announcement_id/read", post(handlers::mark_announcement_read))
        .route("/api/v1/admin/feature-flags", get(handlers::list_feature_flags))
        .route("/api/v1/admin/feature-flags/:flag_id", put(handlers::update_feature_flag))
        .route("/api/v1/admin/cohorts", get(handlers::list_cohorts).post(handlers::create_cohort))
        .route("/api/v1/admin/cohort-assignments", get(handlers::list_cohort_assignments).post(handlers::assign_cohort))
        .route("/api/v1/admin/ratings-review", get(handlers::list_ratings_review))
        .route("/api/v1/admin/appeals", get(handlers::list_appeals))
        .route("/api/v1/admin/local-credentials", get(handlers::list_local_credentials).post(handlers::create_local_credential))
        .route("/api/v1/admin/companion-credentials", get(handlers::list_companion_credentials).post(handlers::create_companion_credential))
        .route("/api/v1/admin/templates", get(handlers::list_templates).post(handlers::create_template))
        .route("/api/v1/admin/templates/:id", put(handlers::update_template))
        .route("/api/v1/admin/announcements", get(handlers::list_announcements).post(handlers::create_announcement))
        .route("/api/v1/admin/announcements/:announcement_id/deliveries", get(handlers::list_announcement_deliveries).post(handlers::create_announcement_deliveries))
        .route("/api/v1/admin/dashboard/metrics", get(handlers::dashboard_metrics))
        .route("/api/v1/admin/media/upload", post(handlers::upload_media).layer(DefaultBodyLimit::max(max_upload)))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::session_middleware,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
