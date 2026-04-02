use axum::{
    extract::{Request, State},
    http::{header, HeaderMap},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use sqlx::Row;

use crate::{
    app::AppState,
    db,
    error::AppError,
    models::CurrentUser,
    security,
};

pub const SESSION_COOKIE: &str = "depotcycle_session";

pub async fn session_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    request.extensions_mut().insert::<Option<CurrentUser>>(None);
    let token = extract_cookie_token(request.headers());

    if let Some(token) = token {
        let token_hash = security::sha256_hex(token.as_bytes());
        let row = sqlx::query(
            "SELECT s.id as session_id, u.id as user_id, u.username, r.name as role_name, s.expires_at
             FROM sessions s
             JOIN users u ON u.id = s.user_id
             JOIN roles r ON r.id = u.role_id
             WHERE s.token_hash = ?",
        )
        .bind(token_hash)
        .fetch_optional(&state.pool)
        .await?;

        if let Some(row) = row {
            let expires_at: String = row.get("expires_at");
            let expires = chrono::DateTime::parse_from_rfc3339(&expires_at)
                .map_err(|_| AppError::internal("invalid session expiry"))?
                .with_timezone(&Utc);

            if expires > Utc::now() {
                let session_id: String = row.get("session_id");
                db::touch_session(
                    &state.pool,
                    &session_id,
                    state.config.session_idle_timeout_minutes,
                )
                .await?;

                request.extensions_mut().insert::<Option<CurrentUser>>(Some(CurrentUser {
                    id: row.get("user_id"),
                    username: row.get("username"),
                    role_name: row.get("role_name"),
                    session_id: Some(session_id),
                }));
            }
        }
    }

    Ok(next.run(request).await)
}

pub fn extract_cookie_token(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for pair in cookie_header.split(';') {
        let mut parts = pair.trim().splitn(2, '=');
        let name = parts.next()?.trim();
        let value = parts.next()?.trim();
        if name == SESSION_COOKIE {
            return Some(value.to_string());
        }
    }
    None
}

pub fn require_user(current_user: Option<CurrentUser>) -> Result<CurrentUser, AppError> {
    current_user.ok_or_else(|| AppError::unauthorized("authentication required"))
}

pub fn require_admin(current_user: Option<CurrentUser>) -> Result<CurrentUser, AppError> {
    let user = require_user(current_user)?;
    if user.role_name != "Administrator" {
        return Err(AppError::forbidden("administrator access required"));
    }
    Ok(user)
}
