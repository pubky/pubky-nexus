use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::db::connectors::redis::get_redis_conn;

pub enum TimeSeriesAggFunc {
    AVERAGE,
    SUM,
    MIN,
    MAX,
    COUNT,
}

impl FromStr for TimeSeriesAggFunc {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let enum_value = match s {
            "avg" => TimeSeriesAggFunc::AVERAGE,
            "sum" => TimeSeriesAggFunc::SUM,
            "min" => TimeSeriesAggFunc::MIN,
            "max" => TimeSeriesAggFunc::MAX,
            "count" => TimeSeriesAggFunc::COUNT,
            _ => return Err(format!("aggregation function {s} not supported.")),
        };
        Ok(enum_value)
    }
}

impl ToString for TimeSeriesAggFunc {
    fn to_string(&self) -> String {
        let str_value = match self {
            TimeSeriesAggFunc::AVERAGE => "avg",
            TimeSeriesAggFunc::SUM => "sum",
            TimeSeriesAggFunc::MIN => "min",
            TimeSeriesAggFunc::MAX => "max",
            TimeSeriesAggFunc::COUNT => "count",
        };
        return str_value.to_string();
    }
}

// TimeSeries struct to hold the Redis client and key prefix
pub struct TimeSeries {
    prefix: String,
}

// TimeSeriesData to represent the data points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesData {
    timestamp: DateTime<Utc>,
    value: f64,
}

impl TimeSeries {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }

    // Add a data point to the time series
    pub async fn add(
        &self,
        key: &str,
        timestamp: Option<DateTime<Utc>>,
        value: f64,
        labels: Option<Vec<&str>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = get_redis_conn().await?;

        let timestamp_value = match timestamp {
            Some(value) => value.timestamp_millis().to_string(),
            None => "*".to_string(),
        };

        let full_key = format!("{}:{}", self.prefix, key);

        let mut cmd = redis::cmd("TS.ADD");
        cmd.arg(&full_key).arg(timestamp_value).arg(value);

        if let Some(label_filters) = labels {
            cmd.arg("FILTER");
            for label in label_filters {
                cmd.arg(label);
            }
        }

        cmd.query_async(&mut conn).await?;
        Ok(())
    }

    // Get data points within a time range, optionally perform aggregation
    pub async fn range(
        &self,
        key: Option<&str>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        labels: Option<Vec<&str>>,
        aggregation: Option<TimeSeriesAggFunc>,
        bucket_size_ms: Option<i64>,
    ) -> Result<Vec<TimeSeriesData>, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = get_redis_conn().await?;

        let cmd_string = match key {
            Some(_) => "TS.RANGE",
            None => "TS.MRANGE",
        };

        let mut cmd = redis::cmd(&cmd_string);

        if key.is_some() {
            cmd.arg(format!("{}:{}", self.prefix, key.unwrap()));
        }

        let start_timestamp = match start {
            None => "-".to_string(),
            Some(start_value) => start_value.timestamp_millis().to_string(),
        };

        let end_timestamp = match end {
            None => "+".to_string(),
            Some(end_value) => end_value.timestamp_millis().to_string(),
        };

        cmd.arg(start_timestamp).arg(end_timestamp);

        if let Some(label_filters) = labels {
            cmd.arg("FILTER");
            for label in label_filters {
                cmd.arg(label);
            }
        }

        if let Some(agg_function) = aggregation {
            cmd.arg("AGGREGATION").arg(agg_function.to_string());
            let bucket_size = bucket_size_ms.unwrap_or(0).to_string();
            cmd.arg(bucket_size);
        }

        let result: Vec<(i64, f64)> = cmd.query_async(&mut conn).await?;

        Ok(result
            .into_iter()
            .map(|(ts, val)| TimeSeriesData {
                timestamp: DateTime::from_timestamp_millis(ts).unwrap(),
                value: val,
            })
            .collect())
    }
}
