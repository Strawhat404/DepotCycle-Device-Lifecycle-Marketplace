#[cfg(test)]
mod tests {
    use backend::{security, workflows};
    use chrono::{TimeZone, Utc};

    const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn password_policy_rejects_short_passwords() {
        assert!(security::validate_password_policy("short").is_err());
    }

    #[test]
    fn password_hash_roundtrip_works() {
        let password = "LongEnoughPassword!";
        let hash = security::hash_password(password).expect("hash");
        let verified = security::verify_password(&hash, password).expect("verify");
        assert!(verified);
    }

    #[test]
    fn encrypted_fields_roundtrip() {
        let plaintext = "Jane Doe";
        let ciphertext = security::encrypt_field(KEY, plaintext).expect("encrypt");
        let decrypted = security::decrypt_field(KEY, &ciphertext).expect("decrypt");
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn approval_thresholds_match_business_rules() {
        assert!(!workflows::requires_manager_approval(250_000, 5));
        assert!(workflows::requires_manager_approval(250_001, 1));
        assert!(workflows::requires_manager_approval(100_000, 6));
    }

    #[test]
    fn shipment_transitions_are_strict() {
        assert!(workflows::valid_shipment_transition("created", "packed"));
        assert!(workflows::valid_shipment_transition("packed", "shipped"));
        assert!(!workflows::valid_shipment_transition("created", "received"));
        assert!(!workflows::valid_shipment_transition("shipped", "completed"));
    }

    #[test]
    fn after_sales_transitions_are_strict() {
        assert!(workflows::valid_after_sales_transition("requested", "evidence_pending"));
        assert!(workflows::valid_after_sales_transition("under_review", "approved"));
        assert!(!workflows::valid_after_sales_transition("requested", "approved"));
        assert!(!workflows::valid_after_sales_transition("approved", "rejected"));
    }

    #[test]
    fn business_day_addition_skips_weekends() {
        let friday = Utc.with_ymd_and_hms(2026, 4, 3, 9, 0, 0).unwrap();
        let next_business = workflows::add_business_days(friday, 1);
        assert_eq!(next_business.date_naive().to_string(), "2026-04-06");
    }
}
