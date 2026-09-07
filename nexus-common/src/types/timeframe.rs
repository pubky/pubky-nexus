use serde::Deserialize;
use std::fmt::Display;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, ToSchema, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Timeframe {
    Today,
    ThisWeek,
    ThisMonth,
    AllTime,
}

impl Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Timeframe::Today => write!(f, "Today"),
            Timeframe::ThisWeek => write!(f, "ThisWeek"),
            Timeframe::ThisMonth => write!(f, "ThisMonth"),
            Timeframe::AllTime => write!(f, "AllTime"),
        }
    }
}

impl Timeframe {
    pub fn to_timestamp_range(&self) -> (i64, i64) {
        let now = chrono::Utc::now();
        let start = match self {
            Timeframe::Today => (now - chrono::Duration::hours(24)).timestamp_millis(),
            Timeframe::ThisWeek => (now - chrono::Duration::days(7)).timestamp_millis(),
            Timeframe::ThisMonth => (now - chrono::Duration::days(30)).timestamp_millis(),
            Timeframe::AllTime => 0,
        };
        (start, now.timestamp_millis())
    }

    pub fn to_cache_period(&self) -> i64 {
        match self {
            Timeframe::Today => 60 * 60,
            Timeframe::ThisWeek => 60 * 60 * 6,
            Timeframe::ThisMonth => 60 * 60 * 24,
            Timeframe::AllTime => 60 * 60 * 24,
        }
    }
}

/// The timeframes whose global ranking is served from a TTL cache.
///
/// `Timeframe::AllTime` is deliberately absent: it is served from the incrementally
/// maintained `Sorted:Users:Influencers` index, which has no cache key and no TTL,
/// so a cache write for it would be an orphan nobody reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTimeframe {
    Today,
    ThisWeek,
    ThisMonth,
}

impl CacheTimeframe {
    pub const ALL: [CacheTimeframe; 3] = [
        CacheTimeframe::Today,
        CacheTimeframe::ThisWeek,
        CacheTimeframe::ThisMonth,
    ];

    /// `None` for `AllTime`, which has no cache.
    pub fn from_timeframe(timeframe: &Timeframe) -> Option<Self> {
        match timeframe {
            Timeframe::Today => Some(CacheTimeframe::Today),
            Timeframe::ThisWeek => Some(CacheTimeframe::ThisWeek),
            Timeframe::ThisMonth => Some(CacheTimeframe::ThisMonth),
            Timeframe::AllTime => None,
        }
    }

    pub fn timeframe(self) -> Timeframe {
        match self {
            CacheTimeframe::Today => Timeframe::Today,
            CacheTimeframe::ThisWeek => Timeframe::ThisWeek,
            CacheTimeframe::ThisMonth => Timeframe::ThisMonth,
        }
    }

    /// TTL in seconds of the cache key for this timeframe.
    pub fn to_cache_period(self) -> i64 {
        self.timeframe().to_cache_period()
    }
}

impl Display for CacheTimeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.timeframe().fmt(f)
    }
}
