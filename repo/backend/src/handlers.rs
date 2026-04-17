use std::{collections::{HashMap, HashSet}, path::PathBuf};

use axum::{
    body::Bytes,
    extract::{Extension, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::Row;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth,
    db,
    error::AppError,
    models::*,
    security,
    workflows,
};

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        mode: "offline-local",
        timestamp_utc: Utc::now().to_rfc3339(),
    })
}

pub async fn workspaces() -> Json<Vec<WorkspaceSummary>> {
    Json(vec![
        WorkspaceSummary {
            role_name: "Shopper".into(),
            capabilities: vec!["browse", "buy", "favorites", "recommendations", "after-sales"],
        },
        WorkspaceSummary {
            role_name: "Inventory Clerk".into(),
            capabilities: vec!["receiving", "issuing", "transfer", "return", "loan", "scrap"],
        },
        WorkspaceSummary {
            role_name: "Manager".into(),
            capabilities: vec!["approval", "exceptions", "dashboards"],
        },
        WorkspaceSummary {
            role_name: "Support Agent".into(),
            capabilities: vec!["after-sales", "evidence", "sla"],
        },
        WorkspaceSummary {
            role_name: "Administrator".into(),
            capabilities: vec!["operations", "flags", "credentials", "announcements", "templates"],
        },
    ])
}

pub async fn list_campuses(State(state): State<AppState>) -> Result<Json<Vec<ReferenceItem>>, AppError> {
    let rows = sqlx::query("SELECT id, name FROM campuses ORDER BY name")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| ReferenceItem {
                id: row.get("id"),
                label: row.get("name"),
            })
            .collect(),
    ))
}

pub async fn list_devices(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<ReferenceItem>>, AppError> {
    let _ = require_roles(current_user, &["Inventory Clerk", "Administrator", "Manager"])?;
    let rows = sqlx::query(
        "SELECT id, COALESCE(asset_tag, serial_number, id) as label
         FROM inventory_devices
         WHERE status = 'on_hand'
         ORDER BY created_at
         LIMIT 50",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| ReferenceItem {
                id: row.get("id"),
                label: row.get("label"),
            })
            .collect(),
    ))
}

pub async fn list_orders(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<OrderResponse>>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let rows = sqlx::query("SELECT id, status, total_cents FROM orders WHERE user_id = ? ORDER BY created_at DESC")
        .bind(&current_user.id)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| OrderResponse {
                order_id: row.get("id"),
                status: row.get("status"),
                total_cents: row.get("total_cents"),
            })
            .collect(),
    ))
}

pub async fn shipment_history(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(shipment_id): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let _ = require_roles(current_user, &["Inventory Clerk", "Administrator", "Support Agent", "Manager"])?;
    let rows = sqlx::query(
        "SELECT from_status, to_status, changed_at FROM shipment_status_history WHERE shipment_order_id = ? ORDER BY changed_at",
    )
    .bind(&shipment_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(|row| json!({
        "from_status": row.get::<Option<String>, _>("from_status"),
        "to_status": row.get::<String, _>("to_status"),
        "changed_at": row.get::<String, _>("changed_at"),
    })).collect()))
}

pub async fn after_sales_history(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(case_id): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let current_user = auth::require_user(current_user)?;
    ensure_after_sales_case_access(&state.pool, &current_user, &case_id).await?;
    let rows = sqlx::query(
        "SELECT from_status, to_status, changed_at FROM after_sales_status_history WHERE case_id = ? ORDER BY changed_at",
    )
    .bind(&case_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(|row| json!({
        "from_status": row.get::<Option<String>, _>("from_status"),
        "to_status": row.get::<String, _>("to_status"),
        "changed_at": row.get::<String, _>("changed_at"),
    })).collect()))
}

pub async fn register(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    security::validate_password_policy(&payload.password)?;
    if payload.role_name != "Shopper" {
        let actor = auth::require_user(current_user)?;
        if actor.role_name != "Administrator" {
            return Err(AppError::forbidden(
                "administrator access required to assign elevated roles",
            ));
        }
    }
    let role_id: Option<i64> = sqlx::query_scalar("SELECT id FROM roles WHERE name = ?")
        .bind(&payload.role_name)
        .fetch_optional(&state.pool)
        .await?;
    let role_id = role_id.ok_or_else(|| AppError::bad_request("unknown role name"))?;

    let user_id = Uuid::new_v4().to_string();
    let password_hash = security::hash_password(&payload.password)?;
    let display_name_enc = payload
        .display_name
        .as_deref()
        .map(|value| security::encrypt_field(&state.config.aes256_key_hex, value))
        .transpose()?;
    let phone_enc = payload
        .phone
        .as_deref()
        .map(|value| security::encrypt_field(&state.config.aes256_key_hex, value))
        .transpose()?;

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role_id, display_name_enc, phone_enc)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&user_id)
    .bind(&payload.username)
    .bind(password_hash)
    .bind(role_id)
    .bind(display_name_enc)
    .bind(phone_enc)
    .execute(&state.pool)
    .await?;

    sqlx::query("INSERT INTO user_settings (user_id, recommendations_enabled) VALUES (?, 1)")
        .bind(&user_id)
        .execute(&state.pool)
        .await?;

    Ok(Json(AuthResponse {
        user_id,
        username: payload.username,
        role_name: payload.role_name,
        display_name_masked: payload.display_name.map(|value| security::mask_value(&value)),
        phone_masked: payload.phone.map(|value| security::mask_value(&value)),
    }))
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let row = sqlx::query(
        "SELECT u.id, u.username, u.password_hash, u.display_name_enc, u.phone_enc,
                u.failed_login_attempts, u.locked_until, r.name as role_name
         FROM users u
         JOIN roles r ON r.id = u.role_id
         WHERE u.username = ?",
    )
    .bind(&payload.username)
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = row else {
        return Err(AppError::unauthorized("invalid credentials"));
    };

    let user_id: String = row.get("id");
    let password_hash: String = row.get("password_hash");
    let failed_attempts: i64 = row.get("failed_login_attempts");
    let locked_until: Option<String> = row.get("locked_until");

    if db::is_locked(locked_until.as_deref())? {
        return Err(AppError::locked("account is temporarily locked"));
    }

    if !security::verify_password(&password_hash, &payload.password)? {
        let next_attempts = failed_attempts + 1;
        let new_lock = if next_attempts >= state.config.login_max_failures {
            Some(db::lockout_until(state.config.login_lockout_minutes))
        } else {
            None
        };

        sqlx::query(
            "UPDATE users
             SET failed_login_attempts = ?, locked_until = ?, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
        )
        .bind(next_attempts)
        .bind(new_lock)
        .bind(&user_id)
        .execute(&state.pool)
        .await?;

        return Err(if next_attempts >= state.config.login_max_failures {
            AppError::locked("too many failed login attempts; account locked")
        } else {
            AppError::unauthorized("invalid credentials")
        });
    }

    sqlx::query(
        "UPDATE users
         SET failed_login_attempts = 0, locked_until = NULL, last_login_at = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(&user_id)
    .execute(&state.pool)
    .await?;

    sqlx::query("INSERT OR IGNORE INTO user_settings (user_id, recommendations_enabled) VALUES (?, 1)")
        .bind(&user_id)
        .execute(&state.pool)
        .await?;

    let session_token = security::random_token();
    let session_hash = security::sha256_hex(session_token.as_bytes());
    let ip_address = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    db::create_session(
        &state.pool,
        &user_id,
        &session_hash,
        ip_address,
        user_agent,
        state.config.session_idle_timeout_minutes,
    )
    .await?;

    let display_name_masked = row
        .get::<Option<String>, _>("display_name_enc")
        .map(|value| security::decrypt_field(&state.config.aes256_key_hex, &value))
        .transpose()?
        .map(|value| security::mask_value(&value));
    let phone_masked = row
        .get::<Option<String>, _>("phone_enc")
        .map(|value| security::decrypt_field(&state.config.aes256_key_hex, &value))
        .transpose()?
        .map(|value| security::mask_value(&value));

    let response = Json(AuthResponse {
        user_id,
        username: row.get("username"),
        role_name: row.get("role_name"),
        display_name_masked,
        phone_masked,
    });

    let cookie = format!(
        "{}={}; HttpOnly; Path=/; Max-Age={}; SameSite=Strict",
        auth::SESSION_COOKIE,
        session_token,
        state.config.session_idle_timeout_minutes * 60
    );

    Ok((
        [(
            header::SET_COOKIE,
            HeaderValue::from_str(&cookie)
                .map_err(|_| AppError::internal("failed to build session cookie"))?,
        )],
        response,
    ))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if let Some(token) = auth::extract_cookie_token(&headers) {
        let hash = security::sha256_hex(token.as_bytes());
        db::delete_session_by_hash(&state.pool, &hash).await?;
    }

    Ok((
        StatusCode::OK,
        [(
            header::SET_COOKIE,
            HeaderValue::from_static("depotcycle_session=deleted; HttpOnly; Path=/; Max-Age=0; SameSite=Strict"),
        )],
        Json(json!({"status": "logged_out"})),
    ))
}

pub async fn me(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<AuthResponse>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let row = sqlx::query("SELECT display_name_enc, phone_enc FROM users WHERE id = ?")
        .bind(&current_user.id)
        .fetch_one(&state.pool)
        .await?;

    let display_name_masked = row
        .get::<Option<String>, _>("display_name_enc")
        .map(|value| security::decrypt_field(&state.config.aes256_key_hex, &value))
        .transpose()?
        .map(|value| security::mask_value(&value));
    let phone_masked = row
        .get::<Option<String>, _>("phone_enc")
        .map(|value| security::decrypt_field(&state.config.aes256_key_hex, &value))
        .transpose()?
        .map(|value| security::mask_value(&value));

    Ok(Json(AuthResponse {
        user_id: current_user.id,
        username: current_user.username,
        role_name: current_user.role_name,
        display_name_masked,
        phone_masked,
    }))
}

pub async fn search_listings(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<ListingCard>>, AppError> {
    let campus_coords = load_campus_coords(&state.pool).await?;
    let rows = sqlx::query_as::<_, ListingCard>(
        "SELECT l.id, l.title, l.description, l.price_cents, l.status, l.created_at,
                c.name as campus_name, c.zip_code as campus_zip_code,
                lc.code as condition_code, t.slug as category_slug
         FROM listings l
         LEFT JOIN campuses c ON c.id = l.campus_id
         LEFT JOIN listing_conditions lc ON lc.id = l.condition_id
         LEFT JOIN taxonomy_nodes t ON t.id = l.taxonomy_node_id
         WHERE l.status = 'published'",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut items: Vec<(ListingCard, i64, f64)> = rows
        .into_iter()
        .filter_map(|card| {
            if let Some(ref category) = query.category {
                if card.category_slug.as_deref() != Some(category.as_str()) {
                    return None;
                }
            }
            if let Some(min_price) = query.min_price {
                if card.price_cents < min_price * 100 {
                    return None;
                }
            }
            if let Some(max_price) = query.max_price {
                if card.price_cents > max_price * 100 {
                    return None;
                }
            }
            if let Some(ref condition) = query.condition {
                if card.condition_code.as_deref() != Some(condition.as_str()) {
                    return None;
                }
            }
            if let Some(ref campus) = query.campus {
                if card.campus_name.as_deref() != Some(campus.as_str()) {
                    return None;
                }
            }
            if let Some(days) = query.post_time_days {
                let created_at = chrono::DateTime::parse_from_rfc3339(&card.created_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .or_else(|_| {
                        chrono::NaiveDateTime::parse_from_str(&card.created_at, "%Y-%m-%d %H:%M:%S")
                            .map(|dt| dt.and_utc())
                    })
                    .ok()?;
                if created_at < Utc::now() - Duration::days(days) {
                    return None;
                }
            }
            if let Some(ref q) = query.q {
                let combined = format!(
                    "{} {} {}",
                    card.title,
                    card.description.clone().unwrap_or_default(),
                    card.category_slug.clone().unwrap_or_default()
                )
                .to_lowercase();
                if !combined.contains(&q.to_lowercase()) {
                    return None;
                }
            }
            let relevance = query
                .q
                .as_ref()
                .map(|q| relevance_score(&card, q))
                .unwrap_or(0);
            let distance =
                approximate_distance_score(&campus_coords, &card, query.zip_code.as_deref())
                    .unwrap_or(0.0);
            Some((card, relevance, distance))
        })
        .collect();

    let popularity = listing_popularity_map(&state.pool).await?;
    match query.sort.as_deref().unwrap_or("relevance") {
        "price" => items.sort_by_key(|(card, _, _)| card.price_cents),
        "popularity" => items.sort_by_key(|(card, _, _)| -popularity.get(&card.id).copied().unwrap_or(0)),
        "distance" => items.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)),
        _ => items.sort_by_key(|(card, score, _)| -(score + popularity.get(&card.id).copied().unwrap_or(0))),
    }

    if let Some(user) = current_user {
        if let Some(ref q) = query.q {
            let filters = serde_json::to_value(&query).unwrap_or(json!({}));
            record_search_history(&state.pool, &user.id, q, Some(filters)).await?;
        }
    }

    Ok(Json(items.into_iter().map(|(card, _, _)| card).collect()))
}

pub async fn search_suggestions(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<SuggestionResponse>, AppError> {
    let q = query.get("q").cloned().unwrap_or_default().to_lowercase();
    let mut suggestions = Vec::new();

    for row in sqlx::query("SELECT title FROM listings WHERE status = 'published' LIMIT 20")
        .fetch_all(&state.pool)
        .await?
    {
        let title: String = row.get("title");
        if q.is_empty() || title.to_lowercase().contains(&q) {
            suggestions.push(title);
        }
    }
    for row in sqlx::query("SELECT keyword FROM taxonomy_keywords LIMIT 20")
        .fetch_all(&state.pool)
        .await?
    {
        let keyword: String = row.get("keyword");
        if q.is_empty() || keyword.to_lowercase().contains(&q) {
            suggestions.push(keyword);
        }
    }
    suggestions.sort();
    suggestions.dedup();
    suggestions.truncate(8);
    Ok(Json(SuggestionResponse { suggestions }))
}

pub async fn list_search_history(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<SearchHistoryItem>>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let rows = sqlx::query(
        "SELECT id, query_text, created_at
         FROM search_history
         WHERE user_id = ?
         ORDER BY created_at DESC
         LIMIT 10",
    )
    .bind(&current_user.id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|row| SearchHistoryItem {
                id: row.get("id"),
                query_text: row.get("query_text"),
                created_at: row.get("created_at"),
            })
            .collect(),
    ))
}

pub async fn create_search_history(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<SearchHistoryRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let current_user = auth::require_user(current_user)?;
    record_search_history(&state.pool, &current_user.id, &payload.query_text, payload.filters_json).await?;
    Ok(Json(json!({"status": "stored"})))
}

pub async fn clear_search_history(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let current_user = auth::require_user(current_user)?;
    sqlx::query("DELETE FROM search_history WHERE user_id = ?")
        .bind(&current_user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({"status": "cleared"})))
}

pub async fn get_listing_detail(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(id): Path<String>,
) -> Result<Json<ListingDetail>, AppError> {
    let listing = sqlx::query_as::<_, ListingCard>(
        "SELECT l.id, l.title, l.description, l.price_cents, l.status, l.created_at,
                c.name as campus_name, c.zip_code as campus_zip_code,
                lc.code as condition_code, t.slug as category_slug
         FROM listings l
         LEFT JOIN campuses c ON c.id = l.campus_id
         LEFT JOIN listing_conditions lc ON lc.id = l.condition_id
         LEFT JOIN taxonomy_nodes t ON t.id = l.taxonomy_node_id
         WHERE l.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("listing not found"))?;

    let popularity_map = listing_popularity_map(&state.pool).await?;
    let inventory_on_hand: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM inventory_devices
         WHERE listing_id = ? AND status = 'on_hand'",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    let recommendations = if let Some(user) = current_user {
        fetch_recommendations(&state.pool, &user.id, Some(&id)).await?
    } else {
        Vec::new()
    };

    Ok(Json(ListingDetail {
        listing,
        popularity_score: popularity_map.get(&id).copied().unwrap_or(0),
        inventory_on_hand,
        recommendations,
    }))
}

pub async fn record_listing_view(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let current_user = auth::require_user(current_user)?;
    log_event(
        &state.pool,
        "listing_view",
        Some(&current_user.id),
        current_user.session_id.as_deref(),
        json!({"listing_id": id}),
    )
    .await?;
    Ok(Json(json!({"status": "recorded"})))
}

pub async fn toggle_favorite(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM favorites WHERE user_id = ? AND listing_id = ?",
    )
    .bind(&current_user.id)
    .bind(&id)
    .fetch_optional(&state.pool)
    .await?;

    let favorited = if let Some(favorite_id) = existing {
        sqlx::query("DELETE FROM favorites WHERE id = ?")
            .bind(favorite_id)
            .execute(&state.pool)
            .await?;
        false
    } else {
        sqlx::query("INSERT INTO favorites (id, user_id, listing_id) VALUES (?, ?, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(&current_user.id)
            .bind(&id)
            .execute(&state.pool)
            .await?;
        log_event(
            &state.pool,
            "favorite_added",
            Some(&current_user.id),
            current_user.session_id.as_deref(),
            json!({"listing_id": id}),
        )
        .await?;
        true
    };

    Ok(Json(json!({ "favorited": favorited })))
}

pub async fn recommendations(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<RecommendationCard>>, AppError> {
    let current_user = auth::require_user(current_user)?;
    Ok(Json(fetch_recommendations(&state.pool, &current_user.id, None).await?))
}

pub async fn get_recommendation_settings(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<RecommendationSettingsResponse>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let enabled: i64 = sqlx::query_scalar(
        "SELECT recommendations_enabled FROM user_settings WHERE user_id = ?",
    )
    .bind(&current_user.id)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(1);
    Ok(Json(RecommendationSettingsResponse {
        recommendations_enabled: enabled == 1,
    }))
}

pub async fn update_recommendation_settings(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<RecommendationSettingsRequest>,
) -> Result<Json<RecommendationSettingsResponse>, AppError> {
    let current_user = auth::require_user(current_user)?;
    sqlx::query(
        "INSERT INTO user_settings (user_id, recommendations_enabled, updated_at)
         VALUES (?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(user_id) DO UPDATE SET recommendations_enabled = excluded.recommendations_enabled, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&current_user.id)
    .bind(if payload.recommendations_enabled { 1 } else { 0 })
    .execute(&state.pool)
    .await?;

    Ok(Json(RecommendationSettingsResponse {
        recommendations_enabled: payload.recommendations_enabled,
    }))
}

pub async fn create_order(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateOrderRequest>,
) -> Result<Json<OrderResponse>, AppError> {
    let current_user = auth::require_user(current_user)?;
    if payload.quantity <= 0 {
        return Err(AppError::bad_request("quantity must be greater than zero"));
    }

    let mut tx = state.pool.begin().await?;
    let listing_row = sqlx::query("SELECT price_cents FROM listings WHERE id = ?")
        .bind(&payload.listing_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::not_found("listing not found"))?;

    let available_devices = sqlx::query(
        "SELECT id FROM inventory_devices WHERE listing_id = ? AND status = 'on_hand' ORDER BY id LIMIT ?",
    )
    .bind(&payload.listing_id)
    .bind(payload.quantity)
    .fetch_all(&mut *tx)
    .await?;

    if available_devices.len() < payload.quantity as usize {
        return Err(AppError::bad_request("not enough on-hand inventory"));
    }

    let price_cents: i64 = listing_row.get("price_cents");
    let total_cents = price_cents * payload.quantity;
    let order_id = Uuid::new_v4().to_string();

    sqlx::query("INSERT INTO orders (id, user_id, status, total_cents) VALUES (?, ?, 'placed', ?)")
        .bind(&order_id)
        .bind(&current_user.id)
        .bind(total_cents)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "INSERT INTO order_items (id, order_id, listing_id, quantity, unit_price_cents)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&order_id)
    .bind(&payload.listing_id)
    .bind(payload.quantity)
    .bind(price_cents)
    .execute(&mut *tx)
    .await?;

    let mut sold_count = 0_i64;
    for row in available_devices {
        let device_id: String = row.get("id");
        let result = sqlx::query(
            "UPDATE inventory_devices
             SET status = 'sold', updated_at = CURRENT_TIMESTAMP
             WHERE id = ? AND status = 'on_hand'",
        )
            .bind(&device_id)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() != 1 {
            return Err(AppError::bad_request("inventory changed during checkout; please retry"));
        }
        sold_count += 1;
    }
    if sold_count != payload.quantity {
        return Err(AppError::bad_request("inventory changed during checkout; please retry"));
    }
    tx.commit().await?;

    log_event(
        &state.pool,
        "order_created",
        Some(&current_user.id),
        current_user.session_id.as_deref(),
        json!({"listing_id": payload.listing_id, "quantity": payload.quantity}),
    )
    .await?;

    Ok(Json(OrderResponse {
        order_id,
        status: "placed".into(),
        total_cents,
    }))
}

pub async fn list_taxonomy(
    State(state): State<AppState>,
) -> Result<Json<Vec<TaxonomyNodeRecord>>, AppError> {
    let rows = sqlx::query_as::<_, TaxonomyNodeRecord>(
        "SELECT id, parent_id, name, slug, level, seo_title, seo_description, seo_keywords, topic_page_path
         FROM taxonomy_nodes ORDER BY level, name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create_taxonomy(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateTaxonomyNodeRequest>,
) -> Result<Json<TaxonomyNodeRecord>, AppError> {
    let current_user = require_roles(current_user, &["Administrator", "Manager"])?;
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO taxonomy_nodes (id, parent_id, name, slug, level, seo_title, seo_description, seo_keywords, topic_page_path)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&payload.parent_id)
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(payload.level)
    .bind(&payload.seo_title)
    .bind(&payload.seo_description)
    .bind(&payload.seo_keywords)
    .bind(&payload.topic_page_path)
    .execute(&state.pool)
    .await?;

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "create_taxonomy_node",
        "taxonomy_nodes",
        &id,
        json!({"slug": payload.slug}),
    )
    .await?;

    let record = sqlx::query_as::<_, TaxonomyNodeRecord>(
        "SELECT id, parent_id, name, slug, level, seo_title, seo_description, seo_keywords, topic_page_path
         FROM taxonomy_nodes WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(record))
}

pub async fn create_upload_session(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateUploadSessionRequest>,
) -> Result<Json<UploadSessionResponse>, AppError> {
    let current_user = require_roles(
        current_user,
        &["Administrator", "Inventory Clerk", "Support Agent", "Manager", "Shopper"],
    )?;
    validate_mime(&payload.mime_type)?;
    let session_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO media_upload_sessions (id, created_by, file_name, mime_type, total_chunks, target_listing_id, expected_sha256)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(&current_user.id)
    .bind(&payload.file_name)
    .bind(&payload.mime_type)
    .bind(payload.total_chunks)
    .bind(&payload.listing_id)
    .bind(&payload.expected_sha256)
    .execute(&state.pool)
    .await?;

    Ok(Json(UploadSessionResponse {
        session_id,
        uploaded_chunks: 0,
        total_chunks: payload.total_chunks,
        status: "created".into(),
    }))
}

pub async fn upload_chunk(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path((session_id, chunk_index)): Path<(String, i64)>,
    body: Bytes,
) -> Result<Json<UploadSessionResponse>, AppError> {
    let current_user = require_roles(
        current_user,
        &["Administrator", "Inventory Clerk", "Support Agent", "Manager", "Shopper"],
    )?;

    let session = sqlx::query(
        "SELECT total_chunks, created_by FROM media_upload_sessions WHERE id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("upload session not found"))?;
    let session_owner: String = session.get("created_by");
    ensure_upload_session_access(&current_user, &session_owner)?;
    let total_chunks: i64 = session.get("total_chunks");
    if chunk_index >= total_chunks {
        return Err(AppError::bad_request("chunk index out of range"));
    }

    let chunk_dir = PathBuf::from(&state.config.upload_dir).join("chunks").join(&session_id);
    tokio::fs::create_dir_all(&chunk_dir).await?;
    let chunk_path = chunk_dir.join(format!("{chunk_index}.part"));
    let mut file = tokio::fs::File::create(&chunk_path).await?;
    file.write_all(&body).await?;
    file.flush().await?;
    let sha256 = security::sha256_hex(&body);

    sqlx::query(
        "INSERT INTO media_upload_chunks (id, session_id, chunk_index, chunk_path, sha256, size_bytes)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(session_id, chunk_index) DO UPDATE SET
           chunk_path = excluded.chunk_path, sha256 = excluded.sha256, size_bytes = excluded.size_bytes",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&session_id)
    .bind(chunk_index)
    .bind(chunk_path.to_string_lossy().to_string())
    .bind(&sha256)
    .bind(body.len() as i64)
    .execute(&state.pool)
    .await?;

    let uploaded_chunks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM media_upload_chunks WHERE session_id = ?",
    )
    .bind(&session_id)
    .fetch_one(&state.pool)
    .await?;

    sqlx::query(
        "UPDATE media_upload_sessions
         SET uploaded_chunks = ?, status = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(uploaded_chunks)
    .bind(if uploaded_chunks == total_chunks { "ready" } else { "uploading" })
    .bind(&session_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(UploadSessionResponse {
        session_id,
        uploaded_chunks,
        total_chunks,
        status: if uploaded_chunks == total_chunks {
            "ready".into()
        } else {
            "uploading".into()
        },
    }))
}

pub async fn finalize_upload(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(session_id): Path<String>,
    Json(payload): Json<FinalizeUploadRequest>,
) -> Result<Json<UploadResponse>, AppError> {
    let current_user = require_roles(
        current_user,
        &["Administrator", "Inventory Clerk", "Support Agent", "Manager", "Shopper"],
    )?;
    let session = sqlx::query(
        "SELECT file_name, mime_type, total_chunks, uploaded_chunks, target_listing_id, expected_sha256, created_by, status
         FROM media_upload_sessions WHERE id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("upload session not found"))?;
    let session_owner: String = session.get("created_by");
    ensure_upload_session_access(&current_user, &session_owner)?;

    let status: String = session.get("status");
    if status == "completed" {
        return Err(AppError::bad_request("upload session already finalized"));
    }

    let total_chunks: i64 = session.get("total_chunks");
    let uploaded_chunks: i64 = session.get("uploaded_chunks");
    if total_chunks != uploaded_chunks {
        return Err(AppError::bad_request("upload is incomplete"));
    }

    let rows = sqlx::query(
        "SELECT chunk_path FROM media_upload_chunks WHERE session_id = ? ORDER BY chunk_index",
    )
    .bind(&session_id)
    .fetch_all(&state.pool)
    .await?;

    let assembled_dir = PathBuf::from(&state.config.upload_dir).join("assembled");
    tokio::fs::create_dir_all(&assembled_dir).await?;
    let assembled_path = assembled_dir.join(format!("{session_id}.bin"));
    let mut output = tokio::fs::File::create(&assembled_path).await?;
    let mut merged = Vec::new();
    for row in rows {
        let chunk_path: String = row.get("chunk_path");
        let bytes = tokio::fs::read(&chunk_path).await?;
        output.write_all(&bytes).await?;
        merged.extend_from_slice(&bytes);
    }
    output.flush().await?;

    let sha256 = security::sha256_hex(&merged);
    let expected = payload
        .expected_sha256
        .or_else(|| session.get::<Option<String>, _>("expected_sha256"));
    if let Some(expected) = expected {
        if sha256 != expected {
            return Err(AppError::bad_request("checksum validation failed"));
        }
    }

    let media_id = Uuid::new_v4().to_string();
    let playback_token = security::random_token();
    let mime_type: String = session.get("mime_type");
    let listing_id: Option<String> = session.get("target_listing_id");

    sqlx::query(
        "INSERT INTO listing_media (id, listing_id, storage_path, mime_type, sha256, size_bytes, media_kind, chunk_group, playback_token)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&media_id)
    .bind(listing_id)
    .bind(assembled_path.to_string_lossy().to_string())
    .bind(&mime_type)
    .bind(&sha256)
    .bind(merged.len() as i64)
    .bind(media_kind(&mime_type))
    .bind(&session_id)
    .bind(&playback_token)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "UPDATE media_upload_sessions
         SET status = 'completed', assembled_path = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(assembled_path.to_string_lossy().to_string())
    .bind(&session_id)
    .execute(&state.pool)
    .await?;

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "finalize_upload",
        "listing_media",
        &media_id,
        json!({"sha256": sha256}),
    )
    .await?;

    Ok(Json(UploadResponse {
        media_id,
        mime_type,
        sha256,
        storage_path: assembled_path.to_string_lossy().to_string(),
        playback_token,
    }))
}

pub async fn playback_link(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(media_id): Path<String>,
) -> Result<Json<PlaybackLinkResponse>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let media_row = sqlx::query("SELECT id, listing_id FROM listing_media WHERE id = ?")
        .bind(&media_id)
        .fetch_optional(&state.pool)
        .await?;
    let media_row = media_row.ok_or_else(|| AppError::not_found("media not found"))?;

    // Support staff can access any media
    if !is_support_staff(&current_user.role_name) {
        let listing_id: Option<String> = media_row.get("listing_id");
        let mut authorized = false;

        // Check if user owns the listing this media belongs to
        if let Some(ref lid) = listing_id {
            let owner: Option<String> = sqlx::query_scalar(
                "SELECT seller_user_id FROM listings WHERE id = ?",
            )
            .bind(lid)
            .fetch_optional(&state.pool)
            .await?;
            if owner.as_deref() == Some(&current_user.id) {
                authorized = true;
            }
        }

        // Check if user is involved in an after-sales case that has this media as evidence
        if !authorized {
            let case_access: Option<String> = sqlx::query_scalar(
                "SELECT ase.id FROM after_sales_evidence ase
                 JOIN after_sales_cases asc2 ON asc2.id = ase.case_id
                 WHERE ase.media_id = ? AND (asc2.opened_by_user_id = ? OR ase.uploaded_by = ?)",
            )
            .bind(&media_id)
            .bind(&current_user.id)
            .bind(&current_user.id)
            .fetch_optional(&state.pool)
            .await?;
            if case_access.is_some() {
                authorized = true;
            }
        }

        if !authorized {
            return Err(AppError::forbidden("you do not have access to this media"));
        }
    }

    let token = security::random_token();
    let expires_at = (Utc::now() + Duration::minutes(15)).to_rfc3339();
    sqlx::query(
        "INSERT INTO media_playback_tokens (id, media_id, token, issued_to_user_id, expires_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&media_id)
    .bind(&token)
    .bind(&current_user.id)
    .bind(&expires_at)
    .execute(&state.pool)
    .await?;

    Ok(Json(PlaybackLinkResponse {
        token: token.clone(),
        stream_url: format!("{}/api/v1/media/stream/{token}", state.config.public_api_base_url),
        expires_at,
    }))
}

pub async fn stream_media(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let current_user = auth::require_user(current_user)?;
    let row = sqlx::query(
        "SELECT lm.storage_path, lm.mime_type, mpt.expires_at, mpt.issued_to_user_id
         FROM media_playback_tokens mpt
         JOIN listing_media lm ON lm.id = mpt.media_id
         WHERE mpt.token = ?",
    )
    .bind(&token)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("playback token not found"))?;

    let expires_at: String = row.get("expires_at");
    let expiry = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|_| AppError::internal("invalid playback expiry"))?
        .with_timezone(&Utc);
    if expiry < Utc::now() {
        return Err(AppError::forbidden("playback token expired"));
    }
    let issued_to_user_id: String = row.get("issued_to_user_id");
    if issued_to_user_id != current_user.id {
        return Err(AppError::forbidden("playback token is not valid for this user"));
    }

    let storage_path: String = row.get("storage_path");
    let mime_type: String = row.get("mime_type");
    let bytes = tokio::fs::read(storage_path).await?;

    Ok((
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&mime_type)
                .map_err(|_| AppError::internal("invalid media mime type"))?,
        )],
        Bytes::from(bytes),
    ))
}

pub async fn create_inventory_document(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateInventoryDocumentRequest>,
) -> Result<Json<InventoryDocumentResponse>, AppError> {
    let current_user = require_roles(current_user, &["Inventory Clerk", "Administrator"])?;
    if payload.lines.is_empty() {
        return Err(AppError::bad_request("at least one line item is required"));
    }
    let mut seen_device_ids = HashSet::new();
    for line in &payload.lines {
        if line.quantity != 1 {
            return Err(AppError::bad_request(
                "each inventory line must reference exactly one physical device",
            ));
        }
        if !seen_device_ids.insert(line.device_id.clone()) {
            return Err(AppError::bad_request(
                "duplicate device_id in inventory document lines",
            ));
        }
        let exists: Option<String> =
            sqlx::query_scalar("SELECT id FROM inventory_devices WHERE id = ?")
                .bind(&line.device_id)
                .fetch_optional(&state.pool)
                .await?;
        if exists.is_none() {
            return Err(AppError::not_found("inventory device not found"));
        }
    }

    let document_id = Uuid::new_v4().to_string();
    let total_value_cents: i64 = payload
        .lines
        .iter()
        .map(|line| line.quantity * line.unit_value_cents)
        .sum();
    let scrap_units: i64 = if payload.doc_type == "scrap" {
        payload.lines.iter().map(|line| line.quantity).sum()
    } else {
        0
    };
    let needs_approval = workflows::requires_manager_approval(total_value_cents, scrap_units);
    let workflow_status = if needs_approval { "pending_approval" } else { "approved" };

    sqlx::query(
        "INSERT INTO inventory_documents (id, doc_type, reference_no, source_campus_id, target_campus_id, notes, workflow_status)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&document_id)
    .bind(&payload.doc_type)
    .bind(&payload.reference_no)
    .bind(&payload.source_campus_id)
    .bind(&payload.target_campus_id)
    .bind(&payload.notes)
    .bind(workflow_status)
    .execute(&state.pool)
    .await?;

    for line in &payload.lines {
        sqlx::query(
            "INSERT INTO inventory_document_lines (id, document_id, device_id, quantity, unit_value_cents, target_campus_id, notes)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&document_id)
        .bind(&line.device_id)
        .bind(line.quantity)
        .bind(line.unit_value_cents)
        .bind(&line.target_campus_id)
        .bind(&line.notes)
        .execute(&state.pool)
        .await?;
    }

    if needs_approval {
        sqlx::query(
            "INSERT INTO approval_requests (id, document_id, status, reason, requested_by)
             VALUES (?, ?, 'pending', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&document_id)
        .bind(format!(
            "Document exceeds threshold: value=${:.2}, scrap_units={}",
            total_value_cents as f64 / 100.0,
            scrap_units
        ))
        .bind(&current_user.id)
        .execute(&state.pool)
        .await?;
    } else {
        execute_inventory_document(&state.pool, &current_user.id, &document_id).await?;
    }

    Ok(Json(InventoryDocumentResponse {
        document_id,
        status: workflow_status.into(),
        requires_manager_approval: needs_approval,
    }))
}

pub async fn list_inventory_documents(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<InventoryDocumentRecord>>, AppError> {
    let _ = require_roles(current_user, &["Inventory Clerk", "Administrator", "Manager"])?;
    let rows = sqlx::query_as::<_, InventoryDocumentRecord>(
        "SELECT id, doc_type, reference_no, workflow_status, notes, created_at
         FROM inventory_documents ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn approve_inventory_document(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(document_id): Path<String>,
) -> Result<Json<InventoryDocumentResponse>, AppError> {
    let current_user = require_roles(current_user, &["Manager", "Administrator"])?;
    let row = sqlx::query(
        "SELECT workflow_status FROM inventory_documents WHERE id = ?",
    )
    .bind(&document_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("document not found"))?;
    let status: String = row.get("workflow_status");
    if status != "pending_approval" {
        return Err(AppError::bad_request("document is not pending approval"));
    }

    sqlx::query(
        "UPDATE approval_requests
         SET status = 'approved', approved_by = ?, approved_at = CURRENT_TIMESTAMP
         WHERE document_id = ?",
    )
    .bind(&current_user.id)
    .bind(&document_id)
    .execute(&state.pool)
    .await?;
    sqlx::query("UPDATE inventory_documents SET workflow_status = 'approved' WHERE id = ?")
        .bind(&document_id)
        .execute(&state.pool)
        .await?;

    execute_inventory_document(&state.pool, &current_user.id, &document_id).await?;

    Ok(Json(InventoryDocumentResponse {
        document_id,
        status: "executed".into(),
        requires_manager_approval: true,
    }))
}

pub async fn execute_inventory_document_endpoint(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(document_id): Path<String>,
) -> Result<Json<InventoryDocumentResponse>, AppError> {
    let current_user = require_roles(current_user, &["Inventory Clerk", "Administrator"])?;
    let status: Option<String> =
        sqlx::query_scalar("SELECT workflow_status FROM inventory_documents WHERE id = ?")
            .bind(&document_id)
            .fetch_optional(&state.pool)
            .await?;
    let status = status.ok_or_else(|| AppError::not_found("document not found"))?;
    if status == "pending_approval" {
        return Err(AppError::bad_request("document requires manager approval"));
    }
    if status == "executed" {
        return Ok(Json(InventoryDocumentResponse {
            document_id,
            status,
            requires_manager_approval: false,
        }));
    }

    execute_inventory_document(&state.pool, &current_user.id, &document_id).await?;
    Ok(Json(InventoryDocumentResponse {
        document_id,
        status: "executed".into(),
        requires_manager_approval: false,
    }))
}

pub async fn create_shipment(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateShipmentRequest>,
) -> Result<Json<ShipmentRecord>, AppError> {
    let current_user =
        require_roles(current_user, &["Inventory Clerk", "Administrator", "Support Agent"])?;
    let shipment_id = Uuid::new_v4().to_string();
    let order_number = format!("SHIP-{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO shipment_orders (id, order_number, listing_id, device_id, from_campus_id, to_campus_id, status, carrier_name, tracking_number, integration_enabled)
         VALUES (?, ?, ?, ?, ?, ?, 'created', ?, ?, 0)",
    )
    .bind(&shipment_id)
    .bind(&order_number)
    .bind(&payload.listing_id)
    .bind(&payload.device_id)
    .bind(&payload.from_campus_id)
    .bind(&payload.to_campus_id)
    .bind(&payload.carrier_name)
    .bind(&payload.tracking_number)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "INSERT INTO shipment_status_history (id, shipment_order_id, from_status, to_status, changed_by)
         VALUES (?, ?, NULL, 'created', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&shipment_id)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;

    fetch_shipment(&state.pool, &shipment_id).await.map(Json)
}

pub async fn list_shipments(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<ShipmentRecord>>, AppError> {
    let _ = require_roles(current_user, &["Inventory Clerk", "Administrator", "Support Agent", "Manager"])?;
    let rows = sqlx::query_as::<_, ShipmentRecord>(
        "SELECT id, order_number, status, carrier_name, tracking_number, integration_enabled, created_at
         FROM shipment_orders ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn transition_shipment(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(shipment_id): Path<String>,
    Json(payload): Json<TransitionRequest>,
) -> Result<Json<ShipmentRecord>, AppError> {
    let current_user = require_roles(current_user, &["Inventory Clerk", "Administrator", "Support Agent", "Manager"])?;
    let current_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM shipment_orders WHERE id = ?")
            .bind(&shipment_id)
            .fetch_optional(&state.pool)
            .await?;
    let current_status =
        current_status.ok_or_else(|| AppError::not_found("shipment not found"))?;
    if !workflows::valid_shipment_transition(&current_status, &payload.next_status) {
        return Err(AppError::bad_request("invalid shipment status transition"));
    }

    sqlx::query("UPDATE shipment_orders SET status = ? WHERE id = ?")
        .bind(&payload.next_status)
        .bind(&shipment_id)
        .execute(&state.pool)
        .await?;
    sqlx::query(
        "INSERT INTO shipment_status_history (id, shipment_order_id, from_status, to_status, changed_by)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&shipment_id)
    .bind(current_status)
    .bind(&payload.next_status)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;

    fetch_shipment(&state.pool, &shipment_id).await.map(Json)
}

pub async fn create_after_sales_case(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateAfterSalesCaseRequest>,
) -> Result<Json<AfterSalesCaseRecord>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let case_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let first_due = workflows::add_business_days(now, 1).to_rfc3339();
    let final_due = workflows::add_business_days(now, 3).to_rfc3339();

    sqlx::query(
        "INSERT INTO after_sales_cases (id, order_id, case_type, status, opened_by_user_id, reason, first_response_due_at, final_decision_due_at)
         VALUES (?, ?, ?, 'requested', ?, ?, ?, ?)",
    )
    .bind(&case_id)
    .bind(&payload.order_id)
    .bind(&payload.case_type)
    .bind(&current_user.id)
    .bind(&payload.reason)
    .bind(&first_due)
    .bind(&final_due)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "INSERT INTO after_sales_status_history (id, case_id, from_status, to_status, changed_by)
         VALUES (?, ?, NULL, 'requested', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&case_id)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;

    fetch_after_sales_case(&state.pool, &case_id).await.map(Json)
}

pub async fn list_after_sales_cases(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<AfterSalesCaseRecord>>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let rows = if matches!(current_user.role_name.as_str(), "Support Agent" | "Administrator" | "Manager") {
        sqlx::query_as::<_, AfterSalesCaseRecord>(
            "SELECT id, case_type, status, reason, first_response_due_at, final_decision_due_at, created_at
             FROM after_sales_cases ORDER BY created_at DESC",
        )
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, AfterSalesCaseRecord>(
            "SELECT id, case_type, status, reason, first_response_due_at, final_decision_due_at, created_at
             FROM after_sales_cases WHERE opened_by_user_id = ? ORDER BY created_at DESC",
        )
        .bind(&current_user.id)
        .fetch_all(&state.pool)
        .await?
    };
    Ok(Json(rows))
}

pub async fn transition_after_sales_case(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(case_id): Path<String>,
    Json(payload): Json<TransitionRequest>,
) -> Result<Json<AfterSalesCaseRecord>, AppError> {
    let current_user = require_roles(current_user, &["Support Agent", "Administrator", "Manager"])?;
    let current_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM after_sales_cases WHERE id = ?")
            .bind(&case_id)
            .fetch_optional(&state.pool)
            .await?;
    let current_status = current_status.ok_or_else(|| AppError::not_found("case not found"))?;
    if !workflows::valid_after_sales_transition(&current_status, &payload.next_status) {
        return Err(AppError::bad_request("invalid after-sales status transition"));
    }

    sqlx::query(
        "UPDATE after_sales_cases
         SET status = ?, assigned_to_user_id = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(&payload.next_status)
    .bind(&current_user.id)
    .bind(&case_id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "INSERT INTO after_sales_status_history (id, case_id, from_status, to_status, changed_by)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&case_id)
    .bind(current_status)
    .bind(&payload.next_status)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;

    fetch_after_sales_case(&state.pool, &case_id).await.map(Json)
}

pub async fn attach_after_sales_evidence(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(case_id): Path<String>,
    Json(payload): Json<AttachEvidenceRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let current_user = auth::require_user(current_user)?;
    ensure_after_sales_case_access(&state.pool, &current_user, &case_id).await?;
    ensure_media_attach_access(&state.pool, &current_user, &payload.media_id).await?;
    sqlx::query(
        "INSERT INTO after_sales_evidence (id, case_id, media_id, uploaded_by)
         VALUES (?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&case_id)
    .bind(&payload.media_id)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({"status": "attached"})))
}

pub async fn upload_after_sales_evidence(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(case_id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, AppError> {
    let current_user = auth::require_user(current_user)?;
    ensure_after_sales_case_access(&state.pool, &current_user, &case_id).await?;
    tokio::fs::create_dir_all(&state.config.upload_dir).await?;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("invalid multipart payload"))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let mime_type = field
            .content_type()
            .map(str::to_string)
            .ok_or_else(|| AppError::bad_request("content type is required"))?;
        validate_mime(&mime_type)?;

        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::bad_request("failed to read upload"))?;
        if bytes.len() > state.config.max_upload_size_bytes {
            return Err(AppError::bad_request("file exceeds maximum allowed upload size"));
        }
        let sha256 = security::sha256_hex(&bytes);
        let media_id = Uuid::new_v4().to_string();
        let playback_token = security::random_token();
        let file_name = format!("{media_id}.bin");
        let mut storage_path = PathBuf::from(&state.config.upload_dir);
        storage_path.push(file_name);

        let mut file = tokio::fs::File::create(&storage_path).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;

        sqlx::query(
            "INSERT INTO listing_media (id, listing_id, storage_path, mime_type, sha256, size_bytes, media_kind, playback_token)
             VALUES (?, NULL, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&media_id)
        .bind(storage_path.to_string_lossy().to_string())
        .bind(&mime_type)
        .bind(&sha256)
        .bind(bytes.len() as i64)
        .bind(media_kind(&mime_type))
        .bind(&playback_token)
        .execute(&state.pool)
        .await?;

        sqlx::query(
            "INSERT INTO after_sales_evidence (id, case_id, media_id, uploaded_by)
             VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&case_id)
        .bind(&media_id)
        .bind(&current_user.id)
        .execute(&state.pool)
        .await?;

        return Ok(Json(UploadResponse {
            media_id,
            mime_type,
            sha256,
            storage_path: storage_path.to_string_lossy().to_string(),
            playback_token,
        }));
    }

    Err(AppError::bad_request("multipart field `file` is required"))
}

pub async fn list_feature_flags(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<FeatureFlagRecord>>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Manager"])?;
    let rows = sqlx::query_as::<_, FeatureFlagRecord>(
        "SELECT id, key, description, enabled, rollout_percent, audience_rules_json
         FROM feature_flags ORDER BY key",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn update_feature_flag(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(flag_id): Path<String>,
    Json(payload): Json<UpdateFeatureFlagRequest>,
) -> Result<Json<FeatureFlagRecord>, AppError> {
    let current_user = require_roles(current_user, &["Administrator", "Manager"])?;
    sqlx::query(
        "UPDATE feature_flags
         SET enabled = ?, rollout_percent = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(if payload.enabled { 1 } else { 0 })
    .bind(payload.rollout_percent)
    .bind(&flag_id)
    .execute(&state.pool)
    .await?;

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "update_feature_flag",
        "feature_flags",
        &flag_id,
        json!({"enabled": payload.enabled, "rollout_percent": payload.rollout_percent}),
    )
    .await?;

    let row = sqlx::query_as::<_, FeatureFlagRecord>(
        "SELECT id, key, description, enabled, rollout_percent, audience_rules_json
         FROM feature_flags WHERE id = ?",
    )
    .bind(&flag_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

pub async fn list_cohorts(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<CohortRecord>>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Manager"])?;
    let rows = sqlx::query_as::<_, CohortRecord>(
        "SELECT id, name, description, created_at FROM cohorts ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create_cohort(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateCohortRequest>,
) -> Result<Json<CohortRecord>, AppError> {
    let current_user = require_roles(current_user, &["Administrator", "Manager"])?;
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO cohorts (id, name, description) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&payload.name)
        .bind(&payload.description)
        .execute(&state.pool)
        .await?;

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "create_cohort",
        "cohorts",
        &id,
        json!({"name": payload.name, "description": payload.description}),
    )
    .await?;

    let row = sqlx::query_as::<_, CohortRecord>(
        "SELECT id, name, description, created_at FROM cohorts WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

pub async fn list_cohort_assignments(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Query(query): Query<ListCohortAssignmentsQuery>,
) -> Result<Json<Vec<CohortAssignmentRecord>>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Manager"])?;
    let rows = match (query.cohort_id.as_deref(), query.user_id.as_deref()) {
        (Some(cohort_id), Some(user_id)) => {
            sqlx::query_as::<_, CohortAssignmentRecord>(
                "SELECT id, cohort_id, user_id, assigned_at AS created_at
                 FROM cohort_assignments
                 WHERE cohort_id = ? AND user_id = ?
                 ORDER BY assigned_at DESC",
            )
            .bind(cohort_id)
            .bind(user_id)
            .fetch_all(&state.pool)
            .await?
        }
        (Some(cohort_id), None) => {
            sqlx::query_as::<_, CohortAssignmentRecord>(
                "SELECT id, cohort_id, user_id, assigned_at AS created_at
                 FROM cohort_assignments
                 WHERE cohort_id = ?
                 ORDER BY assigned_at DESC",
            )
            .bind(cohort_id)
            .fetch_all(&state.pool)
            .await?
        }
        (None, Some(user_id)) => {
            sqlx::query_as::<_, CohortAssignmentRecord>(
                "SELECT id, cohort_id, user_id, assigned_at AS created_at
                 FROM cohort_assignments
                 WHERE user_id = ?
                 ORDER BY assigned_at DESC",
            )
            .bind(user_id)
            .fetch_all(&state.pool)
            .await?
        }
        (None, None) => {
            sqlx::query_as::<_, CohortAssignmentRecord>(
                "SELECT id, cohort_id, user_id, assigned_at AS created_at
                 FROM cohort_assignments
                 ORDER BY assigned_at DESC",
            )
            .fetch_all(&state.pool)
            .await?
        }
    };
    Ok(Json(rows))
}

pub async fn assign_cohort(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<AssignCohortRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let current_user = require_roles(current_user, &["Administrator", "Manager"])?;
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM cohorts WHERE id = ?")
        .bind(&payload.cohort_id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::not_found("cohort not found"));
    }
    let user_exists: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
        .bind(&payload.user_id)
        .fetch_optional(&state.pool)
        .await?;
    if user_exists.is_none() {
        return Err(AppError::not_found("user not found"));
    }

    let result = sqlx::query(
        "INSERT INTO cohort_assignments (id, cohort_id, user_id)
         SELECT ?, ?, ?
         WHERE NOT EXISTS (
           SELECT 1 FROM cohort_assignments WHERE cohort_id = ? AND user_id = ?
         )",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&payload.cohort_id)
    .bind(&payload.user_id)
    .bind(&payload.cohort_id)
    .bind(&payload.user_id)
    .execute(&state.pool)
    .await?;

    let status = if result.rows_affected() == 1 { "assigned" } else { "already_assigned" };

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "assign_cohort",
        "cohort_assignments",
        &payload.cohort_id,
        json!({"cohort_id": payload.cohort_id, "user_id": payload.user_id, "result": status}),
    )
    .await?;

    Ok(Json(json!({ "status": status })))
}

pub async fn list_ratings_review(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<RatingReviewRecord>>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Support Agent", "Manager"])?;
    let rows = sqlx::query_as::<_, RatingReviewRecord>(
        "SELECT r.id as rating_id, r.score, r.comments, rr.review_status
         FROM ratings r
         LEFT JOIN rating_reviews rr ON rr.rating_id = r.id
         ORDER BY r.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn list_appeals(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Support Agent", "Manager"])?;
    let rows = sqlx::query(
        "SELECT id, ticket_no, status, reason, resolution, created_at
         FROM appeal_tickets ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| {
                json!({
                    "id": row.get::<String, _>("id"),
                    "ticket_no": row.get::<String, _>("ticket_no"),
                    "status": row.get::<String, _>("status"),
                    "reason": row.get::<String, _>("reason"),
                    "resolution": row.get::<Option<String>, _>("resolution"),
                    "created_at": row.get::<String, _>("created_at"),
                })
            })
            .collect(),
    ))
}

pub async fn create_listing(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateListingRequest>,
) -> Result<Json<ListingCard>, AppError> {
    let current_user = require_roles(
        current_user,
        &["Shopper", "Inventory Clerk", "Administrator", "Manager"],
    )?;
    if payload.price_cents < 0 {
        return Err(AppError::bad_request("price_cents must be non-negative"));
    }
    let id = Uuid::new_v4().to_string();
    let currency = payload.currency.unwrap_or_else(|| "USD".to_string());
    sqlx::query(
        "INSERT INTO listings (id, seller_user_id, campus_id, taxonomy_node_id, condition_id, title, description, price_cents, currency, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'draft')",
    )
    .bind(&id)
    .bind(&current_user.id)
    .bind(&payload.campus_id)
    .bind(&payload.taxonomy_node_id)
    .bind(payload.condition_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(payload.price_cents)
    .bind(&currency)
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as::<_, ListingCard>(
        "SELECT l.id, l.title, l.description, l.price_cents, l.status, l.created_at,
                c.name as campus_name, c.zip_code as campus_zip_code,
                lc.code as condition_code, tn.slug as category_slug
         FROM listings l
         LEFT JOIN campuses c ON c.id = l.campus_id
         LEFT JOIN listing_conditions lc ON lc.id = l.condition_id
         LEFT JOIN taxonomy_nodes tn ON tn.id = l.taxonomy_node_id
         WHERE l.id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

pub async fn create_rating(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateRatingRequest>,
) -> Result<Json<RatingRecord>, AppError> {
    let current_user = auth::require_user(current_user)?;
    if !(1..=5).contains(&payload.score) {
        return Err(AppError::bad_request("score must be between 1 and 5"));
    }
    let listing_exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM listings WHERE id = ?")
            .bind(&payload.listing_id)
            .fetch_optional(&state.pool)
            .await?;
    if listing_exists.is_none() {
        return Err(AppError::not_found("listing not found"));
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ratings (id, listing_id, user_id, score, comments) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&payload.listing_id)
    .bind(&current_user.id)
    .bind(payload.score)
    .bind(&payload.comments)
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as::<_, RatingRecord>(
        "SELECT id, listing_id, user_id, score, comments, created_at FROM ratings WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

pub async fn create_appeal_ticket(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateAppealTicketRequest>,
) -> Result<Json<AppealTicketRecord>, AppError> {
    let current_user = auth::require_user(current_user)?;
    if payload.reason.trim().is_empty() {
        return Err(AppError::bad_request("reason is required"));
    }
    let id = Uuid::new_v4().to_string();
    let ticket_no = format!("APL-{}", &id[..8].to_uppercase());
    sqlx::query(
        "INSERT INTO appeal_tickets (id, ticket_no, listing_id, shipment_order_id, opened_by_user_id, status, reason)
         VALUES (?, ?, ?, ?, ?, 'open', ?)",
    )
    .bind(&id)
    .bind(&ticket_no)
    .bind(&payload.listing_id)
    .bind(&payload.shipment_order_id)
    .bind(&current_user.id)
    .bind(&payload.reason)
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as::<_, AppealTicketRecord>(
        "SELECT id, ticket_no, status, reason, resolution, created_at FROM appeal_tickets WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

pub async fn list_taxonomy_tags(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<TaxonomyTagRecord>>, AppError> {
    let _ = auth::require_user(current_user)?;
    let rows = sqlx::query_as::<_, TaxonomyTagRecord>(
        "SELECT id, name, slug FROM taxonomy_tags ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create_taxonomy_tag(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateTaxonomyTagRequest>,
) -> Result<Json<TaxonomyTagRecord>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Manager"])?;
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO taxonomy_tags (id, name, slug) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&payload.name)
        .bind(&payload.slug)
        .execute(&state.pool)
        .await?;
    let row = sqlx::query_as::<_, TaxonomyTagRecord>(
        "SELECT id, name, slug FROM taxonomy_tags WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

pub async fn list_taxonomy_keywords(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<TaxonomyKeywordRecord>>, AppError> {
    let _ = auth::require_user(current_user)?;
    let rows = sqlx::query_as::<_, TaxonomyKeywordRecord>(
        "SELECT id, keyword FROM taxonomy_keywords ORDER BY keyword",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create_taxonomy_keyword(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateTaxonomyKeywordRequest>,
) -> Result<Json<TaxonomyKeywordRecord>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Manager"])?;
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO taxonomy_keywords (id, keyword) VALUES (?, ?)")
        .bind(&id)
        .bind(&payload.keyword)
        .execute(&state.pool)
        .await?;
    let row = sqlx::query_as::<_, TaxonomyKeywordRecord>(
        "SELECT id, keyword FROM taxonomy_keywords WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

pub async fn associate_taxonomy_tag(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(node_id): Path<String>,
    Json(payload): Json<AssociateTaxonomyTagRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Manager"])?;
    let node_exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM taxonomy_nodes WHERE id = ?")
            .bind(&node_id)
            .fetch_optional(&state.pool)
            .await?;
    if node_exists.is_none() {
        return Err(AppError::not_found("taxonomy node not found"));
    }
    let tag_exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM taxonomy_tags WHERE id = ?")
            .bind(&payload.tag_id)
            .fetch_optional(&state.pool)
            .await?;
    if tag_exists.is_none() {
        return Err(AppError::not_found("tag not found"));
    }
    let result = sqlx::query(
        "INSERT OR IGNORE INTO taxonomy_node_tags (node_id, tag_id) VALUES (?, ?)",
    )
    .bind(&node_id)
    .bind(&payload.tag_id)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({
        "status": if result.rows_affected() == 1 { "associated" } else { "already_associated" }
    })))
}

pub async fn associate_taxonomy_keyword(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(node_id): Path<String>,
    Json(payload): Json<AssociateTaxonomyKeywordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Manager"])?;
    let node_exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM taxonomy_nodes WHERE id = ?")
            .bind(&node_id)
            .fetch_optional(&state.pool)
            .await?;
    if node_exists.is_none() {
        return Err(AppError::not_found("taxonomy node not found"));
    }
    let kw_exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM taxonomy_keywords WHERE id = ?")
            .bind(&payload.keyword_id)
            .fetch_optional(&state.pool)
            .await?;
    if kw_exists.is_none() {
        return Err(AppError::not_found("keyword not found"));
    }
    let result = sqlx::query(
        "INSERT OR IGNORE INTO taxonomy_node_keywords (node_id, keyword_id) VALUES (?, ?)",
    )
    .bind(&node_id)
    .bind(&payload.keyword_id)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({
        "status": if result.rows_affected() == 1 { "associated" } else { "already_associated" }
    })))
}

pub async fn list_my_announcement_deliveries(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<AnnouncementRecord>>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let rows = sqlx::query_as::<_, AnnouncementRecord>(
        "SELECT a.id, a.title, a.body, a.severity, a.starts_at, a.ends_at, a.created_at
         FROM announcement_deliveries d
         JOIN announcements a ON a.id = d.announcement_id
         WHERE d.user_id = ?
         ORDER BY d.delivered_at DESC",
    )
    .bind(&current_user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn mark_announcement_read(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(announcement_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let current_user = auth::require_user(current_user)?;
    let result = sqlx::query(
        "UPDATE announcement_deliveries
         SET read_at = COALESCE(read_at, CURRENT_TIMESTAMP)
         WHERE announcement_id = ? AND user_id = ?",
    )
    .bind(&announcement_id)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::not_found("announcement delivery not found"));
    }
    Ok(Json(json!({"status": "read"})))
}

pub async fn list_local_credentials(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<CredentialRecord>>, AppError> {
    let _ = auth::require_admin(current_user)?;
    let rows = sqlx::query_as::<_, CredentialRecord>(
        "SELECT id, label, username, notes, created_at, updated_at
         FROM local_credentials ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create_local_credential(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateCredentialRequest>,
) -> Result<Json<CredentialRecord>, AppError> {
    let current_user = auth::require_admin(current_user)?;
    let id = Uuid::new_v4().to_string();
    let secret_enc = security::encrypt_field(&state.config.aes256_key_hex, &payload.secret)?;

    sqlx::query(
        "INSERT INTO local_credentials (id, label, username, secret_enc, notes, created_by)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&payload.label)
    .bind(&payload.username)
    .bind(secret_enc)
    .bind(&payload.notes)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "create_local_credential",
        "local_credentials",
        &id,
        json!({
            "label": payload.label,
            "username": payload.username,
            "secret_masked": security::mask_value(&payload.secret),
        }),
    )
    .await?;

    let record = sqlx::query_as::<_, CredentialRecord>(
        "SELECT id, label, username, notes, created_at, updated_at FROM local_credentials WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(record))
}

pub async fn list_companion_credentials(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<CompanionCredentialRecord>>, AppError> {
    let _ = auth::require_admin(current_user)?;
    let rows = sqlx::query_as::<_, CompanionCredentialRecord>(
        "SELECT id, label, provider, endpoint, username, notes, created_at, updated_at
         FROM companion_credentials ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create_companion_credential(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateCompanionCredentialRequest>,
) -> Result<Json<CompanionCredentialRecord>, AppError> {
    let current_user = auth::require_admin(current_user)?;
    let id = Uuid::new_v4().to_string();
    let secret_enc = security::encrypt_field(&state.config.aes256_key_hex, &payload.secret)?;

    sqlx::query(
        "INSERT INTO companion_credentials (id, label, provider, endpoint, username, secret_enc, notes, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&payload.label)
    .bind(&payload.provider)
    .bind(&payload.endpoint)
    .bind(&payload.username)
    .bind(secret_enc)
    .bind(&payload.notes)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "create_companion_credential",
        "companion_credentials",
        &id,
        json!({
            "label": payload.label,
            "provider": payload.provider,
            "endpoint": payload.endpoint,
            "username": payload.username,
            "secret_masked": security::mask_value(&payload.secret),
        }),
    )
    .await?;

    let record = sqlx::query_as::<_, CompanionCredentialRecord>(
        "SELECT id, label, provider, endpoint, username, notes, created_at, updated_at
         FROM companion_credentials WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(record))
}

pub async fn list_templates(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<TemplateRecord>>, AppError> {
    let _ = auth::require_admin(current_user)?;
    let rows = sqlx::query_as::<_, TemplateRecord>(
        "SELECT id, kind, key, title, content, version, is_active, created_at, updated_at
         FROM templates ORDER BY updated_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create_template(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateTemplateRequest>,
) -> Result<Json<TemplateRecord>, AppError> {
    let current_user = auth::require_admin(current_user)?;
    let id = Uuid::new_v4().to_string();
    let is_active = if payload.is_active.unwrap_or(true) { 1 } else { 0 };
    sqlx::query(
        "INSERT INTO templates (id, kind, key, title, content, is_active, updated_by)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&payload.kind)
    .bind(&payload.key)
    .bind(&payload.title)
    .bind(&payload.content)
    .bind(is_active)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "create_template",
        "templates",
        &id,
        json!({"key": payload.key, "kind": payload.kind}),
    )
    .await?;

    let record = sqlx::query_as::<_, TemplateRecord>(
        "SELECT id, kind, key, title, content, version, is_active, created_at, updated_at
         FROM templates WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(record))
}

pub async fn update_template(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTemplateRequest>,
) -> Result<Json<TemplateRecord>, AppError> {
    let current_user = auth::require_admin(current_user)?;
    let is_active = if payload.is_active.unwrap_or(true) { 1 } else { 0 };
    let result = sqlx::query(
        "UPDATE templates
         SET title = ?, content = ?, is_active = ?, version = version + 1, updated_by = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(&payload.title)
    .bind(&payload.content)
    .bind(is_active)
    .bind(&current_user.id)
    .bind(&id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::not_found("template not found"));
    }

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "update_template",
        "templates",
        &id,
        json!({"title": payload.title}),
    )
    .await?;

    let record = sqlx::query_as::<_, TemplateRecord>(
        "SELECT id, kind, key, title, content, version, is_active, created_at, updated_at
         FROM templates WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(record))
}

pub async fn list_announcements(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<Vec<AnnouncementRecord>>, AppError> {
    let _ = auth::require_admin(current_user)?;
    let rows = sqlx::query_as::<_, AnnouncementRecord>(
        "SELECT id, title, body, severity, starts_at, ends_at, created_at
         FROM announcements ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create_announcement(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Json(payload): Json<CreateAnnouncementRequest>,
) -> Result<Json<AnnouncementRecord>, AppError> {
    let current_user = auth::require_admin(current_user)?;
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO announcements (id, title, body, severity, starts_at, ends_at, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&payload.title)
    .bind(&payload.body)
    .bind(&payload.severity)
    .bind(&payload.starts_at)
    .bind(&payload.ends_at)
    .bind(&current_user.id)
    .execute(&state.pool)
    .await?;

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "create_announcement",
        "announcements",
        &id,
        json!({"title": payload.title, "severity": payload.severity}),
    )
    .await?;

    let record = sqlx::query_as::<_, AnnouncementRecord>(
        "SELECT id, title, body, severity, starts_at, ends_at, created_at
         FROM announcements WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(record))
}

pub async fn create_announcement_deliveries(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(announcement_id): Path<String>,
    Json(payload): Json<CreateAnnouncementDeliveriesRequest>,
) -> Result<Json<AnnouncementDeliveryBatchResponse>, AppError> {
    let current_user = auth::require_admin(current_user)?;
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM announcements WHERE id = ?")
        .bind(&announcement_id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::not_found("announcement not found"));
    }

    let CreateAnnouncementDeliveriesRequest { user_ids, cohort_id } = payload;
    let target_users: Vec<String> = if let Some(user_ids) = user_ids {
        user_ids
    } else if let Some(cohort_id) = cohort_id {
        sqlx::query_scalar("SELECT user_id FROM cohort_assignments WHERE cohort_id = ?")
            .bind(&cohort_id)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query_scalar("SELECT id FROM users")
            .fetch_all(&state.pool)
            .await?
    };

    let mut delivered_count = 0_i64;
    for user_id in target_users {
        let result = sqlx::query(
            "INSERT INTO announcement_deliveries (id, announcement_id, user_id)
             SELECT ?, ?, ?
             WHERE NOT EXISTS (
                 SELECT 1 FROM announcement_deliveries
                 WHERE announcement_id = ? AND user_id = ?
             )",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&announcement_id)
        .bind(&user_id)
        .bind(&announcement_id)
        .bind(&user_id)
        .execute(&state.pool)
        .await?;
        delivered_count += result.rows_affected() as i64;
    }

    db::insert_admin_audit(
        &state.pool,
        &current_user.id,
        "create_announcement_deliveries",
        "announcement_deliveries",
        &announcement_id,
        json!({"announcement_id": &announcement_id, "delivered_count": delivered_count}),
    )
    .await?;

    Ok(Json(AnnouncementDeliveryBatchResponse {
        announcement_id,
        delivered_count,
    }))
}

pub async fn list_announcement_deliveries(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    Path(announcement_id): Path<String>,
) -> Result<Json<Vec<AnnouncementDeliveryRecord>>, AppError> {
    let _ = auth::require_admin(current_user)?;
    let rows = sqlx::query_as::<_, AnnouncementDeliveryRecord>(
        "SELECT announcement_id, user_id, delivered_at, read_at
         FROM announcement_deliveries
         WHERE announcement_id = ?
         ORDER BY delivered_at DESC",
    )
    .bind(&announcement_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn dashboard_metrics(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
) -> Result<Json<DashboardMetricsResponse>, AppError> {
    let _ = require_roles(current_user, &["Administrator", "Manager"])?;
    let pool = &state.pool;

    let active_users_last_30_days: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT user_id)
         FROM event_logs
         WHERE user_id IS NOT NULL AND created_at >= ?",
    )
    .bind((Utc::now() - Duration::days(30)).to_rfc3339())
    .fetch_one(pool)
    .await?;
    let order_count: i64 = db::get_dashboard_count(pool, "orders").await?;
    let listing_view_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM event_logs WHERE event_name = 'listing_view'",
    )
    .fetch_one(pool)
    .await?;
    let average_rating: Option<f64> = sqlx::query_scalar("SELECT AVG(score) FROM ratings")
        .fetch_one(pool)
        .await?;
    let open_support_cases: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM after_sales_cases WHERE status != 'closed'",
    )
    .fetch_one(pool)
    .await?;

    Ok(Json(DashboardMetricsResponse {
        total_users: db::get_dashboard_count(pool, "users").await?,
        total_announcements: db::get_dashboard_count(pool, "announcements").await?,
        total_templates: db::get_dashboard_count(pool, "templates").await?,
        total_local_credentials: db::get_dashboard_count(pool, "local_credentials").await?,
        total_companion_credentials: db::get_dashboard_count(pool, "companion_credentials").await?,
        total_uploads: db::get_dashboard_count(pool, "listing_media").await?,
        total_shipments: db::get_dashboard_count(pool, "shipment_orders").await?,
        total_feature_flags: db::get_dashboard_count(pool, "feature_flags").await?,
        total_events: db::get_dashboard_count(pool, "event_logs").await?,
        active_users_last_30_days,
        conversion_rate_percent: if listing_view_count == 0 {
            0.0
        } else {
            (order_count as f64 / listing_view_count as f64) * 100.0
        },
        average_rating: average_rating.unwrap_or(0.0),
        open_support_cases,
    }))
}

pub async fn upload_media(
    State(state): State<AppState>,
    Extension(current_user): Extension<Option<CurrentUser>>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, AppError> {
    let current_user = auth::require_admin(current_user)?;
    tokio::fs::create_dir_all(&state.config.upload_dir).await?;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("invalid multipart payload"))?
    {
        if field.name() != Some("file") {
            continue;
        }

        let mime_type = field
            .content_type()
            .map(str::to_string)
            .ok_or_else(|| AppError::bad_request("content type is required"))?;
        validate_mime(&mime_type)?;

        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::bad_request("failed to read upload"))?;
        if bytes.len() > state.config.max_upload_size_bytes {
            return Err(AppError::bad_request("file exceeds maximum allowed upload size"));
        }
        let sha256 = security::sha256_hex(&bytes);
        let media_id = Uuid::new_v4().to_string();
        let playback_token = security::random_token();
        let file_name = format!("{media_id}.bin");
        let mut storage_path = PathBuf::from(&state.config.upload_dir);
        storage_path.push(file_name);

        let mut file = tokio::fs::File::create(&storage_path).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;

        sqlx::query(
            "INSERT INTO listing_media (id, listing_id, storage_path, mime_type, sha256, size_bytes, media_kind, playback_token)
             VALUES (?, NULL, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&media_id)
        .bind(storage_path.to_string_lossy().to_string())
        .bind(&mime_type)
        .bind(&sha256)
        .bind(bytes.len() as i64)
        .bind(media_kind(&mime_type))
        .bind(&playback_token)
        .execute(&state.pool)
        .await?;

        db::insert_admin_audit(
            &state.pool,
            &current_user.id,
            "upload_media",
            "listing_media",
            &media_id,
            json!({
                "mime_type": mime_type,
                "sha256": sha256,
                "size_bytes": bytes.len(),
            }),
        )
        .await?;

        return Ok(Json(UploadResponse {
            media_id,
            mime_type,
            sha256,
            storage_path: storage_path.to_string_lossy().to_string(),
            playback_token,
        }));
    }

    Err(AppError::bad_request("multipart field `file` is required"))
}

async fn record_search_history(
    pool: &sqlx::SqlitePool,
    user_id: &str,
    query_text: &str,
    filters_json: Option<serde_json::Value>,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO search_history (id, user_id, query_text, filters_json)
         VALUES (?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(query_text)
    .bind(filters_json.map(|value| value.to_string()))
    .execute(pool)
    .await?;
    Ok(())
}

async fn fetch_recommendations(
    pool: &sqlx::SqlitePool,
    user_id: &str,
    exclude_listing_id: Option<&str>,
) -> Result<Vec<RecommendationCard>, AppError> {
    let enabled: i64 = sqlx::query_scalar(
        "SELECT recommendations_enabled FROM user_settings WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(1);
    if enabled == 0 {
        return Ok(Vec::new());
    }

    let interaction_rows = sqlx::query(
        "SELECT DISTINCT t.slug as category_slug
         FROM listings l
         JOIN taxonomy_nodes t ON t.id = l.taxonomy_node_id
         WHERE l.id IN (
             SELECT json_extract(properties_json, '$.listing_id') FROM event_logs WHERE user_id = ? AND event_name = 'listing_view'
             UNION
             SELECT listing_id FROM favorites WHERE user_id = ?
             UNION
             SELECT listing_id FROM order_items oi JOIN orders o ON o.id = oi.order_id WHERE o.user_id = ?
         )",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let preferred_categories: Vec<String> = interaction_rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("category_slug").ok())
        .collect();
    let popularity_map = listing_popularity_map(pool).await?;

    let rows = sqlx::query(
        "SELECT l.id, l.title, l.price_cents, t.slug as category_slug
         FROM listings l
         LEFT JOIN taxonomy_nodes t ON t.id = l.taxonomy_node_id
         WHERE l.status = 'published'",
    )
    .fetch_all(pool)
    .await?;

    let mut cards: Vec<(RecommendationCard, i64)> = rows
        .into_iter()
        .filter_map(|row| {
            let listing_id: String = row.get("id");
            if exclude_listing_id == Some(listing_id.as_str()) {
                return None;
            }
            let title: String = row.get("title");
            let price_cents: i64 = row.get("price_cents");
            let category_slug: Option<String> = row.get("category_slug");

            let reason = if let Some(category) = category_slug {
                if preferred_categories.iter().any(|value| value == &category) {
                    Some(format!("Because you viewed or ordered {} devices locally", category))
                } else {
                    None
                }
            } else {
                None
            }
            .unwrap_or_else(|| "Popular with local DepotCycle shoppers".into());

            Some((
                RecommendationCard {
                    listing_id: listing_id.clone(),
                    title,
                    reason,
                    price_cents,
                },
                popularity_map.get(&listing_id).copied().unwrap_or(0),
            ))
        })
        .collect();

    cards.sort_by_key(|(_, score)| -*score);
    cards.truncate(4);
    Ok(cards.into_iter().map(|(card, _)| card).collect())
}

async fn listing_popularity_map(
    pool: &sqlx::SqlitePool,
) -> Result<HashMap<String, i64>, AppError> {
    let mut map = HashMap::new();
    for row in sqlx::query(
        "SELECT json_extract(properties_json, '$.listing_id') AS listing_id, COUNT(*) as count
         FROM event_logs
         WHERE event_name = 'listing_view'
         GROUP BY listing_id",
    )
    .fetch_all(pool)
    .await?
    {
        if let Ok(listing_id) = row.try_get::<String, _>("listing_id") {
            map.insert(listing_id, row.get("count"));
        }
    }
    for row in sqlx::query(
        "SELECT listing_id, COUNT(*) as count FROM favorites GROUP BY listing_id",
    )
    .fetch_all(pool)
    .await?
    {
        let listing_id: String = row.get("listing_id");
        *map.entry(listing_id).or_insert(0) += row.get::<i64, _>("count") * 2;
    }
    for row in sqlx::query(
        "SELECT listing_id, SUM(quantity) as count FROM order_items GROUP BY listing_id",
    )
    .fetch_all(pool)
    .await?
    {
        let listing_id: String = row.get("listing_id");
        *map.entry(listing_id).or_insert(0) += row.get::<i64, _>("count") * 3;
    }
    Ok(map)
}

async fn log_event(
    pool: &sqlx::SqlitePool,
    event_name: &str,
    user_id: Option<&str>,
    session_id: Option<&str>,
    properties_json: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO event_logs (id, event_name, user_id, session_id, properties_json)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(event_name)
    .bind(user_id)
    .bind(session_id)
    .bind(properties_json.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

fn relevance_score(card: &ListingCard, query: &str) -> i64 {
    let query = query.to_lowercase();
    let mut score = 0;
    if card.title.to_lowercase().contains(&query) {
        score += 5;
    }
    if card
        .description
        .clone()
        .unwrap_or_default()
        .to_lowercase()
        .contains(&query)
    {
        score += 2;
    }
    if card
        .category_slug
        .clone()
        .unwrap_or_default()
        .to_lowercase()
        .contains(&query)
    {
        score += 3;
    }
    score
}

fn approximate_distance_score(
    campus_coords: &HashMap<String, (f64, f64)>,
    card: &ListingCard,
    zip_code: Option<&str>,
) -> Result<f64, AppError> {
    let Some(zip_code) = zip_code else {
        return Ok(0.0);
    };
    let Some((lat1, lon1)) = campus_coords.get(zip_code).copied() else {
        return Ok(0.0);
    };
    let campus_zip = card.campus_zip_code.clone().unwrap_or_default();
    let Some((lat2, lon2)) = campus_coords.get(&campus_zip).copied() else {
        return Ok(0.0);
    };
    Ok(((lat1 - lat2).powi(2) + (lon1 - lon2).powi(2)).sqrt())
}

async fn load_campus_coords(
    pool: &sqlx::SqlitePool,
) -> Result<HashMap<String, (f64, f64)>, AppError> {
    let mut map = HashMap::new();
    for row in sqlx::query("SELECT zip_code, latitude, longitude FROM campuses")
        .fetch_all(pool)
        .await?
    {
        map.insert(
            row.get::<String, _>("zip_code"),
            (row.get::<f64, _>("latitude"), row.get::<f64, _>("longitude")),
        );
    }
    Ok(map)
}

async fn execute_inventory_document(
    pool: &sqlx::SqlitePool,
    operator_user_id: &str,
    document_id: &str,
) -> Result<(), AppError> {
    let document = sqlx::query(
        "SELECT doc_type, target_campus_id FROM inventory_documents WHERE id = ?",
    )
    .bind(document_id)
    .fetch_one(pool)
    .await?;
    let doc_type: String = document.get("doc_type");
    let doc_target_campus_id: Option<String> = document.get("target_campus_id");
    let lines = sqlx::query(
        "SELECT device_id, quantity, target_campus_id FROM inventory_document_lines WHERE document_id = ?",
    )
    .bind(document_id)
    .fetch_all(pool)
    .await?;

    let mut seen_device_ids = HashSet::new();
    for line in lines {
        let device_id: String = line.get("device_id");
        let quantity: i64 = line.get("quantity");
        if quantity != 1 {
            return Err(AppError::bad_request(
                "inventory document line quantity must be exactly 1",
            ));
        }
        if !seen_device_ids.insert(device_id.clone()) {
            return Err(AppError::bad_request(
                "duplicate device_id in inventory document lines",
            ));
        }
        let target_campus: Option<String> = line.get("target_campus_id");
        let before = sqlx::query(
            "SELECT campus_id, status, metadata_json FROM inventory_devices WHERE id = ?",
        )
        .bind(&device_id)
        .fetch_one(pool)
        .await?;
        let before_json = json!({
            "campus_id": before.get::<Option<String>, _>("campus_id"),
            "status": before.get::<String, _>("status"),
            "metadata_json": before.get::<Option<String>, _>("metadata_json"),
        });
        let (new_status, new_campus_id) = match doc_type.as_str() {
            "receiving" => ("on_hand", target_campus.or(doc_target_campus_id.clone())),
            "issuing" => ("issued", target_campus.or(doc_target_campus_id.clone())),
            "transfer" => ("on_hand", target_campus.or(doc_target_campus_id.clone())),
            "return" => ("returned", target_campus.or(doc_target_campus_id.clone())),
            "loan" => ("on_loan", target_campus.or(doc_target_campus_id.clone())),
            "scrap" => ("scrapped", before.get::<Option<String>, _>("campus_id")),
            _ => ("on_hand", before.get::<Option<String>, _>("campus_id")),
        };

        sqlx::query(
            "UPDATE inventory_devices SET status = ?, campus_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(new_status)
        .bind(new_campus_id.clone())
        .bind(&device_id)
        .execute(pool)
        .await?;

        let after_json = json!({
            "campus_id": new_campus_id,
            "status": new_status,
        });
        sqlx::query(
            "INSERT INTO ledger_change_records (id, table_name, record_id, before_json, after_json, operator_user_id, related_document_id)
             VALUES (?, 'inventory_devices', ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&device_id)
        .bind(before_json.to_string())
        .bind(after_json.to_string())
        .bind(operator_user_id)
        .bind(document_id)
        .execute(pool)
        .await?;
    }

    sqlx::query("UPDATE inventory_documents SET workflow_status = 'executed' WHERE id = ?")
        .bind(document_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn fetch_shipment(pool: &sqlx::SqlitePool, shipment_id: &str) -> Result<ShipmentRecord, AppError> {
    Ok(sqlx::query_as::<_, ShipmentRecord>(
        "SELECT id, order_number, status, carrier_name, tracking_number, integration_enabled, created_at
         FROM shipment_orders WHERE id = ?",
    )
    .bind(shipment_id)
    .fetch_one(pool)
    .await?)
}

async fn fetch_after_sales_case(
    pool: &sqlx::SqlitePool,
    case_id: &str,
) -> Result<AfterSalesCaseRecord, AppError> {
    Ok(sqlx::query_as::<_, AfterSalesCaseRecord>(
        "SELECT id, case_type, status, reason, first_response_due_at, final_decision_due_at, created_at
         FROM after_sales_cases WHERE id = ?",
    )
    .bind(case_id)
    .fetch_one(pool)
    .await?)
}

fn require_roles(
    current_user: Option<CurrentUser>,
    roles: &[&str],
) -> Result<CurrentUser, AppError> {
    let user = auth::require_user(current_user)?;
    if roles.iter().any(|role| *role == user.role_name) {
        Ok(user)
    } else {
        Err(AppError::forbidden("role not permitted for this action"))
    }
}

fn is_support_staff(role_name: &str) -> bool {
    matches!(role_name, "Support Agent" | "Administrator" | "Manager")
}

fn is_upload_session_privileged(role_name: &str) -> bool {
    matches!(role_name, "Administrator" | "Manager")
}

fn ensure_upload_session_access(current_user: &CurrentUser, session_owner_id: &str) -> Result<(), AppError> {
    if current_user.id == session_owner_id || is_upload_session_privileged(&current_user.role_name) {
        Ok(())
    } else {
        Err(AppError::forbidden("not authorized for this upload session"))
    }
}

async fn ensure_after_sales_case_access(
    pool: &sqlx::SqlitePool,
    current_user: &CurrentUser,
    case_id: &str,
) -> Result<(), AppError> {
    let row = sqlx::query("SELECT opened_by_user_id FROM after_sales_cases WHERE id = ?")
        .bind(case_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("case not found"))?;
    let opened_by_user_id: String = row.get("opened_by_user_id");
    if is_support_staff(&current_user.role_name) || opened_by_user_id == current_user.id {
        Ok(())
    } else {
        Err(AppError::forbidden("not authorized for this after-sales case"))
    }
}

async fn ensure_media_attach_access(
    pool: &sqlx::SqlitePool,
    current_user: &CurrentUser,
    media_id: &str,
) -> Result<(), AppError> {
    let row = sqlx::query(
        "SELECT mus.created_by
         FROM listing_media lm
         LEFT JOIN media_upload_sessions mus ON mus.id = lm.chunk_group
         WHERE lm.id = ?",
    )
    .bind(media_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("media not found"))?;

    if is_support_staff(&current_user.role_name) {
        return Ok(());
    }

    let media_owner = row.get::<Option<String>, _>("created_by");
    if media_owner.as_deref() == Some(current_user.id.as_str()) {
        Ok(())
    } else {
        Err(AppError::forbidden("not authorized to attach this media"))
    }
}

fn validate_mime(mime_type: &str) -> Result<(), AppError> {
    match mime_type {
        "image/png" | "image/jpeg" | "image/webp" | "video/mp4" | "video/webm" => Ok(()),
        _ => Err(AppError::bad_request("unsupported MIME type")),
    }
}

fn media_kind(mime_type: &str) -> &'static str {
    if mime_type.starts_with("video/") {
        "video"
    } else {
        "photo"
    }
}
