use gloo_net::http::Request;
use leptos::*;
use serde::{de::DeserializeOwned, Deserialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::{Blob, File, FormData, RequestCredentials};

const API: &str = "/api/v1";

#[derive(Clone, Debug, Deserialize)]
struct AuthUser {
    user_id: String,
    username: String,
    role_name: String,
    display_name_masked: Option<String>,
    phone_masked: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct ListingCard {
    id: String,
    title: String,
    description: Option<String>,
    price_cents: i64,
    status: String,
    created_at: String,
    campus_name: Option<String>,
    campus_zip_code: Option<String>,
    condition_code: Option<String>,
    category_slug: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct RecommendationCard {
    listing_id: String,
    title: String,
    reason: String,
    price_cents: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct ListingDetail {
    listing: ListingCard,
    popularity_score: i64,
    inventory_on_hand: i64,
    recommendations: Vec<RecommendationCard>,
}

#[derive(Clone, Debug, Deserialize)]
struct SuggestionResponse {
    suggestions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct SearchHistoryItem {
    id: String,
    query_text: String,
    created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RecommendationSettings {
    recommendations_enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct ReferenceItem {
    id: String,
    label: String,
}

#[derive(Clone, Debug, Deserialize)]
struct OrderResponse {
    order_id: String,
    status: String,
    total_cents: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct InventoryDocument {
    id: String,
    doc_type: String,
    reference_no: String,
    workflow_status: String,
    notes: Option<String>,
    created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct InventoryDocResponse {
    document_id: String,
    status: String,
    requires_manager_approval: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct ShipmentRecord {
    id: String,
    order_number: String,
    status: String,
    carrier_name: Option<String>,
    tracking_number: Option<String>,
    integration_enabled: i64,
    created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AfterSalesCase {
    id: String,
    case_type: String,
    status: String,
    reason: String,
    first_response_due_at: String,
    final_decision_due_at: String,
    created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct FeatureFlag {
    id: String,
    key: String,
    description: Option<String>,
    enabled: i64,
    rollout_percent: i64,
    audience_rules_json: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct DashboardMetrics {
    total_users: i64,
    total_announcements: i64,
    total_templates: i64,
    total_local_credentials: i64,
    total_companion_credentials: i64,
    total_uploads: i64,
    total_shipments: i64,
    total_feature_flags: i64,
    total_events: i64,
    active_users_last_30_days: i64,
    conversion_rate_percent: f64,
    average_rating: f64,
    open_support_cases: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct CredentialRecord {
    id: String,
    label: String,
    username: String,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CompanionCredentialRecord {
    id: String,
    label: String,
    provider: String,
    endpoint: Option<String>,
    username: String,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TemplateRecord {
    id: String,
    kind: String,
    key: String,
    title: String,
    content: String,
    version: i64,
    is_active: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AnnouncementRecord {
    id: String,
    title: String,
    body: String,
    severity: String,
    starts_at: Option<String>,
    ends_at: Option<String>,
    created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TaxonomyNode {
    id: String,
    parent_id: Option<String>,
    name: String,
    slug: String,
    level: i64,
    seo_title: Option<String>,
    seo_description: Option<String>,
    seo_keywords: Option<String>,
    topic_page_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct RatingReview {
    rating_id: String,
    score: i64,
    comments: Option<String>,
    review_status: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TimelineEntry {
    from_status: Option<String>,
    to_status: String,
    changed_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct UploadSessionResponse {
    session_id: String,
    uploaded_chunks: i64,
    total_chunks: i64,
    status: String,
}

#[derive(Clone, Debug, Deserialize)]
struct UploadFinalizeResponse {
    media_id: String,
    mime_type: String,
    sha256: String,
    storage_path: String,
    playback_token: String,
}

#[component]
fn App() -> impl IntoView {
    let user = create_rw_signal::<Option<AuthUser>>(None);
    let error = create_rw_signal(String::new());
    let info = create_rw_signal(String::new());
    let username = create_rw_signal("admin".to_string());
    let password = create_rw_signal("DepotCycleAdmin123!".to_string());

    let search_query = create_rw_signal(String::new());
    let search_category = create_rw_signal(String::new());
    let search_condition = create_rw_signal(String::new());
    let search_campus = create_rw_signal(String::new());
    let search_sort = create_rw_signal("relevance".to_string());
    let min_price = create_rw_signal(String::new());
    let max_price = create_rw_signal(String::new());
    let zip_code = create_rw_signal(String::new());

    let listings = create_rw_signal(Vec::<ListingCard>::new());
    let selected_listing = create_rw_signal::<Option<ListingDetail>>(None);
    let suggestions = create_rw_signal(Vec::<String>::new());
    let search_history = create_rw_signal(Vec::<SearchHistoryItem>::new());
    let recommendations = create_rw_signal(Vec::<RecommendationCard>::new());
    let recommendation_settings = create_rw_signal(RecommendationSettings {
        recommendations_enabled: true,
    });
    let campuses = create_rw_signal(Vec::<ReferenceItem>::new());
    let devices = create_rw_signal(Vec::<ReferenceItem>::new());
    let orders = create_rw_signal(Vec::<OrderResponse>::new());
    let documents = create_rw_signal(Vec::<InventoryDocument>::new());
    let shipments = create_rw_signal(Vec::<ShipmentRecord>::new());
    let shipment_history = create_rw_signal(Vec::<TimelineEntry>::new());
    let cases = create_rw_signal(Vec::<AfterSalesCase>::new());
    let case_history = create_rw_signal(Vec::<TimelineEntry>::new());
    let flags = create_rw_signal(Vec::<FeatureFlag>::new());
    let metrics = create_rw_signal::<Option<DashboardMetrics>>(None);
    let local_credentials = create_rw_signal(Vec::<CredentialRecord>::new());
    let companion_credentials = create_rw_signal(Vec::<CompanionCredentialRecord>::new());
    let templates = create_rw_signal(Vec::<TemplateRecord>::new());
    let announcements = create_rw_signal(Vec::<AnnouncementRecord>::new());
    let taxonomy = create_rw_signal(Vec::<TaxonomyNode>::new());
    let ratings = create_rw_signal(Vec::<RatingReview>::new());
    let appeals = create_rw_signal(Vec::<serde_json::Value>::new());

    let doc_type = create_rw_signal("receiving".to_string());
    let doc_reference = create_rw_signal("DOC-001".to_string());
    let doc_device_id = create_rw_signal(String::new());
    let doc_target_campus = create_rw_signal(String::new());
    let doc_unit_value = create_rw_signal("10000".to_string());

    let shipment_device_id = create_rw_signal(String::new());
    let shipment_from = create_rw_signal(String::new());
    let shipment_to = create_rw_signal(String::new());
    let shipment_carrier = create_rw_signal("Manual Carrier".to_string());
    let shipment_tracking = create_rw_signal("TRACK-001".to_string());

    let case_type = create_rw_signal("return".to_string());
    let case_reason = create_rw_signal("Device condition mismatch".to_string());
    let evidence_media_id = create_rw_signal(String::new());

    let upload_progress = create_rw_signal(String::new());
    let upload_percent = create_rw_signal(0_i64);
    let upload_in_progress = create_rw_signal(false);

    let flag_rollout = create_rw_signal("50".to_string());
    let taxonomy_name = create_rw_signal("Accessories".to_string());
    let taxonomy_slug = create_rw_signal("accessories".to_string());

    let template_title = create_rw_signal("Welcome".to_string());
    let template_key = create_rw_signal("welcome-card".to_string());
    let template_content = create_rw_signal("Local configuration content".to_string());
    let announcement_title = create_rw_signal("Campus Ops".to_string());
    let announcement_body = create_rw_signal("The offline notification feed is active.".to_string());

    let local_cred_label = create_rw_signal("Printer".to_string());
    let local_cred_user = create_rw_signal("operator".to_string());
    let local_cred_secret = create_rw_signal("LocallyStoredSecret!".to_string());
    let companion_label = create_rw_signal("Warehouse Bridge".to_string());
    let companion_provider = create_rw_signal("manual-adapter".to_string());
    let companion_user = create_rw_signal("bridge".to_string());
    let companion_secret = create_rw_signal("CompanionSecret!".to_string());

    let fetch_all = {
        let campuses = campuses.clone();
        let devices = devices.clone();
        let search_history = search_history.clone();
        let recommendations = recommendations.clone();
        let recommendation_settings = recommendation_settings.clone();
        let orders = orders.clone();
        let documents = documents.clone();
        let shipments = shipments.clone();
        let cases = cases.clone();
        let flags = flags.clone();
        let metrics = metrics.clone();
        let local_credentials = local_credentials.clone();
        let companion_credentials = companion_credentials.clone();
        let templates = templates.clone();
        let announcements = announcements.clone();
        let taxonomy = taxonomy.clone();
        let ratings = ratings.clone();
        let appeals = appeals.clone();
        let error = error.clone();
        move || {
            spawn_local(async move {
                if let Ok(data) = get_json::<Vec<ReferenceItem>>(&format!("{API}/campuses")).await {
                    campuses.set(data);
                }
                if let Ok(data) = get_json::<Vec<ReferenceItem>>(&format!("{API}/inventory/devices")).await {
                    devices.set(data);
                }
                if let Ok(data) = get_json::<Vec<SearchHistoryItem>>(&format!("{API}/search/history")).await {
                    search_history.set(data);
                }
                if let Ok(data) = get_json::<Vec<RecommendationCard>>(&format!("{API}/recommendations")).await {
                    recommendations.set(data);
                }
                if let Ok(data) = get_json::<RecommendationSettings>(&format!("{API}/settings/recommendations")).await {
                    recommendation_settings.set(data);
                }
                if let Ok(data) = get_json::<Vec<OrderResponse>>(&format!("{API}/orders")).await {
                    orders.set(data);
                }
                if let Ok(data) = get_json::<Vec<InventoryDocument>>(&format!("{API}/inventory/documents")).await {
                    documents.set(data);
                }
                if let Ok(data) = get_json::<Vec<ShipmentRecord>>(&format!("{API}/shipments")).await {
                    shipments.set(data);
                }
                if let Ok(data) = get_json::<Vec<AfterSalesCase>>(&format!("{API}/after-sales/cases")).await {
                    cases.set(data);
                }
                if let Ok(data) = get_json::<Vec<FeatureFlag>>(&format!("{API}/admin/feature-flags")).await {
                    flags.set(data);
                }
                if let Ok(data) = get_json::<DashboardMetrics>(&format!("{API}/admin/dashboard/metrics")).await {
                    metrics.set(Some(data));
                }
                if let Ok(data) = get_json::<Vec<CredentialRecord>>(&format!("{API}/admin/local-credentials")).await {
                    local_credentials.set(data);
                }
                if let Ok(data) = get_json::<Vec<CompanionCredentialRecord>>(&format!("{API}/admin/companion-credentials")).await {
                    companion_credentials.set(data);
                }
                if let Ok(data) = get_json::<Vec<TemplateRecord>>(&format!("{API}/admin/templates")).await {
                    templates.set(data);
                }
                if let Ok(data) = get_json::<Vec<AnnouncementRecord>>(&format!("{API}/admin/announcements")).await {
                    announcements.set(data);
                }
                if let Ok(data) = get_json::<Vec<TaxonomyNode>>(&format!("{API}/taxonomy")).await {
                    taxonomy.set(data);
                }
                if let Ok(data) = get_json::<Vec<RatingReview>>(&format!("{API}/admin/ratings-review")).await {
                    ratings.set(data);
                }
                if let Ok(data) = get_json::<Vec<serde_json::Value>>(&format!("{API}/admin/appeals")).await {
                    appeals.set(data);
                }
            });
        }
    };

    let submit_login = {
        let username = username.clone();
        let password = password.clone();
        let user = user.clone();
        let error = error.clone();
        let info = info.clone();
        let fetch_all = fetch_all.clone();
        move |_| {
            spawn_local(async move {
                let payload = serde_json::json!({
                    "username": username.get(),
                    "password": password.get()
                });
                match post_json::<AuthUser>(&format!("{API}/auth/login"), payload).await {
                    Ok(auth_user) => {
                        user.set(Some(auth_user));
                        error.set(String::new());
                        info.set("Logged in. Workspace data loaded.".into());
                        fetch_all();
                    }
                    Err(err) => error.set(err),
                }
            });
        }
    };

    let run_search = {
        let listings = listings.clone();
        let search_query = search_query.clone();
        let search_category = search_category.clone();
        let search_condition = search_condition.clone();
        let search_campus = search_campus.clone();
        let search_sort = search_sort.clone();
        let min_price = min_price.clone();
        let max_price = max_price.clone();
        let zip_code = zip_code.clone();
        let error = error.clone();
        let search_history = search_history.clone();
        move |_| {
            spawn_local(async move {
                let mut query = vec![format!("sort={}", search_sort.get())];
                if !search_query.get().is_empty() {
                    query.push(format!("q={}", encode(&search_query.get())));
                }
                if !search_category.get().is_empty() {
                    query.push(format!("category={}", encode(&search_category.get())));
                }
                if !search_condition.get().is_empty() {
                    query.push(format!("condition={}", encode(&search_condition.get())));
                }
                if !search_campus.get().is_empty() {
                    query.push(format!("campus={}", encode(&search_campus.get())));
                }
                if !min_price.get().is_empty() {
                    query.push(format!("min_price={}", encode(&min_price.get())));
                }
                if !max_price.get().is_empty() {
                    query.push(format!("max_price={}", encode(&max_price.get())));
                }
                if !zip_code.get().is_empty() {
                    query.push(format!("zip_code={}", encode(&zip_code.get())));
                }

                match get_json::<Vec<ListingCard>>(&format!("{API}/listings/search?{}", query.join("&"))).await {
                    Ok(data) => {
                        listings.set(data);
                        error.set(String::new());
                        if let Ok(history) = get_json::<Vec<SearchHistoryItem>>(&format!("{API}/search/history")).await {
                            search_history.set(history);
                        }
                    }
                    Err(err) => error.set(err),
                }
            });
        }
    };

    let load_suggestions = {
        let search_query = search_query.clone();
        let suggestions = suggestions.clone();
        move |_| {
            let value = search_query.get();
            spawn_local(async move {
                if value.is_empty() {
                    suggestions.set(Vec::new());
                } else if let Ok(data) =
                    get_json::<SuggestionResponse>(&format!("{API}/search/suggestions?q={}", encode(&value))).await
                {
                    suggestions.set(data.suggestions);
                }
            });
        }
    };

    let refresh_detail = {
        let selected_listing = selected_listing.clone();
        let error = error.clone();
        move |id: String| {
            spawn_local(async move {
                let _ = post_empty(&format!("{API}/listings/{id}/view")).await;
                match get_json::<ListingDetail>(&format!("{API}/listings/{id}")).await {
                    Ok(data) => selected_listing.set(Some(data)),
                    Err(err) => error.set(err),
                }
            });
        }
    };

    let save_settings = {
        let recommendation_settings = recommendation_settings.clone();
        let info = info.clone();
        move |_| {
            let enabled = recommendation_settings.get().recommendations_enabled;
            spawn_local(async move {
                let payload = serde_json::json!({ "recommendations_enabled": enabled });
                if post_json::<RecommendationSettings>(&format!("{API}/settings/recommendations"), payload).await.is_ok() {
                    info.set("Recommendation setting saved.".into());
                }
            });
        }
    };

    view! {
        <main class="shell">
            <section class="topbar">
                <div>
                    <div class="eyebrow">"DepotCycle Part 2"</div>
                    <h1>"Offline device lifecycle marketplace"</h1>
                    <p class="lede">"One shell for shoppers, clerks, managers, support agents, and administrators. All flows call the local Axum API behind the Nginx proxy."</p>
                </div>
                <div class="login-panel">
                    <input prop:value=move || username.get() on:input=move |ev| username.set(event_target_value(&ev)) placeholder="username" />
                    <input prop:value=move || password.get() on:input=move |ev| password.set(event_target_value(&ev)) type="password" placeholder="password" />
                    <button class="primary" on:click=submit_login>"Log in"</button>
                    <div class="demo-users">
                        <button on:click=move |_| { username.set("admin".into()); password.set("DepotCycleAdmin123!".into()); }>"Admin"</button>
                        <button on:click=move |_| { username.set("shopper".into()); password.set("DepotCycleDemo123!".into()); }>"Shopper"</button>
                        <button on:click=move |_| { username.set("clerk".into()); password.set("DepotCycleDemo123!".into()); }>"Clerk"</button>
                        <button on:click=move |_| { username.set("manager".into()); password.set("DepotCycleDemo123!".into()); }>"Manager"</button>
                        <button on:click=move |_| { username.set("support".into()); password.set("DepotCycleDemo123!".into()); }>"Support"</button>
                    </div>
                    <Show when=move || user.get().is_some() fallback=|| view! { <span class="muted">"Login required for most flows."</span> }>
                        <div class="pill">{move || user.get().map(|u| format!("{} workspace", u.role_name)).unwrap_or_default()}</div>
                    </Show>
                </div>
            </section>

            <Show when=move || !error.get().is_empty() fallback=|| view! {}>
                <div class="flash error">{move || error.get()}</div>
            </Show>
            <Show when=move || !info.get().is_empty() fallback=|| view! {}>
                <div class="flash info">{move || info.get()}</div>
            </Show>

            <section class="grid two">
                <article class="card">
                    <h2>"Discovery"</h2>
                    <div class="form-grid">
                        <input prop:value=move || search_query.get() on:input=move |ev| {
                            search_query.set(event_target_value(&ev));
                            load_suggestions(ev);
                        } placeholder="search devices or keywords" />
                        <select on:change=move |ev| search_sort.set(event_target_value(&ev))>
                            <option value="relevance">"Relevance"</option>
                            <option value="popularity">"Popularity"</option>
                            <option value="distance">"Approx. distance"</option>
                            <option value="price">"Price"</option>
                        </select>
                        <input prop:value=move || search_category.get() on:input=move |ev| search_category.set(event_target_value(&ev)) placeholder="category slug" />
                        <input prop:value=move || search_condition.get() on:input=move |ev| search_condition.set(event_target_value(&ev)) placeholder="condition" />
                        <input prop:value=move || search_campus.get() on:input=move |ev| search_campus.set(event_target_value(&ev)) placeholder="campus name" />
                        <input prop:value=move || min_price.get() on:input=move |ev| min_price.set(event_target_value(&ev)) placeholder="min USD" />
                        <input prop:value=move || max_price.get() on:input=move |ev| max_price.set(event_target_value(&ev)) placeholder="max USD" />
                        <input prop:value=move || zip_code.get() on:input=move |ev| zip_code.set(event_target_value(&ev)) placeholder="ZIP for distance" />
                    </div>
                    <div class="action-row">
                        <button class="primary" on:click=run_search>"Search"</button>
                        <button on:click=move |_| {
                            spawn_local(async move {
                                let _ = Request::delete(&format!("{API}/search/history"))
                                    .credentials(RequestCredentials::Include)
                                    .send()
                                    .await;
                            });
                            search_history.set(Vec::new());
                        }>"Clear history"</button>
                    </div>
                    <div class="pill-list">
                        <For each=move || suggestions.get() key=|item| item.clone() children=move |item| {
                            let item_click = item.clone();
                            view! {
                                <button class="pill" on:click=move |_| search_query.set(item_click.clone())>{item.clone()}</button>
                            }
                        } />
                    </div>
                    <div class="history-list">
                        <For each=move || search_history.get() key=|item| item.id.clone() children=move |item| {
                            let qt = item.query_text.clone();
                            view! {
                            <button class="history-item" on:click=move |_| search_query.set(qt.clone())>
                                <span>{item.query_text.clone()}</span>
                                <small>{item.created_at}</small>
                            </button>
                        }} />
                    </div>
                    <div class="results">
                        <For each=move || listings.get() key=|item| item.id.clone() children=move |item| {
                            let item_id = item.id.clone();
                            let favorite_id = item.id.clone();
                            let buy_id = item.id.clone();
                            view! {
                                <div class="result-card">
                                    <div>
                                        <h3>{item.title.clone()}</h3>
                                        <p>{item.description.unwrap_or_default()}</p>
                                        <small>{format!("{} | {} | {}", item.category_slug.unwrap_or_default(), item.condition_code.unwrap_or_default(), item.campus_name.unwrap_or_default())}</small>
                                    </div>
                                    <div class="result-actions">
                                        <strong>{format_usd(item.price_cents)}</strong>
                                        <button on:click=move |_| refresh_detail(item_id.clone())>"View"</button>
                                        <button on:click=move |_| {
                                            let fav = favorite_id.clone();
                                            spawn_local(async move {
                                                let _ = post_empty(&format!("{API}/favorites/{fav}")).await;
                                            });
                                        }>"Favorite"</button>
                                        <button class="primary" on:click=move |_| {
                                            let bid = buy_id.clone();
                                            spawn_local(async move {
                                                let _ = post_json::<OrderResponse>(&format!("{API}/orders"), serde_json::json!({"listing_id": bid, "quantity": 1})).await;
                                            });
                                        }>"Buy"</button>
                                    </div>
                                </div>
                            }
                        } />
                    </div>
                </article>

                <article class="card">
                    <h2>"Detail & Recommendations"</h2>
                    <Show when=move || selected_listing.get().is_some() fallback=|| view! {
                        <p class="muted">"Open a listing to see popularity, available inventory, and recommendations."</p>
                    }>
                        {move || selected_listing.get().map(|detail| view! {
                            <div class="detail">
                                <h3>{detail.listing.title}</h3>
                                <p>{detail.listing.description.unwrap_or_default()}</p>
                                <div class="metric-strip">
                                    <div><strong>{detail.popularity_score}</strong><span>"Popularity"</span></div>
                                    <div><strong>{detail.inventory_on_hand}</strong><span>"On hand"</span></div>
                                    <div><strong>{format_usd(detail.listing.price_cents)}</strong><span>"Price"</span></div>
                                </div>
                                <h4>"Recommended next"</h4>
                                <For each=move || detail.recommendations.clone() key=|item| item.listing_id.clone() children=move |item| view! {
                                    <div class="mini-card">
                                        <strong>{item.title}</strong>
                                        <small>{item.reason}</small>
                                        <span>{format_usd(item.price_cents)}</span>
                                    </div>
                                } />
                            </div>
                        })}
                    </Show>
                    <div class="settings-row">
                        <label>
                            <input
                                type="checkbox"
                                prop:checked=move || recommendation_settings.get().recommendations_enabled
                                on:change=move |_| {
                                    let current = recommendation_settings.get();
                                    recommendation_settings.set(RecommendationSettings {
                                        recommendations_enabled: !current.recommendations_enabled,
                                    });
                                }
                            />
                            "Enable personalized recommendations"
                        </label>
                        <button on:click=save_settings>"Save setting"</button>
                    </div>
                    <div class="mini-list">
                        <For each=move || recommendations.get() key=|item| item.listing_id.clone() children=move |item| view! {
                            <div class="mini-card">
                                <strong>{item.title}</strong>
                                <small>{item.reason}</small>
                                <span>{format_usd(item.price_cents)}</span>
                            </div>
                        } />
                    </div>
                </article>
            </section>

            <section class="grid two">
                <article class="card">
                    <h2>"Inventory Documents"</h2>
                    <div class="form-grid">
                        <select on:change=move |ev| doc_type.set(event_target_value(&ev))>
                            <option value="receiving">"receiving"</option>
                            <option value="issuing">"issuing"</option>
                            <option value="transfer">"transfer"</option>
                            <option value="return">"return"</option>
                            <option value="loan">"loan"</option>
                            <option value="scrap">"scrap"</option>
                        </select>
                        <input prop:value=move || doc_reference.get() on:input=move |ev| doc_reference.set(event_target_value(&ev)) placeholder="reference" />
                        <select on:change=move |ev| doc_device_id.set(event_target_value(&ev))>
                            <option value="">"device"</option>
                            <For each=move || devices.get() key=|item| item.id.clone() children=move |item| view! {
                                <option value={item.id.clone()}>{item.label}</option>
                            } />
                        </select>
                        <select on:change=move |ev| doc_target_campus.set(event_target_value(&ev))>
                            <option value="">"target campus"</option>
                            <For each=move || campuses.get() key=|item| item.id.clone() children=move |item| view! {
                                <option value={item.id.clone()}>{item.label}</option>
                            } />
                        </select>
                        <input prop:value=move || doc_unit_value.get() on:input=move |ev| doc_unit_value.set(event_target_value(&ev)) placeholder="unit value cents" />
                    </div>
                    <button class="primary" on:click=move |_| {
                        let payload = serde_json::json!({
                            "doc_type": doc_type.get(),
                            "reference_no": doc_reference.get(),
                            "source_campus_id": null,
                            "target_campus_id": if doc_target_campus.get().is_empty() { serde_json::Value::Null } else { serde_json::Value::String(doc_target_campus.get()) },
                            "notes": "Created from UI",
                            "lines": [{
                                "device_id": doc_device_id.get(),
                                "quantity": 1,
                                "unit_value_cents": doc_unit_value.get().parse::<i64>().unwrap_or(10000),
                                "target_campus_id": if doc_target_campus.get().is_empty() { serde_json::Value::Null } else { serde_json::Value::String(doc_target_campus.get()) },
                                "notes": "UI line"
                            }]
                        });
                        let documents = documents.clone();
                        spawn_local(async move {
                            let _ = post_json::<InventoryDocResponse>(&format!("{API}/inventory/documents"), payload).await;
                            if let Ok(data) = get_json::<Vec<InventoryDocument>>(&format!("{API}/inventory/documents")).await {
                                documents.set(data);
                            }
                        });
                    }>"Create document"</button>
                    <For each=move || documents.get() key=|item| item.id.clone() children=move |item| {
                        let doc_id_a = item.id.clone();
                        let doc_id_b = item.id.clone();
                        let documents = documents.clone();
                        view! {
                            <div class="mini-card">
                                <strong>{format!("{} {}", item.doc_type, item.reference_no)}</strong>
                                <small>{format!("status: {}", item.workflow_status)}</small>
                                <span>{item.created_at}</span>
                                <div class="action-row">
                                    <button on:click=move |_| {
                                        let documents = documents.clone();
                                        let id_a = doc_id_a.clone();
                                        spawn_local(async move {
                                            let _ = post_empty(&format!("{API}/inventory/documents/{id_a}/approve")).await;
                                            if let Ok(data) = get_json::<Vec<InventoryDocument>>(&format!("{API}/inventory/documents")).await {
                                                documents.set(data);
                                            }
                                        });
                                    }>"Approve"</button>
                                    <button on:click=move |_| {
                                        let documents = documents.clone();
                                        let id_b = doc_id_b.clone();
                                        spawn_local(async move {
                                            let _ = post_empty(&format!("{API}/inventory/documents/{id_b}/execute")).await;
                                            if let Ok(data) = get_json::<Vec<InventoryDocument>>(&format!("{API}/inventory/documents")).await {
                                                documents.set(data);
                                            }
                                        });
                                    }>"Execute"</button>
                                </div>
                            </div>
                        }
                    } />
                </article>

                <article class="card">
                    <h2>"Shipments & After-Sales"</h2>
                    <div class="subcard">
                        <h3>"Create shipment"</h3>
                        <div class="form-grid">
                            <select on:change=move |ev| shipment_device_id.set(event_target_value(&ev))>
                                <option value="">"device"</option>
                                <For each=move || devices.get() key=|item| item.id.clone() children=move |item| view! {
                                    <option value={item.id.clone()}>{item.label}</option>
                                } />
                            </select>
                            <select on:change=move |ev| shipment_from.set(event_target_value(&ev))>
                                <option value="">"from campus"</option>
                                <For each=move || campuses.get() key=|item| item.id.clone() children=move |item| view! {
                                    <option value={item.id.clone()}>{item.label}</option>
                                } />
                            </select>
                            <select on:change=move |ev| shipment_to.set(event_target_value(&ev))>
                                <option value="">"to campus"</option>
                                <For each=move || campuses.get() key=|item| item.id.clone() children=move |item| view! {
                                    <option value={item.id.clone()}>{item.label}</option>
                                } />
                            </select>
                            <input prop:value=move || shipment_carrier.get() on:input=move |ev| shipment_carrier.set(event_target_value(&ev)) placeholder="carrier" />
                            <input prop:value=move || shipment_tracking.get() on:input=move |ev| shipment_tracking.set(event_target_value(&ev)) placeholder="tracking" />
                        </div>
                        <button class="primary" on:click=move |_| {
                            let shipments = shipments.clone();
                            spawn_local(async move {
                                let _ = post_json::<ShipmentRecord>(&format!("{API}/shipments"), serde_json::json!({
                                    "device_id": shipment_device_id.get(),
                                    "from_campus_id": shipment_from.get(),
                                    "to_campus_id": shipment_to.get(),
                                    "carrier_name": shipment_carrier.get(),
                                    "tracking_number": shipment_tracking.get()
                                })).await;
                                if let Ok(data) = get_json::<Vec<ShipmentRecord>>(&format!("{API}/shipments")).await {
                                    shipments.set(data);
                                }
                            });
                        }>"Create shipment order"</button>
                        <For each=move || shipments.get() key=|item| item.id.clone() children=move |item| {
                            let shipment_id = item.id.clone();
                            let shipment_hist_id = item.id.clone();
                            let sid1 = shipment_id.clone();
                            let sid2 = shipment_id.clone();
                            let sid3 = shipment_id.clone();
                            let sid4 = shipment_id.clone();
                            view! {
                                <div class="mini-card">
                                    <strong>{item.order_number.clone()}</strong>
                                    <small>{format!("{} | integration {}", item.status, if item.integration_enabled == 1 { "enabled" } else { "manual" })}</small>
                                    <div class="action-row">
                                        <button on:click=move |_| transition_and_refresh(&format!("{API}/shipments/{sid1}/transition"), "packed", shipments.clone())>"Pack"</button>
                                        <button on:click=move |_| transition_and_refresh(&format!("{API}/shipments/{sid2}/transition"), "shipped", shipments.clone())>"Ship"</button>
                                        <button on:click=move |_| transition_and_refresh(&format!("{API}/shipments/{sid3}/transition"), "received", shipments.clone())>"Receive"</button>
                                        <button on:click=move |_| transition_and_refresh(&format!("{API}/shipments/{sid4}/transition"), "completed", shipments.clone())>"Complete"</button>
                                    </div>
                                    <button on:click=move |_| {
                                        let shipment_history = shipment_history.clone();
                                        let hist_id = shipment_hist_id.clone();
                                        spawn_local(async move {
                                            if let Ok(data) = get_json::<Vec<TimelineEntry>>(&format!("{API}/shipments/{hist_id}/history")).await {
                                                shipment_history.set(data);
                                            }
                                        });
                                    }>"Timeline"</button>
                                </div>
                            }
                        } />
                        <For each=move || shipment_history.get() key=|item| item.changed_at.clone() children=move |item| view! {
                            <div class="timeline-item">{format!("{} -> {} at {}", item.from_status.unwrap_or_else(|| "start".into()), item.to_status, item.changed_at)}</div>
                        } />
                    </div>

                    <div class="subcard">
                        <h3>"After-sales"</h3>
                        <div class="form-grid">
                            <select on:change=move |ev| case_type.set(event_target_value(&ev))>
                                <option value="return">"return"</option>
                                <option value="exchange">"exchange"</option>
                                <option value="refund">"refund"</option>
                            </select>
                            <input prop:value=move || case_reason.get() on:input=move |ev| case_reason.set(event_target_value(&ev)) placeholder="reason" />
                            <input prop:value=move || evidence_media_id.get() on:input=move |ev| evidence_media_id.set(event_target_value(&ev)) placeholder="media id for evidence" />
                        </div>
                        <button class="primary" on:click=move |_| {
                            let cases = cases.clone();
                            spawn_local(async move {
                                let _ = post_json::<AfterSalesCase>(&format!("{API}/after-sales/cases"), serde_json::json!({
                                    "case_type": case_type.get(),
                                    "reason": case_reason.get()
                                })).await;
                                if let Ok(data) = get_json::<Vec<AfterSalesCase>>(&format!("{API}/after-sales/cases")).await {
                                    cases.set(data);
                                }
                            });
                        }>"Create case"</button>
                        <For each=move || cases.get() key=|item| item.id.clone() children=move |item| {
                            let case_id_a = item.id.clone();
                            let case_id_b = item.id.clone();
                            let case_id_c = item.id.clone();
                            let case_id_attach = item.id.clone();
                            let case_id_upload = item.id.clone();
                            let case_id_timeline = item.id.clone();
                            let file_input = create_node_ref::<html::Input>();
                            view! {
                                <div class="mini-card">
                                    <strong>{format!("{} {}", item.case_type, item.status)}</strong>
                                    <small>{item.reason.clone()}</small>
                                    <span>{format!("First response due {} | final decision due {}", item.first_response_due_at, item.final_decision_due_at)}</span>
                                    <div class="action-row">
                                        <button on:click=move |_| case_transition_and_refresh(&format!("{API}/after-sales/cases/{case_id_a}/transition"), "evidence_pending", cases.clone())>"Need evidence"</button>
                                        <button on:click=move |_| case_transition_and_refresh(&format!("{API}/after-sales/cases/{case_id_b}/transition"), "under_review", cases.clone())>"Review"</button>
                                        <button on:click=move |_| case_transition_and_refresh(&format!("{API}/after-sales/cases/{case_id_c}/transition"), "approved", cases.clone())>"Approve"</button>
                                    </div>
                                    <button on:click=move |_| {
                                        let media_id = evidence_media_id.get();
                                        let case_id_attach = case_id_attach.clone();
                                        spawn_local(async move {
                                            if !media_id.is_empty() {
                                                let _ = post_json_value(&format!("{API}/after-sales/cases/{case_id_attach}/evidence"), serde_json::json!({"media_id": media_id})).await;
                                            }
                                        });
                                    }>"Attach evidence"</button>
                                    <input type="file" node_ref=file_input />
                                    <button
                                        disabled=move || upload_in_progress.get()
                                        on:click=move |_| {
                                        let case_id_upload = case_id_upload.clone();
                                        let maybe_file = file_input
                                            .get()
                                            .and_then(|input| input.files())
                                            .and_then(|files| files.get(0));
                                        if let Some(file) = maybe_file {
                                            let progress = upload_progress.clone();
                                            let percent = upload_percent.clone();
                                            let in_progress = upload_in_progress.clone();
                                            spawn_local(async move {
                                                in_progress.set(true);
                                                progress.set("Starting chunked upload...".into());
                                                percent.set(0);
                                                match chunked_upload_and_attach(&case_id_upload, file, progress, percent).await {
                                                    Ok(msg) => progress.set(msg),
                                                    Err(e) => progress.set(format!("Upload failed: {e}")),
                                                }
                                                in_progress.set(false);
                                            });
                                        }
                                    }>"Chunked upload + attach evidence"</button>
                                    <Show when=move || !upload_progress.get().is_empty() fallback=|| ()>
                                        <div class="upload-status">
                                            <div class="progress-bar" style="width:100%;background:#333;height:8px;border-radius:4px;margin-top:4px;">
                                                <div style=move || format!("width:{}%;background:#4caf50;height:8px;border-radius:4px;transition:width 0.2s", upload_percent.get())></div>
                                            </div>
                                            <small>{move || upload_progress.get()}</small>
                                        </div>
                                    </Show>
                                    <button on:click=move |_| {
                                        let case_history = case_history.clone();
                                        let hist_id = case_id_timeline.clone();
                                        spawn_local(async move {
                                            if let Ok(data) = get_json::<Vec<TimelineEntry>>(&format!("{API}/after-sales/cases/{hist_id}/history")).await {
                                                case_history.set(data);
                                            }
                                        });
                                    }>"Timeline"</button>
                                </div>
                            }
                        } />
                        <For each=move || case_history.get() key=|item| item.changed_at.clone() children=move |item| view! {
                            <div class="timeline-item">{format!("{} -> {} at {}", item.from_status.unwrap_or_else(|| "start".into()), item.to_status, item.changed_at)}</div>
                        } />
                    </div>
                </article>
            </section>

            <section class="grid two">
                <article class="card">
                    <h2>"Admin Dashboard & Flags"</h2>
                    <Show when=move || metrics.get().is_some() fallback=|| view! { <p class="muted">"Log in as manager/admin to load metrics."</p> }>
                        {move || metrics.get().map(|m| view! {
                            <div class="metric-grid">
                                <div><strong>{m.total_users}</strong><span>"Users"</span></div>
                                <div><strong>{m.total_events}</strong><span>"Events"</span></div>
                                <div><strong>{format!("{:.1}%", m.conversion_rate_percent)}</strong><span>"Conversion"</span></div>
                                <div><strong>{format!("{:.1}", m.average_rating)}</strong><span>"Avg rating"</span></div>
                                <div><strong>{m.open_support_cases}</strong><span>"Open support cases"</span></div>
                                <div><strong>{m.total_feature_flags}</strong><span>"Feature flags"</span></div>
                            </div>
                        })}
                    </Show>
                    <div class="form-grid">
                        <input prop:value=move || flag_rollout.get() on:input=move |ev| flag_rollout.set(event_target_value(&ev)) placeholder="rollout %" />
                    </div>
                    <For each=move || flags.get() key=|item| item.id.clone() children=move |item| {
                        let flag_id = item.id.clone();
                        let enabled = item.enabled == 1;
                        let flags = flags.clone();
                        view! {
                            <div class="mini-card">
                                <strong>{item.key.clone()}</strong>
                                <small>{item.description.unwrap_or_default()}</small>
                                <span>{format!("enabled={} rollout={}%", enabled, item.rollout_percent)}</span>
                                <button on:click=move |_| {
                                    let flags = flags.clone();
                                    let next_enabled = !enabled;
                                    let rollout = flag_rollout.get().parse::<i64>().unwrap_or(item.rollout_percent);
                                    let fid = flag_id.clone();
                                    spawn_local(async move {
                                        let _ = put_json::<FeatureFlag>(&format!("{API}/admin/feature-flags/{fid}"), serde_json::json!({
                                            "enabled": next_enabled,
                                            "rollout_percent": rollout
                                        })).await;
                                        if let Ok(data) = get_json::<Vec<FeatureFlag>>(&format!("{API}/admin/feature-flags")).await {
                                            flags.set(data);
                                        }
                                    });
                                }>{if enabled { "Disable" } else { "Enable" }}</button>
                            </div>
                        }
                    } />
                </article>

                <article class="card">
                    <h2>"System Ops"</h2>
                    <div class="subcard">
                        <h3>"Taxonomy"</h3>
                        <div class="form-grid">
                            <input prop:value=move || taxonomy_name.get() on:input=move |ev| taxonomy_name.set(event_target_value(&ev)) placeholder="name" />
                            <input prop:value=move || taxonomy_slug.get() on:input=move |ev| taxonomy_slug.set(event_target_value(&ev)) placeholder="slug" />
                        </div>
                        <button on:click=move |_| {
                            let taxonomy = taxonomy.clone();
                            spawn_local(async move {
                                let _ = post_json::<TaxonomyNode>(&format!("{API}/taxonomy"), serde_json::json!({
                                    "parent_id": null,
                                    "name": taxonomy_name.get(),
                                    "slug": taxonomy_slug.get(),
                                    "level": 1,
                                    "seo_title": taxonomy_name.get(),
                                    "seo_description": "Added from UI",
                                    "seo_keywords": "local,offline",
                                    "topic_page_path": format!("/topics/{}", taxonomy_slug.get())
                                })).await;
                                if let Ok(data) = get_json::<Vec<TaxonomyNode>>(&format!("{API}/taxonomy")).await {
                                    taxonomy.set(data);
                                }
                            });
                        }>"Add taxonomy node"</button>
                        <For each=move || taxonomy.get() key=|item| item.id.clone() children=move |item| view! {
                            <div class="mini-card">
                                <strong>{item.name}</strong>
                                <small>{format!("slug={} level={}", item.slug, item.level)}</small>
                            </div>
                        } />
                    </div>

                    <div class="subcard">
                        <h3>"Credentials"</h3>
                        <div class="form-grid">
                            <input prop:value=move || local_cred_label.get() on:input=move |ev| local_cred_label.set(event_target_value(&ev)) placeholder="local label" />
                            <input prop:value=move || local_cred_user.get() on:input=move |ev| local_cred_user.set(event_target_value(&ev)) placeholder="local username" />
                            <input prop:value=move || local_cred_secret.get() on:input=move |ev| local_cred_secret.set(event_target_value(&ev)) placeholder="local secret" />
                            <button on:click=move |_| {
                                let local_credentials = local_credentials.clone();
                                spawn_local(async move {
                                    let _ = post_json::<CredentialRecord>(&format!("{API}/admin/local-credentials"), serde_json::json!({
                                        "label": local_cred_label.get(),
                                        "username": local_cred_user.get(),
                                        "secret": local_cred_secret.get(),
                                        "notes": "UI created"
                                    })).await;
                                    if let Ok(data) = get_json::<Vec<CredentialRecord>>(&format!("{API}/admin/local-credentials")).await {
                                        local_credentials.set(data);
                                    }
                                });
                            }>"Save local credential"</button>
                        </div>
                        <div class="form-grid">
                            <input prop:value=move || companion_label.get() on:input=move |ev| companion_label.set(event_target_value(&ev)) placeholder="companion label" />
                            <input prop:value=move || companion_provider.get() on:input=move |ev| companion_provider.set(event_target_value(&ev)) placeholder="provider" />
                            <input prop:value=move || companion_user.get() on:input=move |ev| companion_user.set(event_target_value(&ev)) placeholder="companion username" />
                            <input prop:value=move || companion_secret.get() on:input=move |ev| companion_secret.set(event_target_value(&ev)) placeholder="companion secret" />
                            <button on:click=move |_| {
                                let companion_credentials = companion_credentials.clone();
                                spawn_local(async move {
                                    let _ = post_json::<CompanionCredentialRecord>(&format!("{API}/admin/companion-credentials"), serde_json::json!({
                                        "label": companion_label.get(),
                                        "provider": companion_provider.get(),
                                        "endpoint": "offline://manual",
                                        "username": companion_user.get(),
                                        "secret": companion_secret.get(),
                                        "notes": "UI created"
                                    })).await;
                                    if let Ok(data) = get_json::<Vec<CompanionCredentialRecord>>(&format!("{API}/admin/companion-credentials")).await {
                                        companion_credentials.set(data);
                                    }
                                });
                            }>"Save companion credential"</button>
                        </div>
                    </div>

                    <div class="subcard">
                        <h3>"Templates & Announcements"</h3>
                        <div class="form-grid">
                            <input prop:value=move || template_title.get() on:input=move |ev| template_title.set(event_target_value(&ev)) placeholder="template title" />
                            <input prop:value=move || template_key.get() on:input=move |ev| template_key.set(event_target_value(&ev)) placeholder="template key" />
                            <textarea prop:value=move || template_content.get() on:input=move |ev| template_content.set(event_target_value(&ev))></textarea>
                            <button on:click=move |_| {
                                let templates = templates.clone();
                                spawn_local(async move {
                                    let _ = post_json::<TemplateRecord>(&format!("{API}/admin/templates"), serde_json::json!({
                                        "kind": "content",
                                        "key": template_key.get(),
                                        "title": template_title.get(),
                                        "content": template_content.get(),
                                        "is_active": true
                                    })).await;
                                    if let Ok(data) = get_json::<Vec<TemplateRecord>>(&format!("{API}/admin/templates")).await {
                                        templates.set(data);
                                    }
                                });
                            }>"Create template"</button>
                        </div>
                        <div class="form-grid">
                            <input prop:value=move || announcement_title.get() on:input=move |ev| announcement_title.set(event_target_value(&ev)) placeholder="announcement title" />
                            <textarea prop:value=move || announcement_body.get() on:input=move |ev| announcement_body.set(event_target_value(&ev))></textarea>
                            <button on:click=move |_| {
                                let announcements = announcements.clone();
                                spawn_local(async move {
                                    let _ = post_json::<AnnouncementRecord>(&format!("{API}/admin/announcements"), serde_json::json!({
                                        "title": announcement_title.get(),
                                        "body": announcement_body.get(),
                                        "severity": "info",
                                        "starts_at": null,
                                        "ends_at": null
                                    })).await;
                                    if let Ok(data) = get_json::<Vec<AnnouncementRecord>>(&format!("{API}/admin/announcements")).await {
                                        announcements.set(data);
                                    }
                                });
                            }>"Post announcement"</button>
                        </div>
                    </div>
                </article>
            </section>

            <section class="grid two">
                <article class="card">
                    <h2>"Ratings Review"</h2>
                    <For each=move || ratings.get() key=|item| item.rating_id.clone() children=move |item| view! {
                        <div class="mini-card">
                            <strong>{format!("Rating {}", item.score)}</strong>
                            <small>{item.comments.unwrap_or_default()}</small>
                            <span>{item.review_status.unwrap_or_else(|| "pending".into())}</span>
                        </div>
                    } />
                </article>
                <article class="card">
                    <h2>"Appeal Tickets"</h2>
                    <For each=move || appeals.get() key=|item| item["id"].as_str().unwrap_or_default().to_string() children=move |item| view! {
                        <div class="mini-card">
                            <strong>{item["ticket_no"].as_str().unwrap_or_default().to_string()}</strong>
                            <small>{item["reason"].as_str().unwrap_or_default().to_string()}</small>
                            <span>{item["status"].as_str().unwrap_or_default().to_string()}</span>
                        </div>
                    } />
                </article>
            </section>
        </main>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> });
}

async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    let response = Request::get(url)
        .credentials(RequestCredentials::Include)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(response.text().await.unwrap_or_else(|_| "request failed".into()));
    }
    response.json::<T>().await.map_err(|e| e.to_string())
}

async fn post_json<T: DeserializeOwned>(url: &str, payload: serde_json::Value) -> Result<T, String> {
    let request = Request::post(url)
        .header("Content-Type", "application/json")
        .credentials(RequestCredentials::Include)
        .body(payload.to_string())
        .map_err(|e| e.to_string())?;
    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(response.text().await.unwrap_or_else(|_| "request failed".into()));
    }
    response.json::<T>().await.map_err(|e| e.to_string())
}

async fn post_json_value(url: &str, payload: serde_json::Value) -> Result<(), String> {
    let request = Request::post(url)
        .header("Content-Type", "application/json")
        .credentials(RequestCredentials::Include)
        .body(payload.to_string())
        .map_err(|e| e.to_string())?;
    let response = request.send().await.map_err(|e| e.to_string())?;
    if response.ok() { Ok(()) } else { Err(response.text().await.unwrap_or_else(|_| "request failed".into())) }
}

async fn post_empty(url: &str) -> Result<(), String> {
    let response = Request::post(url)
        .credentials(RequestCredentials::Include)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.ok() { Ok(()) } else { Err(response.text().await.unwrap_or_else(|_| "request failed".into())) }
}

async fn put_json<T: DeserializeOwned>(url: &str, payload: serde_json::Value) -> Result<T, String> {
    let request = Request::put(url)
        .header("Content-Type", "application/json")
        .credentials(RequestCredentials::Include)
        .body(payload.to_string())
        .map_err(|e| e.to_string())?;
    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(response.text().await.unwrap_or_else(|_| "request failed".into()));
    }
    response.json::<T>().await.map_err(|e| e.to_string())
}

const CHUNK_SIZE: f64 = 1_048_576.0; // 1 MiB per chunk
const MAX_RETRIES: u32 = 3;

async fn chunked_upload_and_attach(
    case_id: &str,
    file: File,
    progress: RwSignal<String>,
    percent: RwSignal<i64>,
) -> Result<String, String> {
    let file_size = file.size();
    let total_chunks = ((file_size / CHUNK_SIZE).ceil() as i64).max(1);
    let file_name = file.name();
    let mime_type = file.type_();
    let mime_type = if mime_type.is_empty() { "application/octet-stream".to_string() } else { mime_type };

    // Step 1: Start upload session
    progress.set(format!("Creating upload session ({total_chunks} chunks)..."));
    let session = post_json::<UploadSessionResponse>(
        &format!("{API}/media/uploads/start"),
        serde_json::json!({
            "file_name": file_name,
            "mime_type": mime_type,
            "total_chunks": total_chunks,
            "listing_id": null,
            "expected_sha256": null
        }),
    )
    .await?;
    let session_id = session.session_id;

    // Step 2: Upload chunks with progress and retry
    for i in 0..total_chunks {
        let start = i as f64 * CHUNK_SIZE;
        let end = ((i as f64 + 1.0) * CHUNK_SIZE).min(file_size);
        let blob: Blob = file.slice_with_f64_and_f64(start, end).map_err(|_| "failed to slice file".to_string())?;

        let mut success = false;
        for attempt in 1..=MAX_RETRIES {
            progress.set(format!("Uploading chunk {}/{total_chunks} (attempt {attempt})...", i + 1));

            let array_buffer = wasm_bindgen_futures::JsFuture::from(blob.array_buffer())
                .await
                .map_err(|_| "failed to read chunk".to_string())?;
            let uint8_array = js_sys::Uint8Array::new(&array_buffer);
            let chunk_bytes = uint8_array.to_vec();

            let response = Request::put(&format!("{API}/media/uploads/{session_id}/chunks/{i}"))
                .credentials(RequestCredentials::Include)
                .header("Content-Type", "application/octet-stream")
                .body(chunk_bytes)
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if response.ok() {
                success = true;
                let pct = ((i + 1) as f64 / total_chunks as f64 * 90.0) as i64;
                percent.set(pct);
                break;
            }

            if attempt == MAX_RETRIES {
                let err_text = response.text().await.unwrap_or_else(|_| "upload failed".into());
                return Err(format!("Chunk {} failed after {MAX_RETRIES} retries: {err_text}", i + 1));
            }
            // Brief delay before retry
            gloo_timers::future::TimeoutFuture::new(500 * attempt).await;
        }
        if !success {
            return Err(format!("Chunk {} failed", i + 1));
        }
    }

    // Step 3: Finalize upload with checksum verification
    progress.set("Finalizing upload & verifying checksum...".into());
    percent.set(95);
    let finalize = post_json::<UploadFinalizeResponse>(
        &format!("{API}/media/uploads/{session_id}/finalize"),
        serde_json::json!({ "expected_sha256": null }),
    )
    .await?;

    // Step 4: Attach media to case
    progress.set("Attaching evidence to case...".into());
    post_json_value(
        &format!("{API}/after-sales/cases/{case_id}/evidence"),
        serde_json::json!({ "media_id": finalize.media_id }),
    )
    .await?;

    percent.set(100);
    Ok(format!(
        "Upload complete. Media: {} | SHA-256: {} | Size: {:.1} KB",
        finalize.media_id,
        finalize.sha256,
        file_size / 1024.0
    ))
}

fn transition_and_refresh(url: &str, next_status: &str, signal: RwSignal<Vec<ShipmentRecord>>) {
    let url = url.to_string();
    let next_status = next_status.to_string();
    spawn_local(async move {
        let _ = post_json::<ShipmentRecord>(&url, serde_json::json!({ "next_status": next_status })).await;
        if let Ok(data) = get_json::<Vec<ShipmentRecord>>(&format!("{API}/shipments")).await {
            signal.set(data);
        }
    });
}

fn case_transition_and_refresh(url: &str, next_status: &str, signal: RwSignal<Vec<AfterSalesCase>>) {
    let url = url.to_string();
    let next_status = next_status.to_string();
    spawn_local(async move {
        let _ = post_json::<AfterSalesCase>(&url, serde_json::json!({ "next_status": next_status })).await;
        if let Ok(data) = get_json::<Vec<AfterSalesCase>>(&format!("{API}/after-sales/cases")).await {
            signal.set(data);
        }
    });
}

fn format_usd(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}

fn encode(value: &str) -> String {
    js_sys::encode_uri_component(value).as_string().unwrap_or_default()
}

// ── Frontend component & unit tests ─────────────────────────────────────
// Run with:  wasm-pack test --headless --chrome  (or --firefox)
// These tests import actual frontend types and components.

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    // ── Pure helper tests ───────────────────────────────────────────────

    #[wasm_bindgen_test]
    fn format_usd_basic_conversion() {
        assert_eq!(format_usd(12345), "$123.45");
    }

    #[wasm_bindgen_test]
    fn format_usd_zero() {
        assert_eq!(format_usd(0), "$0.00");
    }

    #[wasm_bindgen_test]
    fn format_usd_single_cent() {
        assert_eq!(format_usd(1), "$0.01");
    }

    #[wasm_bindgen_test]
    fn format_usd_large_value() {
        assert_eq!(format_usd(99999999), "$999999.99");
    }

    #[wasm_bindgen_test]
    fn format_usd_negative() {
        assert_eq!(format_usd(-500), "$-5.00");
    }

    #[wasm_bindgen_test]
    fn format_usd_exact_dollar() {
        assert_eq!(format_usd(100), "$1.00");
    }

    #[wasm_bindgen_test]
    fn encode_escapes_special_characters() {
        let encoded = encode("hello world");
        assert_eq!(encoded, "hello%20world");
    }

    #[wasm_bindgen_test]
    fn encode_handles_empty_string() {
        assert_eq!(encode(""), "");
    }

    #[wasm_bindgen_test]
    fn encode_preserves_alphanumeric() {
        assert_eq!(encode("abc123"), "abc123");
    }

    #[wasm_bindgen_test]
    fn encode_escapes_ampersand_and_equals() {
        let encoded = encode("q=foo&bar=baz");
        assert!(encoded.contains("%3D") || encoded.contains("%26"));
    }

    // ── Frontend type deserialization tests ──────────────────────────────
    // These verify the actual frontend types parse backend JSON correctly.

    #[wasm_bindgen_test]
    fn auth_user_deserializes_from_login_response() {
        let json = r#"{
            "user_id": "u-1",
            "username": "admin",
            "role_name": "Administrator",
            "display_name_masked": "S****r",
            "phone_masked": null
        }"#;
        let user: AuthUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.username, "admin");
        assert_eq!(user.role_name, "Administrator");
        assert!(user.display_name_masked.is_some());
        assert!(user.phone_masked.is_none());
    }

    #[wasm_bindgen_test]
    fn listing_card_deserializes_with_optional_fields() {
        let json = r#"{
            "id": "l-1",
            "title": "ThinkPad X1",
            "description": null,
            "price_cents": 129900,
            "status": "published",
            "created_at": "2026-04-01T00:00:00Z",
            "campus_name": "North Campus",
            "campus_zip_code": "10001",
            "condition_code": "good",
            "category_slug": "laptops"
        }"#;
        let card: ListingCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.id, "l-1");
        assert_eq!(card.price_cents, 129900);
        assert!(card.description.is_none());
        assert_eq!(card.campus_name.unwrap(), "North Campus");
    }

    #[wasm_bindgen_test]
    fn listing_detail_deserializes_with_nested_recommendations() {
        let json = r#"{
            "listing": {
                "id": "l-1",
                "title": "Test",
                "description": null,
                "price_cents": 100,
                "status": "published",
                "created_at": "2026-01-01T00:00:00Z",
                "campus_name": null,
                "campus_zip_code": null,
                "condition_code": null,
                "category_slug": null
            },
            "popularity_score": 42,
            "inventory_on_hand": 3,
            "recommendations": [
                {"listing_id": "l-2", "title": "Rec", "reason": "similar", "price_cents": 200}
            ]
        }"#;
        let detail: ListingDetail = serde_json::from_str(json).unwrap();
        assert_eq!(detail.popularity_score, 42);
        assert_eq!(detail.inventory_on_hand, 3);
        assert_eq!(detail.recommendations.len(), 1);
        assert_eq!(detail.recommendations[0].reason, "similar");
    }

    #[wasm_bindgen_test]
    fn suggestion_response_deserializes() {
        let json = r#"{"suggestions": ["ThinkPad", "Pixel"]}"#;
        let resp: SuggestionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.suggestions.len(), 2);
        assert_eq!(resp.suggestions[0], "ThinkPad");
    }

    #[wasm_bindgen_test]
    fn recommendation_settings_deserializes() {
        let json = r#"{"recommendations_enabled": false}"#;
        let settings: RecommendationSettings = serde_json::from_str(json).unwrap();
        assert!(!settings.recommendations_enabled);
    }

    #[wasm_bindgen_test]
    fn order_response_deserializes() {
        let json = r#"{"order_id": "o-1", "status": "placed", "total_cents": 25000}"#;
        let order: OrderResponse = serde_json::from_str(json).unwrap();
        assert_eq!(order.status, "placed");
        assert_eq!(order.total_cents, 25000);
    }

    #[wasm_bindgen_test]
    fn shipment_record_deserializes() {
        let json = r#"{
            "id": "s-1",
            "order_number": "SHIP-001",
            "status": "created",
            "carrier_name": "FedEx",
            "tracking_number": "TRACK-001",
            "integration_enabled": 0,
            "created_at": "2026-04-01T00:00:00Z"
        }"#;
        let record: ShipmentRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.status, "created");
        assert_eq!(record.integration_enabled, 0);
    }

    #[wasm_bindgen_test]
    fn after_sales_case_deserializes() {
        let json = r#"{
            "id": "c-1",
            "case_type": "return",
            "status": "requested",
            "reason": "Damaged",
            "first_response_due_at": "2026-04-02T00:00:00Z",
            "final_decision_due_at": "2026-04-04T00:00:00Z",
            "created_at": "2026-04-01T00:00:00Z"
        }"#;
        let case: AfterSalesCase = serde_json::from_str(json).unwrap();
        assert_eq!(case.case_type, "return");
        assert_eq!(case.status, "requested");
    }

    #[wasm_bindgen_test]
    fn dashboard_metrics_deserializes_all_fields() {
        let json = r#"{
            "total_users": 10,
            "total_announcements": 2,
            "total_templates": 1,
            "total_local_credentials": 3,
            "total_companion_credentials": 1,
            "total_uploads": 5,
            "total_shipments": 4,
            "total_feature_flags": 2,
            "total_events": 100,
            "active_users_last_30_days": 8,
            "conversion_rate_percent": 12.5,
            "average_rating": 4.2,
            "open_support_cases": 3
        }"#;
        let metrics: DashboardMetrics = serde_json::from_str(json).unwrap();
        assert_eq!(metrics.total_users, 10);
        assert_eq!(metrics.conversion_rate_percent, 12.5);
        assert_eq!(metrics.average_rating, 4.2);
    }

    #[wasm_bindgen_test]
    fn feature_flag_deserializes() {
        let json = r#"{
            "id": "f-1",
            "key": "new-search",
            "description": "test flag",
            "enabled": 1,
            "rollout_percent": 75,
            "audience_rules_json": "{\"mode\":\"local\"}"
        }"#;
        let flag: FeatureFlag = serde_json::from_str(json).unwrap();
        assert_eq!(flag.enabled, 1);
        assert_eq!(flag.rollout_percent, 75);
    }

    #[wasm_bindgen_test]
    fn timeline_entry_deserializes_with_null_from() {
        let json = r#"{"from_status": null, "to_status": "created", "changed_at": "2026-04-01T00:00:00Z"}"#;
        let entry: TimelineEntry = serde_json::from_str(json).unwrap();
        assert!(entry.from_status.is_none());
        assert_eq!(entry.to_status, "created");
    }

    #[wasm_bindgen_test]
    fn template_record_deserializes() {
        let json = r#"{
            "id": "t-1", "kind": "email", "key": "welcome",
            "title": "Welcome", "content": "Hello",
            "version": 2, "is_active": 1,
            "created_at": "2026-04-01T00:00:00Z",
            "updated_at": "2026-04-02T00:00:00Z"
        }"#;
        let tmpl: TemplateRecord = serde_json::from_str(json).unwrap();
        assert_eq!(tmpl.version, 2);
        assert_eq!(tmpl.is_active, 1);
    }

    #[wasm_bindgen_test]
    fn taxonomy_node_deserializes_with_optionals() {
        let json = r#"{
            "id": "n-1", "parent_id": null, "name": "Laptops",
            "slug": "laptops", "level": 1,
            "seo_title": "Laptop Devices",
            "seo_description": null,
            "seo_keywords": "laptop",
            "topic_page_path": "/topics/laptops"
        }"#;
        let node: TaxonomyNode = serde_json::from_str(json).unwrap();
        assert!(node.parent_id.is_none());
        assert!(node.seo_description.is_none());
        assert_eq!(node.level, 1);
    }

    #[wasm_bindgen_test]
    fn credential_record_deserializes() {
        let json = r#"{
            "id": "cr-1", "label": "DB", "username": "db_user",
            "notes": "prod", "created_at": "2026-04-01T00:00:00Z",
            "updated_at": "2026-04-01T00:00:00Z"
        }"#;
        let cred: CredentialRecord = serde_json::from_str(json).unwrap();
        assert_eq!(cred.label, "DB");
    }

    #[wasm_bindgen_test]
    fn announcement_record_deserializes() {
        let json = r#"{
            "id": "a-1", "title": "Notice", "body": "Hello",
            "severity": "info", "starts_at": null, "ends_at": null,
            "created_at": "2026-04-01T00:00:00Z"
        }"#;
        let ann: AnnouncementRecord = serde_json::from_str(json).unwrap();
        assert_eq!(ann.severity, "info");
        assert!(ann.starts_at.is_none());
    }

    // ── Reactive state logic tests ──────────────────────────────────────
    // These test the actual Leptos signal patterns the App component uses.

    #[wasm_bindgen_test]
    fn leptos_signals_initialize_correctly() {
        let _runtime = create_runtime();
        let user = create_rw_signal::<Option<AuthUser>>(None);
        let error = create_rw_signal(String::new());
        let listings = create_rw_signal(Vec::<ListingCard>::new());
        let recommendation_settings = create_rw_signal(RecommendationSettings {
            recommendations_enabled: true,
        });

        assert!(user.get().is_none());
        assert!(error.get().is_empty());
        assert!(listings.get().is_empty());
        assert!(recommendation_settings.get().recommendations_enabled);
    }

    #[wasm_bindgen_test]
    fn recommendation_toggle_flips_state() {
        let _runtime = create_runtime();
        let settings = create_rw_signal(RecommendationSettings {
            recommendations_enabled: true,
        });

        // Simulate the toggle logic from the App component
        let current = settings.get();
        settings.set(RecommendationSettings {
            recommendations_enabled: !current.recommendations_enabled,
        });
        assert!(!settings.get().recommendations_enabled);

        // Toggle back
        let current = settings.get();
        settings.set(RecommendationSettings {
            recommendations_enabled: !current.recommendations_enabled,
        });
        assert!(settings.get().recommendations_enabled);
    }

    #[wasm_bindgen_test]
    fn user_signal_set_and_check_role() {
        let _runtime = create_runtime();
        let user = create_rw_signal::<Option<AuthUser>>(None);

        assert!(user.get().is_none());

        user.set(Some(AuthUser {
            user_id: "u-1".into(),
            username: "admin".into(),
            role_name: "Administrator".into(),
            display_name_masked: None,
            phone_masked: None,
        }));

        assert!(user.get().is_some());
        assert_eq!(user.get().unwrap().role_name, "Administrator");
    }

    #[wasm_bindgen_test]
    fn search_query_signal_builds_url_params() {
        let _runtime = create_runtime();
        let search_query = create_rw_signal("ThinkPad".to_string());
        let search_sort = create_rw_signal("relevance".to_string());
        let search_category = create_rw_signal(String::new());
        let min_price = create_rw_signal(String::new());
        let max_price = create_rw_signal(String::new());

        // Simulate the URL building logic from run_search
        let mut query_parts = vec![format!("sort={}", search_sort.get())];
        if !search_query.get().is_empty() {
            query_parts.push(format!("q={}", encode(&search_query.get())));
        }
        if !search_category.get().is_empty() {
            query_parts.push(format!("category={}", encode(&search_category.get())));
        }
        if !min_price.get().is_empty() {
            query_parts.push(format!("min_price={}", encode(&min_price.get())));
        }
        if !max_price.get().is_empty() {
            query_parts.push(format!("max_price={}", encode(&max_price.get())));
        }

        let url = query_parts.join("&");
        assert!(url.starts_with("sort=relevance"));
        assert!(url.contains("q=ThinkPad"));
        // Empty fields should NOT appear
        assert!(!url.contains("category="));
        assert!(!url.contains("min_price="));
    }

    #[wasm_bindgen_test]
    fn listings_signal_updates_on_search_results() {
        let _runtime = create_runtime();
        let listings = create_rw_signal(Vec::<ListingCard>::new());
        assert!(listings.get().is_empty());

        // Simulate setting search results
        let mock_results = vec![
            ListingCard {
                id: "l-1".into(),
                title: "ThinkPad X1".into(),
                description: Some("Ultrabook".into()),
                price_cents: 129900,
                status: "published".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                campus_name: Some("North".into()),
                campus_zip_code: Some("10001".into()),
                condition_code: Some("good".into()),
                category_slug: Some("laptops".into()),
            },
        ];
        listings.set(mock_results);

        assert_eq!(listings.get().len(), 1);
        assert_eq!(listings.get()[0].title, "ThinkPad X1");
        assert_eq!(format_usd(listings.get()[0].price_cents), "$1299.00");
    }

    #[wasm_bindgen_test]
    fn selected_listing_detail_signal() {
        let _runtime = create_runtime();
        let selected = create_rw_signal::<Option<ListingDetail>>(None);
        assert!(selected.get().is_none());

        selected.set(Some(ListingDetail {
            listing: ListingCard {
                id: "l-1".into(), title: "Test".into(), description: None,
                price_cents: 5000, status: "published".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                campus_name: None, campus_zip_code: None,
                condition_code: None, category_slug: None,
            },
            popularity_score: 15,
            inventory_on_hand: 3,
            recommendations: vec![
                RecommendationCard {
                    listing_id: "l-2".into(),
                    title: "Similar".into(),
                    reason: "category match".into(),
                    price_cents: 4500,
                },
            ],
        }));

        let detail = selected.get().unwrap();
        assert_eq!(detail.popularity_score, 15);
        assert_eq!(detail.inventory_on_hand, 3);
        assert_eq!(detail.recommendations.len(), 1);
    }

    #[wasm_bindgen_test]
    fn error_and_info_signals_work() {
        let _runtime = create_runtime();
        let error = create_rw_signal(String::new());
        let info = create_rw_signal(String::new());

        assert!(error.get().is_empty());
        error.set("Something went wrong".into());
        assert!(!error.get().is_empty());

        info.set("Logged in. Workspace data loaded.".into());
        assert_eq!(info.get(), "Logged in. Workspace data loaded.");

        // Clear error
        error.set(String::new());
        assert!(error.get().is_empty());
    }

    #[wasm_bindgen_test]
    fn search_history_signal_crud_pattern() {
        let _runtime = create_runtime();
        let history = create_rw_signal(Vec::<SearchHistoryItem>::new());

        assert!(history.get().is_empty());

        // Simulate adding items
        let mut items = history.get();
        items.push(SearchHistoryItem {
            id: "h-1".into(),
            query_text: "laptop".into(),
            created_at: "2026-04-01T00:00:00Z".into(),
        });
        history.set(items);
        assert_eq!(history.get().len(), 1);

        // Simulate clear
        history.set(Vec::new());
        assert!(history.get().is_empty());
    }

    #[wasm_bindgen_test]
    fn admin_metrics_signal_optional() {
        let _runtime = create_runtime();
        let metrics = create_rw_signal::<Option<DashboardMetrics>>(None);
        assert!(metrics.get().is_none());

        metrics.set(Some(DashboardMetrics {
            total_users: 5,
            total_announcements: 1,
            total_templates: 0,
            total_local_credentials: 0,
            total_companion_credentials: 0,
            total_uploads: 0,
            total_shipments: 0,
            total_feature_flags: 2,
            total_events: 10,
            active_users_last_30_days: 3,
            conversion_rate_percent: 0.0,
            average_rating: 0.0,
            open_support_cases: 0,
        }));
        assert_eq!(metrics.get().unwrap().total_users, 5);
    }

    #[wasm_bindgen_test]
    fn shipment_and_case_history_signals() {
        let _runtime = create_runtime();
        let shipment_history = create_rw_signal(Vec::<TimelineEntry>::new());
        let case_history = create_rw_signal(Vec::<TimelineEntry>::new());

        shipment_history.set(vec![
            TimelineEntry { from_status: None, to_status: "created".into(), changed_at: "2026-04-01T00:00:00Z".into() },
            TimelineEntry { from_status: Some("created".into()), to_status: "packed".into(), changed_at: "2026-04-01T01:00:00Z".into() },
        ]);
        assert_eq!(shipment_history.get().len(), 2);
        assert!(shipment_history.get()[0].from_status.is_none());
        assert_eq!(shipment_history.get()[1].to_status, "packed");

        case_history.set(vec![
            TimelineEntry { from_status: None, to_status: "requested".into(), changed_at: "2026-04-01T00:00:00Z".into() },
        ]);
        assert_eq!(case_history.get()[0].to_status, "requested");
    }

    // ── Component render tests ──────────────────────────────────────────
    // These mount actual Leptos components into the browser DOM and inspect
    // the rendered HTML output — proving real UI wiring, not just state logic.

    fn mount_app_to_test_container() -> web_sys::Element {
        let document = web_sys::window().unwrap().document().unwrap();
        let container = document.create_element("div").unwrap();
        container.set_id("test-root");
        document.body().unwrap().append_child(&container).unwrap();
        let _dispose = mount_to(container.clone().unchecked_into(), || view! { <App /> });
        container
    }

    fn cleanup_test_container() {
        let document = web_sys::window().unwrap().document().unwrap();
        if let Some(el) = document.get_element_by_id("test-root") {
            el.remove();
        }
    }

    #[wasm_bindgen_test]
    fn render_app_shows_login_form_with_inputs() {
        let container = mount_app_to_test_container();
        let html = container.inner_html();

        // Login heading and description text
        assert!(html.contains("DepotCycle Part 2"), "should render app title");
        assert!(html.contains("Offline device lifecycle marketplace"), "should render h1 heading");

        // Username and password input fields
        let inputs = container.query_selector_all("input").unwrap();
        assert!(inputs.length() >= 2, "should render at least username + password inputs, found {}", inputs.length());

        // Check that username placeholder exists
        assert!(html.contains("username"), "should have username input placeholder");
        // Check that password placeholder exists
        assert!(html.contains("password"), "should have password input placeholder");

        // Log in button
        assert!(html.contains("Log in"), "should render 'Log in' button");

        cleanup_test_container();
    }

    #[wasm_bindgen_test]
    fn render_app_shows_role_quick_select_buttons() {
        let container = mount_app_to_test_container();
        let html = container.inner_html();

        // The demo-users div should contain quick-select buttons for all roles
        assert!(html.contains("Admin"), "should have Admin quick-select button");
        assert!(html.contains("Shopper"), "should have Shopper quick-select button");
        assert!(html.contains("Clerk"), "should have Clerk quick-select button");
        assert!(html.contains("Manager"), "should have Manager quick-select button");
        assert!(html.contains("Support"), "should have Support quick-select button");

        // Should show the demo-users section
        assert!(html.contains("demo-users"), "should render demo-users container class");

        cleanup_test_container();
    }

    #[wasm_bindgen_test]
    fn render_app_shows_logged_out_message_initially() {
        let container = mount_app_to_test_container();
        let html = container.inner_html();

        // When not logged in, the "Login required" fallback should be visible
        assert!(
            html.contains("Login required for most flows"),
            "should show login-required message when not authenticated"
        );

        // The workspace pill should NOT be visible (no user set)
        assert!(
            !html.contains("workspace</div>"),
            "should not show workspace pill when not logged in"
        );

        cleanup_test_container();
    }

    #[wasm_bindgen_test]
    fn render_app_shows_discovery_section() {
        let container = mount_app_to_test_container();
        let html = container.inner_html();

        // Discovery section heading
        assert!(html.contains("Discovery"), "should render Discovery section heading");

        // Search input
        assert!(html.contains("search devices or keywords"), "should render search input placeholder");

        // Sort dropdown options
        assert!(html.contains("Relevance"), "should render Relevance sort option");
        assert!(html.contains("Popularity"), "should render Popularity sort option");
        assert!(html.contains("Price"), "should render Price sort option");

        // Search button
        assert!(html.contains("Search"), "should render Search button");
        assert!(html.contains("Clear history"), "should render Clear history button");

        cleanup_test_container();
    }

    #[wasm_bindgen_test]
    fn render_app_shows_detail_and_recommendations_section() {
        let container = mount_app_to_test_container();
        let html = container.inner_html();

        // Detail panel
        assert!(html.contains("Detail &amp; Recommendations") || html.contains("Detail & Recommendations"),
            "should render Detail & Recommendations section heading");

        // When no listing is selected, the placeholder text should show
        assert!(
            html.contains("Open a listing to see popularity"),
            "should show detail placeholder when no listing selected"
        );

        // Recommendation settings toggle
        assert!(
            html.contains("Enable personalized recommendations"),
            "should render recommendation toggle label"
        );
        assert!(html.contains("Save setting"), "should render Save setting button");

        cleanup_test_container();
    }

    #[wasm_bindgen_test]
    fn render_app_shows_inventory_and_shipment_sections() {
        let container = mount_app_to_test_container();
        let html = container.inner_html();

        // Inventory Documents section
        assert!(html.contains("Inventory Documents"), "should render Inventory Documents heading");
        assert!(html.contains("Create document"), "should render Create document button");
        assert!(html.contains("receiving"), "should render doc type options");

        // Shipments section
        assert!(html.contains("Shipments"), "should render Shipments heading");
        assert!(html.contains("Create shipment order"), "should render Create shipment order button");

        // After-sales section
        assert!(html.contains("After-sales"), "should render After-sales heading");
        assert!(html.contains("Create case"), "should render Create case button");

        cleanup_test_container();
    }

    #[wasm_bindgen_test]
    fn render_app_recommendation_toggle_checkbox_exists() {
        let container = mount_app_to_test_container();

        // Find the recommendation checkbox input
        let checkboxes = container.query_selector_all("input[type='checkbox']").unwrap();
        assert!(
            checkboxes.length() >= 1,
            "should have at least one checkbox (recommendation toggle), found {}",
            checkboxes.length()
        );

        cleanup_test_container();
    }

    #[wasm_bindgen_test]
    fn render_app_search_controls_include_all_expected_filters() {
        let container = mount_app_to_test_container();
        let html = container.inner_html();

        assert!(html.contains("category slug"), "should render category filter input");
        assert!(html.contains("condition"), "should render condition filter input");
        assert!(html.contains("campus name"), "should render campus filter input");
        assert!(html.contains("min USD"), "should render min price input");
        assert!(html.contains("max USD"), "should render max price input");
        assert!(html.contains("ZIP for distance"), "should render ZIP filter input");

        cleanup_test_container();
    }

    #[wasm_bindgen_test]
    fn render_app_shows_workflow_action_buttons_for_core_user_flows() {
        let container = mount_app_to_test_container();
        let html = container.inner_html();

        assert!(html.contains("Favorite"), "should render favorite action button");
        assert!(html.contains("Buy"), "should render buy action button");
        assert!(html.contains("Create shipment order"), "should render shipment creation action");
        assert!(html.contains("Create case"), "should render after-sales case action");
        assert!(html.contains("Chunked upload + attach evidence"), "should render evidence upload action");

        cleanup_test_container();
    }

    #[wasm_bindgen_test]
    fn render_app_shows_admin_sections() {
        let container = mount_app_to_test_container();
        let html = container.inner_html();

        // Admin-visible sections should be rendered (though data may be empty until login)
        assert!(html.contains("Feature Flags") || html.contains("Taxonomy") || html.contains("Credentials") || html.contains("Templates"),
            "should render at least one admin section heading in the shell");

        cleanup_test_container();
    }
}
