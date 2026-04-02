use std::env;

use crate::error::AppError;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub upload_dir: String,
    pub aes256_key_hex: String,
    pub session_idle_timeout_minutes: i64,
    pub login_lockout_minutes: i64,
    pub login_max_failures: i64,
    pub admin_username: String,
    pub admin_password: String,
    pub admin_display_name: String,
    pub admin_phone: String,
    pub public_api_base_url: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, AppError> {
        Ok(Self {
            host: env::var("BACKEND_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("BACKEND_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|_| AppError::internal("invalid BACKEND_PORT"))?,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:///data/depotcycle.db".to_string()),
            upload_dir: env::var("UPLOAD_DIR").unwrap_or_else(|_| "/app/uploads".to_string()),
            aes256_key_hex: env::var("AES256_KEY_HEX")
                .map_err(|_| AppError::internal("AES256_KEY_HEX is required"))?,
            session_idle_timeout_minutes: env::var("SESSION_IDLE_TIMEOUT_MINUTES")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .map_err(|_| AppError::internal("invalid SESSION_IDLE_TIMEOUT_MINUTES"))?,
            login_lockout_minutes: env::var("LOGIN_LOCKOUT_MINUTES")
                .unwrap_or_else(|_| "15".to_string())
                .parse()
                .map_err(|_| AppError::internal("invalid LOGIN_LOCKOUT_MINUTES"))?,
            login_max_failures: env::var("LOGIN_MAX_FAILURES")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .map_err(|_| AppError::internal("invalid LOGIN_MAX_FAILURES"))?,
            admin_username: env::var("ADMIN_USERNAME")
                .unwrap_or_else(|_| "admin".to_string()),
            admin_password: env::var("ADMIN_PASSWORD")
                .unwrap_or_else(|_| "DepotCycleAdmin123!".to_string()),
            admin_display_name: env::var("ADMIN_DISPLAY_NAME")
                .unwrap_or_else(|_| "System Administrator".to_string()),
            admin_phone: env::var("ADMIN_PHONE")
                .unwrap_or_else(|_| "+15550001111".to_string()),
            public_api_base_url: env::var("PUBLIC_API_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
        })
    }
}
