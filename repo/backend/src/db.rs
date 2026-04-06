use chrono::{Duration, Utc};
use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Row, SqlitePool};
use uuid::Uuid;

use crate::{config::AppConfig, error::AppError, security};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, AppError> {
    if let Some(path) = database_url.strip_prefix("sqlite://") {
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }

    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;
    let opts = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| AppError { status: axum::http::StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() })?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON;").execute(&pool).await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), AppError> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

pub async fn bootstrap_admin_if_missing(pool: &SqlitePool, config: &AppConfig) -> Result<(), AppError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    if count > 0 {
        return Ok(());
    }

    let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE name = 'Administrator'")
        .fetch_one(pool)
        .await?;
    let password_hash = security::hash_password(&config.admin_password)?;
    let display_name_enc = security::encrypt_field(&config.aes256_key_hex, &config.admin_display_name)?;
    let phone_enc = security::encrypt_field(&config.aes256_key_hex, &config.admin_phone)?;

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role_id, display_name_enc, phone_enc)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&config.admin_username)
    .bind(password_hash)
    .bind(role_id)
    .bind(display_name_enc)
    .bind(phone_enc)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn seed_demo_data(pool: &SqlitePool, config: &AppConfig) -> Result<(), AppError> {
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(pool).await?;
    if user_count < 5 {
        let roles = [
            ("shopper", "Shopper", "Campus Shopper"),
            ("clerk", "Inventory Clerk", "Inventory Clerk"),
            ("manager", "Manager", "Operations Manager"),
            ("support", "Support Agent", "Support Agent"),
        ];

        for (username, role_name, display_name) in roles {
            let exists: Option<String> =
                sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
            if exists.is_some() {
                continue;
            }

            let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE name = ?")
                .bind(role_name)
                .fetch_one(pool)
                .await?;
            let password_hash = security::hash_password("DepotCycleDemo123!")?;
            let display_name_enc = security::encrypt_field(&config.aes256_key_hex, display_name)?;
            let phone_enc =
                security::encrypt_field(&config.aes256_key_hex, "+15550000000")?;

            sqlx::query(
                "INSERT INTO users (id, username, password_hash, role_id, display_name_enc, phone_enc)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(username)
            .bind(password_hash)
            .bind(role_id)
            .bind(display_name_enc)
            .bind(phone_enc)
            .execute(pool)
            .await?;
        }
    }

    let campus_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM campuses").fetch_one(pool).await?;
    if campus_count == 0 {
        for (name, zip, lat, lon) in [
            ("North Campus", "10001", 40.7506_f64, -73.9972_f64),
            ("West Campus", "60601", 41.8864_f64, -87.6186_f64),
            ("South Campus", "73301", 30.2669_f64, -97.7428_f64),
        ] {
            sqlx::query(
                "INSERT INTO campuses (id, name, zip_code, latitude, longitude) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(name)
            .bind(zip)
            .bind(lat)
            .bind(lon)
            .execute(pool)
            .await?;
        }
    }

    let taxonomy_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM taxonomy_nodes").fetch_one(pool).await?;
    if taxonomy_count == 0 {
        let laptops = Uuid::new_v4().to_string();
        let phones = Uuid::new_v4().to_string();
        let support = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO taxonomy_nodes (id, parent_id, name, slug, level, seo_title, seo_description, seo_keywords, topic_page_path) VALUES (?, NULL, 'Laptops', 'laptops', 1, 'Laptop Devices', 'Portable computers', 'laptop,portable', '/topics/laptops')")
            .bind(&laptops)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO taxonomy_nodes (id, parent_id, name, slug, level, seo_title, seo_description, seo_keywords, topic_page_path) VALUES (?, NULL, 'Phones', 'phones', 1, 'Phone Devices', 'Mobile phones', 'phone,mobile', '/topics/phones')")
            .bind(&phones)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO taxonomy_nodes (id, parent_id, name, slug, level, seo_title, seo_description, seo_keywords, topic_page_path) VALUES (?, NULL, 'Help Center', 'help-center', 1, 'Help Center', 'Support topics', 'help,returns', '/topics/help-center')")
            .bind(&support)
            .execute(pool)
            .await?;
    }

    let listing_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM listings").fetch_one(pool).await?;
    if listing_count == 0 {
        let shopper_id: String = sqlx::query_scalar("SELECT id FROM users WHERE username = 'shopper'")
            .fetch_one(pool)
            .await?;
        let campus_rows =
            sqlx::query("SELECT id FROM campuses ORDER BY name").fetch_all(pool).await?;
        let taxonomy_rows = sqlx::query(
            "SELECT id, slug FROM taxonomy_nodes WHERE slug IN ('laptops','phones') ORDER BY slug",
        )
        .fetch_all(pool)
        .await?;
        let good_condition: i64 =
            sqlx::query_scalar("SELECT id FROM listing_conditions WHERE code = 'good'")
                .fetch_one(pool)
                .await?;
        let refurbished_condition: i64 =
            sqlx::query_scalar("SELECT id FROM listing_conditions WHERE code = 'refurbished'")
                .fetch_one(pool)
                .await?;

        let campus_a: String = campus_rows[0].get("id");
        let campus_b: String = campus_rows[1].get("id");
        let laptops_id: String = taxonomy_rows[0].get("id");
        let phones_id: String = taxonomy_rows[1].get("id");

        let samples = [
            (
                "Latitude Pro 7420",
                "Business laptop with local warranty",
                89_900_i64,
                good_condition,
                campus_a.clone(),
                laptops_id.clone(),
            ),
            (
                "ThinkPad X1 Carbon",
                "Refurbished ultrabook with dock",
                129_900_i64,
                refurbished_condition,
                campus_b.clone(),
                laptops_id,
            ),
            (
                "Pixel 8",
                "Unlocked phone with fresh battery report",
                54_900_i64,
                good_condition,
                campus_a.clone(),
                phones_id.clone(),
            ),
            (
                "iPhone 13",
                "Student favorite with high popularity",
                64_900_i64,
                refurbished_condition,
                campus_b,
                phones_id,
            ),
        ];

        for (title, description, price, condition_id, campus_id, taxonomy_node_id) in samples {
            let listing_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO listings (id, seller_user_id, campus_id, taxonomy_node_id, condition_id, title, description, price_cents, status)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'published')",
            )
            .bind(&listing_id)
            .bind(&shopper_id)
            .bind(campus_id)
            .bind(taxonomy_node_id)
            .bind(condition_id)
            .bind(title)
            .bind(description)
            .bind(price)
            .execute(pool)
            .await?;
        }
    }

    let device_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_devices")
        .fetch_one(pool)
        .await?;
    if device_count == 0 {
        let listing_rows = sqlx::query("SELECT id, campus_id FROM listings ORDER BY created_at")
            .fetch_all(pool)
            .await?;
        for (idx, row) in listing_rows.into_iter().enumerate() {
            for unit in 0..3 {
                sqlx::query(
                    "INSERT INTO inventory_devices (id, listing_id, serial_number, asset_tag, status, campus_id, metadata_json)
                     VALUES (?, ?, ?, ?, 'on_hand', ?, ?)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(row.get::<String, _>("id"))
                .bind(format!("SERIAL-{idx}-{unit}"))
                .bind(format!("ASSET-{idx}-{unit}"))
                .bind(row.get::<String, _>("campus_id"))
                .bind(serde_json::json!({"grade": "A", "batch": idx}).to_string())
                .execute(pool)
                .await?;
            }
        }
    }

    let flag_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feature_flags")
        .fetch_one(pool)
        .await?;
    if flag_count == 0 {
        for (key, description, enabled, rollout) in [
            ("new-search-ranking", "A/B toggle for search ranking weights", 0_i64, 50_i64),
            ("support-canary", "Canary release for support workflows", 0_i64, 10_i64),
        ] {
            sqlx::query(
                "INSERT INTO feature_flags (id, key, description, enabled, rollout_percent, audience_rules_json)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(key)
            .bind(description)
            .bind(enabled)
            .bind(rollout)
            .bind(serde_json::json!({"mode": "local"}).to_string())
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

pub async fn insert_admin_audit(
    pool: &SqlitePool,
    actor_user_id: &str,
    action: &str,
    target_table: &str,
    target_id: &str,
    details_json: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO admin_audit_trails (id, actor_user_id, action, target_table, target_id, details_json)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(actor_user_id)
    .bind(action)
    .bind(target_table)
    .bind(target_id)
    .bind(details_json.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn create_session(
    pool: &SqlitePool,
    user_id: &str,
    token_hash: &str,
    ip_address: Option<String>,
    user_agent: Option<String>,
    idle_timeout_minutes: i64,
) -> Result<String, AppError> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires = now + Duration::minutes(idle_timeout_minutes);

    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, ip_address, user_agent, last_activity_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(user_id)
    .bind(token_hash)
    .bind(ip_address)
    .bind(user_agent)
    .bind(now.to_rfc3339())
    .bind(expires.to_rfc3339())
    .execute(pool)
    .await?;

    Ok(session_id)
}

pub async fn touch_session(pool: &SqlitePool, session_id: &str, idle_timeout_minutes: i64) -> Result<(), AppError> {
    let now = Utc::now();
    let expires = now + Duration::minutes(idle_timeout_minutes);
    sqlx::query("UPDATE sessions SET last_activity_at = ?, expires_at = ? WHERE id = ?")
        .bind(now.to_rfc3339())
        .bind(expires.to_rfc3339())
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_session_by_hash(pool: &SqlitePool, token_hash: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub fn is_locked(locked_until: Option<&str>) -> Result<bool, AppError> {
    match locked_until {
        Some(value) if !value.is_empty() => {
            let ts = chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|_| AppError::internal("invalid lock timestamp in database"))?;
            Ok(ts.with_timezone(&Utc) > Utc::now())
        }
        _ => Ok(false),
    }
}

pub fn lockout_until(minutes: i64) -> String {
    (Utc::now() + Duration::minutes(minutes)).to_rfc3339()
}

pub async fn get_dashboard_count(pool: &SqlitePool, table: &str) -> Result<i64, AppError> {
    let sql = match table {
        "users"                 => "SELECT COUNT(*) FROM users",
        "announcements"         => "SELECT COUNT(*) FROM announcements",
        "templates"             => "SELECT COUNT(*) FROM templates",
        "local_credentials"     => "SELECT COUNT(*) FROM local_credentials",
        "companion_credentials" => "SELECT COUNT(*) FROM companion_credentials",
        "listing_media"         => "SELECT COUNT(*) FROM listing_media",
        "shipment_orders"       => "SELECT COUNT(*) FROM shipment_orders",
        "feature_flags"         => "SELECT COUNT(*) FROM feature_flags",
        "event_logs"            => "SELECT COUNT(*) FROM event_logs",
        "orders"                => "SELECT COUNT(*) FROM orders",
        _ => return Err(AppError::internal("unknown dashboard table")),
    };
    let count: i64 = sqlx::query_scalar(sql).fetch_one(pool).await?;
    Ok(count)
}
