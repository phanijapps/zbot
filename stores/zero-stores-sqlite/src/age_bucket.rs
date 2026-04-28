use chrono::{DateTime, Duration, Utc};

/// Classify a timestamp into a human-meaningful recency bucket relative to `now`.
/// Returns one of: "today", "last_7_days", "historical".
pub fn age_bucket(now: DateTime<Utc>, created_at: DateTime<Utc>) -> &'static str {
    let age = now.signed_duration_since(created_at);
    if age < Duration::hours(24) {
        "today"
    } else if age < Duration::days(7) {
        "last_7_days"
    } else {
        "historical"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn twelve_hours_ago_is_today() {
        let t = now() - Duration::hours(12);
        assert_eq!(age_bucket(now(), t), "today");
    }

    #[test]
    fn three_days_ago_is_last_7_days() {
        let t = now() - Duration::days(3);
        assert_eq!(age_bucket(now(), t), "last_7_days");
    }

    #[test]
    fn thirty_days_ago_is_historical() {
        let t = now() - Duration::days(30);
        assert_eq!(age_bucket(now(), t), "historical");
    }

    #[test]
    fn exactly_seven_days_is_historical() {
        let t = now() - Duration::days(7);
        assert_eq!(age_bucket(now(), t), "historical");
    }
}
