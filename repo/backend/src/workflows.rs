use chrono::{Datelike, Duration, NaiveDate, Timelike, Utc, Weekday};

pub fn requires_manager_approval(total_value_cents: i64, scrap_units: i64) -> bool {
    total_value_cents > 250_000 || scrap_units > 5
}

pub fn valid_shipment_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("created", "packed")
            | ("packed", "shipped")
            | ("shipped", "received")
            | ("received", "completed")
            | ("created", "canceled")
            | ("packed", "canceled")
    )
}

pub fn valid_after_sales_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("requested", "evidence_pending")
            | ("requested", "under_review")
            | ("evidence_pending", "under_review")
            | ("under_review", "approved")
            | ("under_review", "rejected")
            | ("approved", "closed")
            | ("rejected", "closed")
    )
}

pub fn add_business_days(start: chrono::DateTime<Utc>, business_days: i64) -> chrono::DateTime<Utc> {
    let mut date: NaiveDate = start.date_naive();
    let mut added = 0;
    while added < business_days {
        date = date.succ_opt().unwrap_or(date);
        if !matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
            added += 1;
        }
    }
    start
        .date_naive()
        .and_hms_opt(start.hour(), start.minute(), start.second())
        .unwrap()
        .and_utc()
        + Duration::days((date - start.date_naive()).num_days())
}
