//! Calendar estimate v4. Dates are civil dates, never timestamps.
//! See docs/CYCLE_ALGORITHM.md for sources, counting conventions and limitations.
use serde::{Deserialize, Serialize};
use time::{Date, Duration, Month};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CycleSettings {
    pub cycle_length: Option<i64>,
    pub period_length: i64,
    pub context: String,
    pub length_method: String,
}
impl Default for CycleSettings {
    fn default() -> Self {
        Self {
            cycle_length: None,
            period_length: 5,
            context: "regular".into(),
            length_method: "auto".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Period {
    pub id: String,
    pub start_date: String,
    pub end_date: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CycleEstimate {
    pub kind: &'static str,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub ovulation_date: Option<String>,
    pub fertile_start: Option<String>,
    pub fertile_end: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Prediction {
    pub algorithm_version: &'static str,
    pub status: &'static str,
    pub reason: Option<&'static str>,
    pub source: Option<&'static str>,
    pub cycle_length: Option<i64>,
    pub period_length: i64,
    pub next_period_start: Option<String>,
    pub next_period_end: Option<String>,
    pub ovulation_date: Option<String>,
    pub fertile_start: Option<String>,
    pub fertile_end: Option<String>,
    pub interval_count: usize,
    pub variability_days: Option<i64>,
    pub additional_cycles: Vec<CycleEstimate>,
}

pub(crate) fn parse_date(value: &str) -> Option<Date> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return None;
    }
    let year: i32 = value[0..4].parse().ok()?;
    if !(2000..=2100).contains(&year) {
        return None;
    }
    let month = Month::try_from(value[5..7].parse::<u8>().ok()?).ok()?;
    Date::from_calendar_date(year, month, value[8..10].parse().ok()?).ok()
}
pub(crate) fn date_string(value: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        value.year(),
        u8::from(value.month()),
        value.day()
    )
}

/// Integer median, rounded half up for an even count; inputs are small positive day counts.
fn median(values: &mut [i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid] + 1) / 2
    } else {
        values[mid]
    })
}

// ASRM calendar convention: day L-14 is 1-based. Expand the standard
// six-day interval by one day before and four after for timing uncertainty.
fn fertility(start: Date, length: i64) -> (String, String, String) {
    let ovulation = start + Duration::days(length - 15);
    let first = (ovulation - Duration::days(6)).max(start);
    let last = (ovulation + Duration::days(4)).min(start + Duration::days(length - 1));
    (
        date_string(ovulation),
        date_string(first),
        date_string(last),
    )
}

pub(crate) fn predict(settings: &CycleSettings, periods: &[Period], as_of: Date) -> Prediction {
    let mut output = Prediction {
        algorithm_version: "calendar-v4",
        status: "insufficient_data",
        reason: Some("no_periods"),
        source: None,
        cycle_length: None,
        period_length: settings.period_length.clamp(1, 14),
        next_period_start: None,
        next_period_end: None,
        ovulation_date: None,
        fertile_start: None,
        fertile_end: None,
        interval_count: 0,
        variability_days: None,
        additional_cycles: Vec::new(),
    };

    // Invalid dates cannot become facts or silently distort the history.
    let mut history = Vec::with_capacity(periods.len());
    for period in periods {
        let Some(start) = parse_date(&period.start_date) else {
            output.reason = Some("invalid_history");
            return output;
        };
        let end = match &period.end_date {
            Some(value) => match parse_date(value) {
                Some(end) if end >= start && end <= as_of => Some(end),
                _ => {
                    output.reason = Some("invalid_history");
                    return output;
                }
            },
            None => None,
        };
        if start > as_of {
            output.reason = Some("invalid_history");
            return output;
        }
        history.push((start, end));
    }
    history.sort_unstable_by_key(|(start, _)| *start);
    if history
        .windows(2)
        .any(|pair| pair[0].1.unwrap_or(as_of) >= pair[1].0)
    {
        output.reason = Some("invalid_history");
        return output;
    }
    let Some(&(last_start, last_end)) = history.last() else {
        return output;
    };

    let cutoff = last_start - Duration::days(365);
    let recent: Vec<_> = history
        .windows(2)
        .rev()
        .filter(|pair| pair[0].0 >= cutoff)
        .take(12)
        .map(|pair| (pair[1].0 - pair[0].0).whole_days())
        .collect();
    // Very short intervals and long gaps remain in the log but do not train the estimate.
    let mut intervals: Vec<_> = recent
        .iter()
        .copied()
        .filter(|length| (10..=90).contains(length))
        .collect();
    output.interval_count = intervals.len();
    let stability = &recent[..recent.len().min(6)];
    output.variability_days = stability
        .iter()
        .max()
        .zip(stability.iter().min())
        .map(|(max, min)| max - min);

    let mut durations: Vec<i64> = history
        .iter()
        .rev()
        .filter(|(start, _)| *start >= cutoff)
        .filter_map(|(start, end)| end.map(|end| (end - *start).whole_days() + 1))
        .filter(|length| (1..=14).contains(length))
        .take(6)
        .collect();
    if settings.length_method == "auto" {
        output.period_length = median(&mut durations).unwrap_or(output.period_length);
    }

    if !matches!(settings.context.as_str(), "regular" | "irregular") {
        output.status = "paused";
        output.reason = Some("context_paused");
        return output;
    }

    // Prefer an explicit length over a lone observed interval, then adapt to logged history.
    let (length, source) = if settings.length_method == "manual" || intervals.len() < 2 {
        match settings.cycle_length {
            Some(length) => (length, "user"),
            None => match median(&mut intervals) {
                Some(length) => (length, "history"),
                None => (28, "baseline"),
            },
        }
    } else {
        (median(&mut intervals).unwrap_or(28), "history")
    };
    // Retrospective windows use two actual starts, never projected anchors.
    if settings.context == "regular" {
        for pair in history.windows(2) {
            let (start, end) = pair[0];
            let length = (pair[1].0 - start).whole_days();
            if !(21..=35).contains(&length)
                || end.is_some_and(|end| (end - start).whole_days() >= 14)
            {
                continue;
            }
            let (ovulation, first, last) = fertility(start, length);
            output.additional_cycles.push(CycleEstimate {
                kind: "retrospective",
                period_start: None,
                period_end: None,
                ovulation_date: Some(ovulation),
                fertile_start: Some(first),
                fertile_end: Some(last),
            });
        }
    }

    output.source = Some(source);
    output.cycle_length = Some(length);
    let next_start = last_start + Duration::days(length);
    let next_end = next_start + Duration::days(output.period_length - 1);
    output.next_period_start = Some(date_string(next_start));
    output.next_period_end = Some(date_string(next_end));
    if as_of > next_start {
        output.status = "overdue";
        output.reason = Some("period_overdue");
        return output;
    }

    let fertility_uncertain = settings.context == "irregular"
        || !(21..=35).contains(&length)
        || stability.iter().any(|length| !(21..=35).contains(length))
        || output.variability_days.is_some_and(|spread| spread > 7)
        || (last_end.unwrap_or(as_of) - last_start).whole_days() + 1 > 14;

    for step in 0..3 {
        let start = next_start + Duration::days(step * length);
        let end = start + Duration::days(output.period_length - 1);
        let mut cycle = CycleEstimate {
            kind: "projected",
            period_start: Some(date_string(start)),
            period_end: Some(date_string(end)),
            ovulation_date: None,
            fertile_start: None,
            fertile_end: None,
        };
        if !fertility_uncertain {
            let (ovulation, first, last) = fertility(start, length);
            cycle.ovulation_date = Some(ovulation);
            cycle.fertile_start = Some(first);
            cycle.fertile_end = Some(last);
        }
        output.additional_cycles.push(cycle);
    }

    if fertility_uncertain {
        output.status = "irregular";
        output.reason = Some("fertility_uncertain");
        return output;
    }

    let (ovulation, first, last) = fertility(last_start, length);
    output.ovulation_date = Some(ovulation);
    output.fertile_start = Some(first);
    output.fertile_end = Some(last);
    output.status = "estimated";
    output.reason = None;
    output
}
