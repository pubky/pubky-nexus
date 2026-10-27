//! Bounded JSCalendar expansion at write/maintenance time, never on an API read.
use chrono::{
    DateTime, Datelike, Days, Duration, Months, NaiveDate, NaiveDateTime, Offset, TimeZone, Utc,
};
use chrono_tz::{GapInfo, Tz};
use pubky_social_specs::{jscalendar, PubkySocialEventContent, PubkySocialRecurrenceRule};
use std::collections::BTreeMap;

pub const MAX_OCCURRENCES: usize = 1_000;
pub const MAX_MONTHS: u32 = 36;
pub const WINDOW_MONTHS: u32 = 12;

pub fn local(value: &str) -> Result<NaiveDateTime, String> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").map_err(|e| e.to_string())
}

pub fn zone(event: &PubkySocialEventContent) -> Result<Tz, String> {
    event
        .time_zone
        .parse()
        .map_err(|_| format!("Unknown IANA time zone: {}", event.time_zone))
}

/// RFC 8984 uses the pre-transition offset for both ambiguous and missing wall times.
pub fn instant(zone: Tz, local: NaiveDateTime) -> Option<i64> {
    if let Some(value) = zone.from_local_datetime(&local).earliest() {
        return Some(value.timestamp_millis());
    }
    let (_, before) = GapInfo::new(&local, &zone)?.begin?;
    local
        .and_utc()
        .timestamp_millis()
        .checked_sub(i64::from(before.fix().local_minus_utc()) * 1_000)
}

pub fn timing(event: &PubkySocialEventContent, start: NaiveDateTime) -> Result<(i64, i64), String> {
    let tz = zone(event)?;
    let start_at = instant(tz, start).ok_or("Event start is outside the time-zone range")?;
    let duration = match event.duration.as_deref() {
        Some(value) => value,
        // An all-day event without an explicit duration lasts one local calendar day.
        None if event.show_without_time => "P1D",
        None => return Err("Event duration is required".into()),
    };
    let seconds = jscalendar::validate_duration("duration", duration)?;
    let calendar = duration
        .trim_start_matches('P')
        .split('T')
        .next()
        .unwrap_or("");
    let days = if let Some(weeks) = calendar.strip_suffix('W') {
        weeks.parse::<u64>().map_err(|e| e.to_string())? * 7
    } else if let Some(days) = calendar.strip_suffix('D') {
        days.parse::<u64>().map_err(|e| e.to_string())?
    } else {
        0
    };
    // Calendar days advance the local date; hours/minutes/seconds advance absolute time.
    let end_local = start
        .checked_add_days(Days::new(days))
        .ok_or("Event end overflows")?;
    let end_at = instant(tz, end_local)
        .ok_or("Event end is outside the time-zone range")?
        .checked_add(((seconds - days * 86_400) as i64) * 1_000)
        .ok_or("Event end overflows")?;
    Ok((start_at, end_at))
}

pub fn validate(event: &PubkySocialEventContent, now: DateTime<Utc>) -> Result<(), String> {
    let start = local(&event.start)?;
    for rule in &event.recurrence_rules {
        if !matches!(rule.frequency.wire_name(), "monthly" | "yearly")
            && rule.by_day.iter().any(|d| d.nth_of_period.is_some())
        {
            return Err("nthOfPeriod requires monthly or yearly recurrence".into());
        }
    }
    let (start_at, _) = timing(event, start)?;
    let horizon = now
        .checked_add_months(Months::new(MAX_MONTHS))
        .ok_or("Event horizon overflows")?;
    if start_at > horizon.timestamp_millis() {
        return Err("Event start is more than three years ahead".into());
    }
    Ok(())
}

fn weekday(value: &str) -> u32 {
    match value {
        "mo" => 0,
        "tu" => 1,
        "we" => 2,
        "th" => 3,
        "fr" => 4,
        "sa" => 5,
        _ => 6,
    }
}
fn month_days(date: NaiveDate) -> i64 {
    let first = date.with_day(1).unwrap();
    (first.checked_add_months(Months::new(1)).unwrap() - first).num_days()
}
fn week_start(date: NaiveDate, first: u32) -> NaiveDate {
    date - Duration::days(((date.weekday().num_days_from_monday() + 7 - first) % 7) as i64)
}
fn period(date: NaiveDate, rule: &PubkySocialRecurrenceRule) -> i64 {
    match rule.frequency.wire_name() {
        "yearly" => date.year() as i64,
        "monthly" => date.year() as i64 * 12 + date.month0() as i64,
        "weekly" => {
            week_start(
                date,
                rule.first_day_of_week
                    .as_ref()
                    .map_or(0, |d| weekday(d.wire_name())),
            )
            .num_days_from_ce() as i64
                / 7
        }
        _ => date.num_days_from_ce() as i64,
    }
}
fn matches_day(date: NaiveDate, start: NaiveDate, rule: &PubkySocialRecurrenceRule) -> bool {
    let freq = rule.frequency.wire_name();
    if (period(date, rule) - period(start, rule)) % rule.interval.unwrap_or(1) != 0 {
        return false;
    }
    if !rule.by_month.is_empty()
        && !rule
            .by_month
            .iter()
            .any(|m| m.parse::<u32>().ok() == Some(date.month()))
    {
        return false;
    }
    if !rule.by_month_day.is_empty()
        && !rule
            .by_month_day
            .iter()
            .any(|d| *d == date.day() as i64 || *d == date.day() as i64 - month_days(date) - 1)
    {
        return false;
    }
    if !rule.by_day.is_empty()
        && !rule.by_day.iter().any(|d| {
            if weekday(d.day.wire_name()) != date.weekday().num_days_from_monday() {
                return false;
            }
            let Some(nth) = d.nth_of_period else {
                return true;
            };
            let (day, days) = if freq == "yearly" && rule.by_month.is_empty() {
                (
                    date.ordinal() as i64,
                    NaiveDate::from_ymd_opt(date.year(), 12, 31)
                        .unwrap()
                        .ordinal() as i64,
                )
            } else {
                (date.day() as i64, month_days(date))
            };
            nth == (day - 1) / 7 + 1 || nth == -((days - day) / 7 + 1)
        })
    {
        return false;
    }
    if freq == "weekly" && rule.by_day.is_empty() && date.weekday() != start.weekday() {
        return false;
    }
    if rule.by_day.is_empty()
        && rule.by_month_day.is_empty()
        && matches!(freq, "monthly" | "yearly")
        && date.day() != start.day()
    {
        return false;
    }
    if freq == "yearly"
        && rule.by_month.is_empty()
        && (!rule.by_month_day.is_empty() || rule.by_day.is_empty())
        && date.month() != start.month()
    {
        return false;
    }
    true
}

/// All generated local keys over the bounded series lifetime. This is also used by
/// maintenance to distinguish a removed occurrence from one outside the current window.
pub fn generated(event: &PubkySocialEventContent) -> Result<Vec<NaiveDateTime>, String> {
    let start = local(&event.start)?;
    let mut starts = vec![start];
    for rule in &event.recurrence_rules {
        starts.extend(generated_rule(start, rule)?);
    }
    starts.sort_unstable();
    starts.dedup();
    // Each rule's COUNT is applied before EXDATE; the combined set is bounded too.
    starts.truncate(MAX_OCCURRENCES);
    starts.retain(|d| {
        !event
            .excluded_dates
            .contains(&d.format("%Y-%m-%dT%H:%M:%S").to_string())
    });
    Ok(starts)
}

fn generated_rule(
    start: NaiveDateTime,
    rule: &PubkySocialRecurrenceRule,
) -> Result<Vec<NaiveDateTime>, String> {
    let end = start
        .checked_add_months(Months::new(MAX_MONTHS))
        .ok_or("Recurrence horizon overflows")?;
    let until = rule
        .until
        .as_deref()
        .map(local)
        .transpose()?
        .unwrap_or(end)
        .min(end);
    let count = rule.count.unwrap_or(MAX_OCCURRENCES as i64) as usize;
    let mut periods: BTreeMap<i64, Vec<NaiveDateTime>> = BTreeMap::new();
    // Start at the beginning of the containing period so bySetPosition is evaluated on
    // the whole period before DTSTART truncates the first one.
    let first = match rule.frequency.wire_name() {
        "yearly" => NaiveDate::from_ymd_opt(start.year(), 1, 1).unwrap(),
        "monthly" => start.date().with_day(1).unwrap(),
        "weekly" => week_start(
            start.date(),
            rule.first_day_of_week
                .as_ref()
                .map_or(0, |d| weekday(d.wire_name())),
        ),
        _ => start.date(),
    };
    let period_end = match rule.frequency.wire_name() {
        "yearly" => NaiveDate::from_ymd_opt(until.year(), 12, 31).unwrap(),
        "monthly" => until
            .date()
            .with_day(month_days(until.date()) as u32)
            .unwrap(),
        "weekly" => {
            week_start(
                until.date(),
                rule.first_day_of_week
                    .as_ref()
                    .map_or(0, |d| weekday(d.wire_name())),
            ) + Duration::days(6)
        }
        _ => until.date(),
    };
    let mut date = first;
    while date <= period_end {
        if matches_day(date, start.date(), rule) {
            periods
                .entry(period(date, rule))
                .or_default()
                .push(date.and_time(start.time()));
        }
        date = date.succ_opt().ok_or("Recurrence date overflows")?;
    }
    let mut result = Vec::new();
    // DTSTART always belongs to the recurrence set, even if it does not match a rule.
    let mut candidates = vec![start];
    for values in periods.values() {
        for (i, value) in values.iter().enumerate() {
            if rule.by_set_position.is_empty()
                || rule
                    .by_set_position
                    .iter()
                    .any(|p| *p == i as i64 + 1 || *p == i as i64 - values.len() as i64)
            {
                candidates.push(*value);
            }
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    for value in candidates
        .into_iter()
        .filter(|d| *d >= start && *d <= until)
    {
        result.push(value);
        if result.len() >= count.min(MAX_OCCURRENCES) {
            break;
        }
    }
    Ok(result)
}

pub fn window_end(now: DateTime<Utc>) -> i64 {
    now.checked_add_months(Months::new(WINDOW_MONTHS))
        .expect("current date in range")
        .timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn event(start: &str, zone: &str, rule: serde_json::Value) -> PubkySocialEventContent {
        serde_json::from_value(json!({"title":"test", "start":start,"timeZone":zone,"duration":"PT1H","recurrenceRules":rule})).unwrap()
    }
    #[test]
    fn multiple_recurrence_rules_form_one_bounded_set() {
        let mut e = event(
            "2027-01-01T10:00:00",
            "UTC",
            json!([
                {"frequency":"daily", "count":2},
                {"frequency":"weekly", "count":2}
            ]),
        );
        assert_eq!(
            generated(&e)
                .unwrap()
                .iter()
                .map(|d| d.day())
                .collect::<Vec<_>>(),
            [1, 2, 8]
        );
        e.excluded_dates.push("2027-01-02T10:00:00".into());
        assert_eq!(
            generated(&e)
                .unwrap()
                .iter()
                .map(|d| d.day())
                .collect::<Vec<_>>(),
            [1, 8]
        );
        let e = event(
            "2027-01-01T10:00:00",
            "UTC",
            json!([
                {"frequency":"daily"}, {"frequency":"weekly"}
            ]),
        );
        let dates = generated(&e).unwrap();
        assert_eq!(dates.len(), MAX_OCCURRENCES);
        assert!(dates.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(dates.last().unwrap() <= &local("2030-01-01T10:00:00").unwrap());
    }

    #[test]
    fn timezone_dst_all_day_and_gaps() {
        let e = event(
            "2027-03-27T12:00:00",
            "Europe/Zurich",
            json!([{"frequency":"daily","count":3}]),
        );
        let dates = generated(&e).unwrap();
        let instants: Vec<_> = dates.iter().map(|d| timing(&e, *d).unwrap().0).collect();
        assert_eq!(instants[1] - instants[0], 23 * 3_600_000);
        let mut all_day = e.clone();
        all_day.start = "2027-03-28T00:00:00".into();
        all_day.show_without_time = true;
        all_day.duration = None;
        let (s, end) = timing(&all_day, local(&all_day.start).unwrap()).unwrap();
        assert_eq!(end - s, 23 * 3_600_000);
        let gap = event("2027-03-28T02:30:00", "Europe/Zurich", json!([]));
        assert!(validate(&gap, Utc::now()).is_ok());
        assert_eq!(
            timing(&gap, local(&gap.start).unwrap()).unwrap().0,
            local("2027-03-28T01:30:00")
                .unwrap()
                .and_utc()
                .timestamp_millis()
        );
        let fold = event("2027-10-31T02:30:00", "Europe/Zurich", json!([]));
        assert_eq!(
            timing(&fold, local(&fold.start).unwrap()).unwrap().0,
            local("2027-10-31T00:30:00")
                .unwrap()
                .and_utc()
                .timestamp_millis()
        );
        for (duration, hours) in [("P1D", 23), ("PT24H", 24), ("P1DT2H", 25), ("P1W", 167)] {
            all_day.duration = Some(duration.into());
            let (s, e) = timing(&all_day, local(&all_day.start).unwrap()).unwrap();
            assert_eq!(e - s, hours * 3_600_000, "{duration}");
        }
        let repeat = event(
            "2027-03-27T02:30:00",
            "Europe/Zurich",
            json!([{"frequency":"daily","count":3}]),
        );
        assert_eq!(generated(&repeat).unwrap()[1].day(), 28);
        let mut bad = e;
        bad.time_zone = "No/SuchZone".into();
        assert!(validate(&bad, Utc::now()).is_err());
    }
    #[test]
    fn recurrence_filters_positions_exclusions_and_bounds() {
        let e = event(
            "2027-01-12T18:30:00",
            "UTC",
            json!([{"frequency":"monthly","byDay":[{"day":"tu","nthOfPeriod":2}],"count":4}]),
        );
        let days: Vec<_> = generated(&e)
            .unwrap()
            .iter()
            .map(|d| d.format("%Y-%m-%d").to_string())
            .collect();
        assert_eq!(
            days,
            ["2027-01-12", "2027-02-09", "2027-03-09", "2027-04-13"]
        );
        let mut e = event(
            "2027-01-01T10:00:00",
            "UTC",
            json!([{"frequency":"monthly","byDay":[{"day":"fr"}],"bySetPosition":[-1],"until":"2027-02-15T10:00:00"}]),
        );
        assert_eq!(generated(&e).unwrap().len(), 2); // DTSTART and January's last Friday; not February 12
        e.excluded_dates.push(e.start.clone());
        assert_eq!(generated(&e).unwrap()[0].day(), 29);
        let e = event("2027-01-01T10:00:00", "UTC", json!([{"frequency":"daily"}]));
        assert_eq!(generated(&e).unwrap().len(), MAX_OCCURRENCES);
        let e = event(
            "2027-01-01T10:00:00",
            "UTC",
            json!([{"frequency":"yearly"}]),
        );
        assert_eq!(generated(&e).unwrap().len(), 4);
        let e = event(
            "2027-01-31T10:00:00",
            "UTC",
            json!([{"frequency":"monthly","count":3}]),
        );
        assert_eq!(
            generated(&e)
                .unwrap()
                .iter()
                .map(|d| d.month())
                .collect::<Vec<_>>(),
            [1, 3, 5]
        );
    }
}
