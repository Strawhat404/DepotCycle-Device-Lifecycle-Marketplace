use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::error::AppError;

pub fn validate_password_policy(password: &str) -> Result<(), AppError> {
    if password.chars().count() < 12 {
        return Err(AppError::bad_request(
            "password must be at least 12 characters long",
        ));
    }
    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    validate_password_policy(password)?;
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub fn verify_password(hash: &str, password: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn encrypt_field(key_hex: &str, plaintext: &str) -> Result<String, AppError> {
    let key = hex::decode(key_hex).map_err(|_| AppError::internal("invalid AES256_KEY_HEX"))?;
    if key.len() != 32 {
        return Err(AppError::internal("AES256_KEY_HEX must be 32 bytes"));
    }
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|_| AppError::internal("invalid AES key"))?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())?;
    Ok(format!("{}:{}", B64.encode(nonce_bytes), B64.encode(ciphertext)))
}

pub fn decrypt_field(key_hex: &str, payload: &str) -> Result<String, AppError> {
    let key = hex::decode(key_hex).map_err(|_| AppError::internal("invalid AES256_KEY_HEX"))?;
    if key.len() != 32 {
        return Err(AppError::internal("AES256_KEY_HEX must be 32 bytes"));
    }
    let mut parts = payload.split(':');
    let nonce_b64 = parts
        .next()
        .ok_or_else(|| AppError::internal("invalid encrypted payload"))?;
    let cipher_b64 = parts
        .next()
        .ok_or_else(|| AppError::internal("invalid encrypted payload"))?;
    let nonce_bytes = B64
        .decode(nonce_b64)
        .map_err(|_| AppError::internal("invalid nonce encoding"))?;
    let cipher_bytes = B64
        .decode(cipher_b64)
        .map_err(|_| AppError::internal("invalid ciphertext encoding"))?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|_| AppError::internal("invalid AES key"))?;
    let plaintext = cipher.decrypt(Nonce::from_slice(&nonce_bytes), cipher_bytes.as_ref())?;
    String::from_utf8(plaintext).map_err(|_| AppError::internal("plaintext was not valid utf8"))
}

pub fn mask_value(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 2 {
        return "*".repeat(chars.len());
    }
    let first = chars[0];
    let last = chars[chars.len() - 1];
    format!("{first}{}{last}", "*".repeat(chars.len() - 2))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn random_token() -> String {
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    B64.encode(raw)
}

pub fn now_utc() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}
