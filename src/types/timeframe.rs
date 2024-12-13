use chrono::Datelike;
use serde::Deserialize;
use std::fmt::Display;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, ToSchema, Clone)]
#[serde(rename_all = "snake_case")]
pub enum Timeframe {
    Today,
    ThisMonth,
    AllTime,
}

impl Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Timeframe::Today => write!(f, "Today"),
            Timeframe::ThisMonth => write!(f, "ThisMonth"),
            Timeframe::AllTime => write!(f, "AllTime"),
        }
    }
}

impl Timeframe {
    pub fn to_timestamp_range(&self) -> (i64, i64) {
        let now = chrono::Utc::now();
        let start = match self {
            Timeframe::Today => now
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp_millis(),
            Timeframe::ThisMonth => now
                .date_naive()
                .with_day(1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp_millis(),
            Timeframe::AllTime => 0,
        };
        (start, now.timestamp_millis())
    }

    pub fn to_cache_period(&self) -> i64 {
        match self {
            Timeframe::Today => 60 * 60,
            Timeframe::ThisMonth => 60 * 60 * 24,
            Timeframe::AllTime => 60 * 60 * 24,
        }
    }
}
