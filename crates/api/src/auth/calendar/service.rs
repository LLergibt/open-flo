use super::*;
use std::collections::HashSet;

async fn tracker_transaction<'a>(
    state: &'a AuthState,
    actor: &Authenticated,
) -> Result<Transaction<'a, Sqlite>, AuthError> {
    let mut tx = actor_transaction(state, actor, false).await?;
    let live = store::live_actor(&mut tx, actor).await?;
    if live.role != "tracker" {
        return Err(AuthError::forbidden());
    }
    Ok(tx)
}
pub(super) async fn calendar(
    state: &AuthState,
    actor: &Authenticated,
    today: Date,
) -> Result<CalendarData, AuthError> {
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AuthError::internal())?;
    let live = store::live_actor(&mut tx, actor).await?;
    let viewer_id = live.id.clone();
    let (owner, access) = match live.role.as_str() {
        "tracker" => (live, "owner"),
        "partner" => (
            store::tracker_for_partner(&mut tx, &live.id).await?,
            "partner",
        ),
        _ => return Err(AuthError::forbidden()),
    };
    let settings = store::settings(&mut tx, &owner.id).await?;
    let periods = store::periods(&mut tx, &owner.id).await?;
    let entries = store::diary(&mut tx, &owner.id).await?;
    let pair = store::active_pair_id(&mut tx, &owner.id).await?;
    let partner_entries = match &pair {
        Some(id) => store::partner_diary(&mut tx, id).await?,
        None => Vec::new(),
    };
    let mut prediction = serde_json::to_value(predict(&settings, &periods, today))
        .map_err(|_| AuthError::internal())?;
    if access == "partner" {
        // A pause reason may contain pregnancy/contraception context, which is private.
        if !prediction["reason"].is_null() {
            prediction["reason"] = serde_json::json!("prediction_unavailable");
        }
    }
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(CalendarData {
        access,
        viewer_id,
        owner_display_name: owner.display_name,
        as_of: date_string(today),
        periods,
        entries,
        partner_entries,
        partner_diary_enabled: pair.is_some(),
        partner_pair_id: pair.filter(|_| access == "partner"),
        prediction,
        settings: if access == "owner" {
            Some(settings)
        } else {
            None
        },
    })
}
pub(super) async fn settings(
    state: &AuthState,
    actor: &Authenticated,
    input: SettingsInput,
) -> Result<(), AuthError> {
    if input
        .cycle_length
        .is_some_and(|length| !(10..=90).contains(&length))
        || !(1..=14).contains(&input.period_length)
        || ![
            "regular",
            "irregular",
            "pregnancy",
            "postpartum",
            "hormonal_contraception",
            "paused",
        ]
        .contains(&input.context.as_str())
        || !["auto", "manual"].contains(&input.length_method.as_str())
        || input.length_method == "manual" && input.cycle_length.is_none()
    {
        return Err(AuthError::invalid());
    }
    let mut tx = tracker_transaction(state, actor).await?;
    store::save_settings(&mut tx, &actor.user.id, &input).await?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(())
}
fn diary_date(value: &str, today: Date) -> Result<Date, AuthError> {
    let date = parse_date(value).ok_or_else(AuthError::invalid)?;
    if date > today || !(2000..=2100).contains(&date.year()) || date_string(date) != value {
        return Err(AuthError::invalid());
    }
    Ok(date)
}
fn validate_period(period: &Period, today: Date) -> Result<(), AuthError> {
    let start = diary_date(&period.start_date, today)?;
    let end = match &period.end_date {
        Some(value) => diary_date(value, today)?,
        None => today,
    };
    if end < start || (end - start).whole_days() >= 90 {
        return Err(AuthError::invalid());
    }
    Ok(())
}
fn validate_history(periods: &[Period], today: Date) -> Result<(), AuthError> {
    let mut previous_end: Option<Date> = None;
    for (index, period) in periods.iter().enumerate() {
        let start = parse_date(&period.start_date).ok_or_else(AuthError::internal)?;
        let end = period
            .end_date
            .as_deref()
            .and_then(parse_date)
            .unwrap_or(today);
        if previous_end.is_some_and(|previous| start <= previous)
            || period.end_date.is_none() && index + 1 != periods.len()
        {
            return Err(AuthError::conflict());
        }
        previous_end = Some(end);
    }
    Ok(())
}
pub(super) async fn save_period(
    state: &AuthState,
    actor: &Authenticated,
    id: Option<String>,
    input: PeriodInput,
    today: Date,
) -> Result<Period, AuthError> {
    let editing = id.is_some();
    let period = Period {
        id: id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        start_date: input.start_date,
        end_date: input.end_date,
    };
    validate_period(&period, today)?;
    let mut tx = tracker_transaction(state, actor).await?;
    let mut history = store::periods(&mut tx, &actor.user.id).await?;
    if editing && !history.iter().any(|item| item.id == period.id) {
        return Err(not_found());
    }
    if !editing && history.len() >= 500 {
        return Err(AuthError::conflict());
    }
    history.retain(|item| item.id != period.id);
    history.push(Period {
        id: period.id.clone(),
        start_date: period.start_date.clone(),
        end_date: period.end_date.clone(),
    });
    history.sort_by(|a, b| a.start_date.cmp(&b.start_date));
    validate_history(&history, today)?;
    store::save_period(&mut tx, &actor.user.id, &period).await?;
    tx.commit().await.map_err(conflict_or_internal)?;
    Ok(period)
}
pub(super) async fn delete_period(
    state: &AuthState,
    actor: &Authenticated,
    id: &str,
) -> Result<(), AuthError> {
    let mut tx = tracker_transaction(state, actor).await?;
    store::delete_period(&mut tx, &actor.user.id, id).await?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(())
}
#[derive(Deserialize)]
struct DiaryCategory {
    field: String,
    options: Vec<(String, String)>,
    #[serde(default)]
    owner_only: bool,
    #[serde(default)]
    owner_only_options: HashSet<String>,
}

fn diary_catalog() -> &'static [DiaryCategory] {
    static CATALOG: std::sync::OnceLock<Vec<DiaryCategory>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../apps/web/src/lib/diary-catalog.json"
        ))
        .expect("bundled diary catalog is valid")
    })
}

fn enum_values(values: &[String], field: &str, owner: bool) -> bool {
    let allowed: HashSet<&str> = diary_catalog()
        .iter()
        .filter(|category| category.field == field && (owner || !category.owner_only))
        .flat_map(|category| {
            category
                .options
                .iter()
                .map(|(id, _)| id)
                .filter(move |id| owner || !category.owner_only_options.contains(*id))
                .map(String::as_str)
        })
        .collect();
    values.len() <= allowed.len()
        && values.iter().all(|value| allowed.contains(value.as_str()))
        && values.iter().collect::<HashSet<_>>().len() == values.len()
}

fn valid_diary_fields(
    symptoms: &[String],
    events: &[String],
    mood: &str,
    pain: i64,
    notes: &str,
    owner: bool,
) -> bool {
    enum_values(symptoms, "symptoms", owner)
        && enum_values(events, "events", owner)
        && ["", "calm", "happy", "sad", "anxious", "irritable", "energetic"].contains(&mood)
        && (0..=3).contains(&pain)
        && notes.chars().count() <= 2000
}

fn validate_diary(input: &DiaryInput) -> Result<(), AuthError> {
    if !valid_diary_fields(&input.symptoms, &input.events, &input.mood, input.pain, &input.notes, true)
        || !["", "spotting", "light", "medium", "heavy"].contains(&input.flow.as_str())
    {
        return Err(AuthError::invalid());
    }
    Ok(())
}

pub(super) async fn save_diary(
    state: &AuthState,
    actor: &Authenticated,
    date: &str,
    input: DiaryInput,
    today: Date,
) -> Result<(), AuthError> {
    diary_date(date, today)?;
    validate_diary(&input)?;
    let mut tx = tracker_transaction(state, actor).await?;
    let (count, exists) = store::diary_size(&mut tx, &actor.user.id, date).await?;
    if !exists && count >= 1000 {
        return Err(AuthError::conflict());
    }
    store::save_diary(&mut tx, &actor.user.id, date, &input).await?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(())
}
pub(super) async fn delete_diary(
    state: &AuthState,
    actor: &Authenticated,
    date: &str,
    today: Date,
) -> Result<(), AuthError> {
    diary_date(date, today)?;
    let mut tx = tracker_transaction(state, actor).await?;
    store::delete_diary(&mut tx, &actor.user.id, date).await?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(())
}

pub(super) async fn change_partner_diary(
    state: &AuthState,
    actor: &Authenticated,
    expected_pair: &str,
    date: &str,
    input: Option<PartnerDiaryInput>,
    today: Date,
) -> Result<(), AuthError> {
    diary_date(date, today)?;
    if input.as_ref().is_some_and(|entry| {
        let symptoms = entry.symptoms.as_deref().unwrap_or_default();
        let events = entry.events.as_deref().unwrap_or_default();
        let pain = entry.pain.unwrap_or_default();
        !valid_diary_fields(symptoms, events, &entry.mood, pain, &entry.notes, false)
            || (symptoms.is_empty() && events.is_empty() && pain == 0
                && entry.mood.is_empty() && entry.notes.trim().is_empty())
    }) {
        return Err(AuthError::invalid());
    }
    let mut tx = actor_transaction(state, actor, false).await?;
    let live = store::live_actor(&mut tx, actor).await?;
    if live.role != "partner" {
        return Err(AuthError::forbidden());
    }
    let owner = store::tracker_for_partner(&mut tx, &live.id).await?;
    let pair = store::active_pair_id(&mut tx, &owner.id).await?.ok_or_else(AuthError::forbidden)?;
    if pair != expected_pair {
        return Err(AuthError::forbidden());
    }
    store::change_partner_diary(&mut tx, &pair, date, input).await?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(())
}
