use super::*;
use crate::cycle::{CycleSettings, Period, date_string, parse_date, predict};
use time::Date;

mod service;
mod store;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SettingsInput {
    cycle_length: Option<i64>,
    period_length: i64,
    context: String,
    length_method: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PeriodInput {
    start_date: String,
    end_date: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiaryInput {
    symptoms: Vec<String>,
    events: Vec<String>,
    mood: String,
    flow: String,
    pain: i64,
    notes: String,
}

#[derive(Serialize)]
pub(super) struct DiaryEntry {
    date: String,
    #[serde(flatten)]
    data: DiaryInput,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PartnerDiaryInput {
    symptoms: Option<Vec<String>>,
    events: Option<Vec<String>>,
    mood: String,
    pain: Option<i64>,
    notes: String,
}

#[derive(Serialize)]
pub(super) struct PartnerDiaryEntry {
    date: String,
    #[serde(flatten)]
    data: PartnerDiaryInput,
}

#[derive(Serialize)]
pub(super) struct CalendarData {
    access: &'static str,
    viewer_id: String,
    owner_display_name: String,
    as_of: String,
    periods: Vec<Period>,
    entries: Vec<DiaryEntry>,
    partner_entries: Vec<PartnerDiaryEntry>,
    partner_diary_enabled: bool,
    partner_pair_id: Option<String>,
    prediction: serde_json::Value,
    settings: Option<CycleSettings>,
}

fn request_date(headers: &HeaderMap) -> Result<Date, AuthError> {
    let utc = OffsetDateTime::now_utc().date();
    if headers.get_all("x-calendar-date").iter().count() > 1 {
        return Err(AuthError::invalid());
    }
    let Some(value) = headers.get("x-calendar-date") else {
        return Ok(utc);
    };
    let day = value
        .to_str()
        .ok()
        .and_then(parse_date)
        .ok_or_else(AuthError::invalid)?;
    if (day - utc).whole_days().abs() > 1 {
        return Err(AuthError::invalid());
    }
    Ok(day)
}

fn require_tracker(actor: &Authenticated) -> Result<(), AuthError> {
    if actor.user.role == "tracker" {
        Ok(())
    } else {
        Err(AuthError::forbidden())
    }
}

fn verify_viewer(headers: &HeaderMap, actor: &Authenticated) -> Result<(), AuthError> {
    if let Some(viewer) = headers.get("x-calendar-viewer") {
        if viewer.to_str().ok() != Some(actor.user.id.as_str()) {
            return Err(AuthError::forbidden());
        }
    }
    Ok(())
}

pub(super) async fn calendar(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<Json<CalendarData>, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    verify_viewer(&headers, &actor)?;
    let today = request_date(&headers)?;
    Ok(Json(service::calendar(&state, &actor, today).await?))
}

pub(super) async fn settings(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<SettingsInput>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    verify_viewer(&headers, &actor)?;
    require_tracker(&actor)?;
    check_origin(&headers, &state.config, false)?;
    service::settings(&state, &actor, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn create_period(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<PeriodInput>,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    verify_viewer(&headers, &actor)?;
    require_tracker(&actor)?;
    check_origin(&headers, &state.config, false)?;
    let today = request_date(&headers)?;
    let period = service::save_period(&state, &actor, None, input, today).await?;
    Ok((StatusCode::CREATED, Json(period)).into_response())
}

pub(super) async fn update_period(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    ApiJson(input): ApiJson<PeriodInput>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    verify_viewer(&headers, &actor)?;
    require_tracker(&actor)?;
    check_origin(&headers, &state.config, false)?;
    valid_id(&id)?;
    service::save_period(&state, &actor, Some(id), input, request_date(&headers)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn delete_period(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    verify_viewer(&headers, &actor)?;
    require_tracker(&actor)?;
    check_origin(&headers, &state.config, false)?;
    valid_id(&id)?;
    service::delete_period(&state, &actor, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn save_diary(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path(date): Path<String>,
    ApiJson(input): ApiJson<DiaryInput>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    verify_viewer(&headers, &actor)?;
    require_tracker(&actor)?;
    check_origin(&headers, &state.config, false)?;
    service::save_diary(&state, &actor, &date, input, request_date(&headers)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn delete_diary(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path(date): Path<String>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    verify_viewer(&headers, &actor)?;
    require_tracker(&actor)?;
    check_origin(&headers, &state.config, false)?;
    service::delete_diary(&state, &actor, &date, request_date(&headers)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn save_partner_diary(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path((pair_id, date)): Path<(String, String)>,
    ApiJson(input): ApiJson<PartnerDiaryInput>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    verify_viewer(&headers, &actor)?;
    check_origin(&headers, &state.config, false)?;
    service::change_partner_diary(&state, &actor, &pair_id, &date, Some(input), request_date(&headers)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn delete_partner_diary(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path((pair_id, date)): Path<(String, String)>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    verify_viewer(&headers, &actor)?;
    check_origin(&headers, &state.config, false)?;
    service::change_partner_diary(&state, &actor, &pair_id, &date, None, request_date(&headers)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn not_found() -> AuthError {
    AuthError {
        status: StatusCode::NOT_FOUND,
        error: "not_found",
    }
}
