use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct CurrentUser {
    pub id: String,
    pub username: String,
    pub role_name: String,
    pub session_id: Option<String>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub mode: &'static str,
    pub timestamp_utc: String,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub role_name: String,
    pub display_name: Option<String>,
    pub phone: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user_id: String,
    pub username: String,
    pub role_name: String,
    pub display_name_masked: Option<String>,
    pub phone_masked: Option<String>,
}

#[derive(Serialize)]
pub struct WorkspaceSummary {
    pub role_name: String,
    pub capabilities: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct ReferenceItem {
    pub id: String,
    pub label: String,
}

#[derive(Deserialize, Serialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub category: Option<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
    pub condition: Option<String>,
    pub campus: Option<String>,
    pub post_time_days: Option<i64>,
    pub sort: Option<String>,
    pub zip_code: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ListingCard {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub price_cents: i64,
    pub status: String,
    pub created_at: String,
    pub campus_name: Option<String>,
    pub campus_zip_code: Option<String>,
    pub condition_code: Option<String>,
    pub category_slug: Option<String>,
}

#[derive(Serialize)]
pub struct ListingDetail {
    pub listing: ListingCard,
    pub popularity_score: i64,
    pub inventory_on_hand: i64,
    pub recommendations: Vec<RecommendationCard>,
}

#[derive(Serialize)]
pub struct SuggestionResponse {
    pub suggestions: Vec<String>,
}

#[derive(Serialize)]
pub struct SearchHistoryItem {
    pub id: String,
    pub query_text: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct SearchHistoryRequest {
    pub query_text: String,
    pub filters_json: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct RecommendationCard {
    pub listing_id: String,
    pub title: String,
    pub reason: String,
    pub price_cents: i64,
}

#[derive(Serialize)]
pub struct RecommendationSettingsResponse {
    pub recommendations_enabled: bool,
}

#[derive(Deserialize)]
pub struct RecommendationSettingsRequest {
    pub recommendations_enabled: bool,
}

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub listing_id: String,
    pub quantity: i64,
}

#[derive(Serialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub status: String,
    pub total_cents: i64,
}

#[derive(Deserialize)]
pub struct CreateCredentialRequest {
    pub label: String,
    pub username: String,
    pub secret: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateCompanionCredentialRequest {
    pub label: String,
    pub provider: String,
    pub endpoint: Option<String>,
    pub username: String,
    pub secret: String,
    pub notes: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CredentialRecord {
    pub id: String,
    pub label: String,
    pub username: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CompanionCredentialRecord {
    pub id: String,
    pub label: String,
    pub provider: String,
    pub endpoint: Option<String>,
    pub username: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct CreateTemplateRequest {
    pub kind: String,
    pub key: String,
    pub title: String,
    pub content: String,
    pub is_active: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateTemplateRequest {
    pub title: String,
    pub content: String,
    pub is_active: Option<bool>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TemplateRecord {
    pub id: String,
    pub kind: String,
    pub key: String,
    pub title: String,
    pub content: String,
    pub version: i64,
    pub is_active: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct CreateAnnouncementRequest {
    pub title: String,
    pub body: String,
    pub severity: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AnnouncementRecord {
    pub id: String,
    pub title: String,
    pub body: String,
    pub severity: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateCohortRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CohortRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct AssignCohortRequest {
    pub cohort_id: String,
    pub user_id: String,
}

#[derive(Deserialize)]
pub struct ListCohortAssignmentsQuery {
    pub cohort_id: Option<String>,
    pub user_id: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CohortAssignmentRecord {
    pub id: String,
    pub cohort_id: String,
    pub user_id: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateAnnouncementDeliveriesRequest {
    pub user_ids: Option<Vec<String>>,
    pub cohort_id: Option<String>,
}

#[derive(Serialize)]
pub struct AnnouncementDeliveryBatchResponse {
    pub announcement_id: String,
    pub delivered_count: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AnnouncementDeliveryRecord {
    pub announcement_id: String,
    pub user_id: String,
    pub delivered_at: String,
    pub read_at: Option<String>,
}

#[derive(Serialize)]
pub struct DashboardMetricsResponse {
    pub total_users: i64,
    pub total_announcements: i64,
    pub total_templates: i64,
    pub total_local_credentials: i64,
    pub total_companion_credentials: i64,
    pub total_uploads: i64,
    pub total_shipments: i64,
    pub total_feature_flags: i64,
    pub total_events: i64,
    pub active_users_last_30_days: i64,
    pub conversion_rate_percent: f64,
    pub average_rating: f64,
    pub open_support_cases: i64,
}

#[derive(Serialize)]
pub struct UploadResponse {
    pub media_id: String,
    pub mime_type: String,
    pub sha256: String,
    pub storage_path: String,
    pub playback_token: String,
}

#[derive(Deserialize)]
pub struct CreateUploadSessionRequest {
    pub file_name: String,
    pub mime_type: String,
    pub total_chunks: i64,
    pub listing_id: Option<String>,
    pub expected_sha256: Option<String>,
}

#[derive(Serialize)]
pub struct UploadSessionResponse {
    pub session_id: String,
    pub uploaded_chunks: i64,
    pub total_chunks: i64,
    pub status: String,
}

#[derive(Deserialize)]
pub struct FinalizeUploadRequest {
    pub expected_sha256: Option<String>,
}

#[derive(Serialize)]
pub struct PlaybackLinkResponse {
    pub token: String,
    pub stream_url: String,
    pub expires_at: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TaxonomyNodeRecord {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub slug: String,
    pub level: i64,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub seo_keywords: Option<String>,
    pub topic_page_path: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateTaxonomyNodeRequest {
    pub parent_id: Option<String>,
    pub name: String,
    pub slug: String,
    pub level: i64,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub seo_keywords: Option<String>,
    pub topic_page_path: Option<String>,
}

#[derive(Deserialize)]
pub struct InventoryLineInput {
    pub device_id: String,
    pub quantity: i64,
    pub unit_value_cents: i64,
    pub target_campus_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateInventoryDocumentRequest {
    pub doc_type: String,
    pub reference_no: String,
    pub source_campus_id: Option<String>,
    pub target_campus_id: Option<String>,
    pub notes: Option<String>,
    pub lines: Vec<InventoryLineInput>,
}

#[derive(Serialize)]
pub struct InventoryDocumentResponse {
    pub document_id: String,
    pub status: String,
    pub requires_manager_approval: bool,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct InventoryDocumentRecord {
    pub id: String,
    pub doc_type: String,
    pub reference_no: String,
    pub workflow_status: String,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateShipmentRequest {
    pub listing_id: Option<String>,
    pub device_id: Option<String>,
    pub from_campus_id: Option<String>,
    pub to_campus_id: Option<String>,
    pub carrier_name: Option<String>,
    pub tracking_number: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ShipmentRecord {
    pub id: String,
    pub order_number: String,
    pub status: String,
    pub carrier_name: Option<String>,
    pub tracking_number: Option<String>,
    pub integration_enabled: i64,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct TransitionRequest {
    pub next_status: String,
}

#[derive(Deserialize)]
pub struct CreateAfterSalesCaseRequest {
    pub order_id: Option<String>,
    pub case_type: String,
    pub reason: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AfterSalesCaseRecord {
    pub id: String,
    pub case_type: String,
    pub status: String,
    pub reason: String,
    pub first_response_due_at: String,
    pub final_decision_due_at: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct AttachEvidenceRequest {
    pub media_id: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct FeatureFlagRecord {
    pub id: String,
    pub key: String,
    pub description: Option<String>,
    pub enabled: i64,
    pub rollout_percent: i64,
    pub audience_rules_json: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateFeatureFlagRequest {
    pub enabled: bool,
    pub rollout_percent: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RatingReviewRecord {
    pub rating_id: String,
    pub score: i64,
    pub comments: Option<String>,
    pub review_status: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateListingRequest {
    pub title: String,
    pub description: Option<String>,
    pub price_cents: i64,
    pub campus_id: Option<String>,
    pub taxonomy_node_id: Option<String>,
    pub condition_id: Option<i64>,
    pub currency: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateRatingRequest {
    pub listing_id: String,
    pub score: i64,
    pub comments: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RatingRecord {
    pub id: String,
    pub listing_id: Option<String>,
    pub user_id: Option<String>,
    pub score: i64,
    pub comments: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateAppealTicketRequest {
    pub listing_id: Option<String>,
    pub shipment_order_id: Option<String>,
    pub reason: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AppealTicketRecord {
    pub id: String,
    pub ticket_no: String,
    pub status: String,
    pub reason: String,
    pub resolution: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateTaxonomyTagRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TaxonomyTagRecord {
    pub id: String,
    pub name: String,
    pub slug: String,
}

#[derive(Deserialize)]
pub struct CreateTaxonomyKeywordRequest {
    pub keyword: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TaxonomyKeywordRecord {
    pub id: String,
    pub keyword: String,
}

#[derive(Deserialize)]
pub struct AssociateTaxonomyTagRequest {
    pub tag_id: String,
}

#[derive(Deserialize)]
pub struct AssociateTaxonomyKeywordRequest {
    pub keyword_id: String,
}
