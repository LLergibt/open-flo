use super::*;

#[derive(FromRow)]
struct PeriodRow {
    id: String,
    start_date: String,
    end_date: Option<String>,
}
#[derive(FromRow)]
struct SettingsRow {
    cycle_length: Option<i64>,
    period_length: i64,
    context: String,
    length_method: String,
}
#[derive(FromRow)]
struct DiaryRow {
    date: String,
    symptoms: String,
    events: String,
    mood: String,
    flow: String,
    pain: i64,
    notes: String,
}

#[derive(FromRow)]
struct PartnerDiaryRow {
    date: String,
    symptoms: String,
    events: String,
    mood: String,
    pain: i64,
    notes: String,
}

pub(super) async fn live_actor(
    tx: &mut Transaction<'_, Sqlite>,
    actor: &Authenticated,
) -> Result<UserRecord, AuthError> {
    sqlx::query_as::<_,UserRecord>("SELECT u.* FROM users u JOIN sessions s ON s.user_id=u.id WHERE s.id=? AND u.id=? AND u.is_active=1 AND s.revoked_at IS NULL AND s.expires_at>?")
        .bind(&actor.sid).bind(&actor.user.id).bind(now()).fetch_optional(&mut **tx).await
        .map_err(|_|AuthError::internal())?.ok_or_else(AuthError::unauthorized)
}

pub(super) async fn tracker_for_partner(
    tx: &mut Transaction<'_, Sqlite>,
    partner_id: &str,
) -> Result<UserRecord, AuthError> {
    sqlx::query_as::<_, UserRecord>("SELECT u.* FROM users u JOIN pairs p ON p.tracker_id=u.id WHERE p.partner_id=? AND u.is_active=1 AND u.role='tracker'")
        .bind(partner_id).fetch_optional(&mut **tx).await.map_err(|_|AuthError::internal())?.ok_or_else(AuthError::forbidden)
}

pub(super) async fn settings(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<CycleSettings, AuthError> {
    let row = sqlx::query_as::<_,SettingsRow>("SELECT cycle_length,period_length,context,length_method FROM cycle_settings WHERE tracker_id=?")
        .bind(id).fetch_optional(&mut **tx).await.map_err(|_|AuthError::internal())?;
    Ok(match row {
        Some(row) => CycleSettings {
            cycle_length: row.cycle_length,
            period_length: row.period_length,
            context: row.context,
            length_method: row.length_method,
        },
        None => CycleSettings {
            cycle_length: None,
            period_length: 5,
            context: "regular".into(),
            length_method: "auto".into(),
        },
    })
}
pub(super) async fn save_settings(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &SettingsInput,
) -> Result<(), AuthError> {
    sqlx::query("INSERT INTO cycle_settings(tracker_id,cycle_length,period_length,context,length_method) VALUES(?,?,?,?,?) ON CONFLICT(tracker_id) DO UPDATE SET cycle_length=excluded.cycle_length,period_length=excluded.period_length,context=excluded.context,length_method=excluded.length_method")
        .bind(id).bind(input.cycle_length).bind(input.period_length).bind(&input.context).bind(&input.length_method)
        .execute(&mut **tx).await.map_err(|_|AuthError::internal())?;
    Ok(())
}
pub(super) async fn periods(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<Vec<Period>, AuthError> {
    let rows = sqlx::query_as::<_,PeriodRow>("SELECT id,start_date,end_date FROM cycle_periods WHERE tracker_id=? ORDER BY start_date LIMIT 500")
        .bind(id).fetch_all(&mut **tx).await.map_err(|_|AuthError::internal())?;
    Ok(rows
        .into_iter()
        .map(|row| Period {
            id: row.id,
            start_date: row.start_date,
            end_date: row.end_date,
        })
        .collect())
}
pub(super) async fn save_period(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    period: &Period,
) -> Result<(), AuthError> {
    sqlx::query("INSERT INTO cycle_periods(id,tracker_id,start_date,end_date) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET start_date=excluded.start_date,end_date=excluded.end_date WHERE cycle_periods.tracker_id=excluded.tracker_id")
        .bind(&period.id).bind(id).bind(&period.start_date).bind(&period.end_date)
        .execute(&mut **tx).await.map_err(conflict_or_internal)?;
    Ok(())
}
pub(super) async fn delete_period(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    period_id: &str,
) -> Result<(), AuthError> {
    let rows = sqlx::query("DELETE FROM cycle_periods WHERE tracker_id=? AND id=?")
        .bind(id)
        .bind(period_id)
        .execute(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?
        .rows_affected();
    if rows == 0 {
        return Err(not_found());
    }
    Ok(())
}
pub(super) async fn diary(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<Vec<DiaryEntry>, AuthError> {
    let rows=sqlx::query_as::<_,DiaryRow>("SELECT date,symptoms,events,mood,flow,pain,notes FROM diary_entries WHERE tracker_id=? ORDER BY date LIMIT 1000")
        .bind(id).fetch_all(&mut **tx).await.map_err(|_|AuthError::internal())?;
    rows.into_iter()
        .map(|row| {
            Ok(DiaryEntry {
                date: row.date,
                data: DiaryInput {
                    symptoms: serde_json::from_str(&row.symptoms)
                        .map_err(|_| AuthError::internal())?,
                    events: serde_json::from_str(&row.events).map_err(|_| AuthError::internal())?,
                    mood: row.mood,
                    flow: row.flow,
                    pain: row.pain,
                    notes: row.notes,
                },
            })
        })
        .collect()
}
pub(super) async fn diary_size(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    date: &str,
) -> Result<(i64, bool), AuthError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM diary_entries WHERE tracker_id=?")
        .bind(id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM diary_entries WHERE tracker_id=? AND date=?)",
    )
    .bind(id)
    .bind(date)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| AuthError::internal())?;
    Ok((count, exists))
}
pub(super) async fn save_diary(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    date: &str,
    input: &DiaryInput,
) -> Result<(), AuthError> {
    let symptoms = serde_json::to_string(&input.symptoms).map_err(|_| AuthError::internal())?;
    let events = serde_json::to_string(&input.events).map_err(|_| AuthError::internal())?;
    sqlx::query("INSERT INTO diary_entries(tracker_id,date,symptoms,events,mood,flow,pain,notes) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(tracker_id,date) DO UPDATE SET symptoms=excluded.symptoms,events=excluded.events,mood=excluded.mood,flow=excluded.flow,pain=excluded.pain,notes=excluded.notes")
        .bind(id).bind(date).bind(symptoms).bind(events).bind(&input.mood).bind(&input.flow).bind(input.pain).bind(&input.notes)
        .execute(&mut **tx).await.map_err(|_|AuthError::internal())?;
    Ok(())
}
pub(super) async fn delete_diary(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    date: &str,
) -> Result<(), AuthError> {
    sqlx::query("DELETE FROM diary_entries WHERE tracker_id=? AND date=?")
        .bind(id)
        .bind(date)
        .execute(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(())
}

pub(super) async fn active_pair_id(
    tx: &mut Transaction<'_, Sqlite>,
    tracker: &str,
) -> Result<Option<String>, AuthError> {
    sqlx::query_scalar("SELECT p.id FROM pairs p JOIN users u ON u.id=p.partner_id WHERE p.tracker_id=? AND u.is_active=1 AND u.role='partner'")
        .bind(tracker).fetch_optional(&mut **tx).await.map_err(|_| AuthError::internal())
}

pub(super) async fn partner_diary(
    tx: &mut Transaction<'_, Sqlite>,
    pair: &str,
) -> Result<Vec<PartnerDiaryEntry>, AuthError> {
    let rows = sqlx::query_as::<_, PartnerDiaryRow>(
        "SELECT date,symptoms,events,mood,pain,notes FROM partner_diary_entries WHERE pair_id=? ORDER BY date LIMIT 1000"
    )
    .bind(pair)
    .fetch_all(&mut **tx)
    .await
    .map_err(|_| AuthError::internal())?;
    rows.into_iter()
        .map(|row| {
            Ok(PartnerDiaryEntry {
                date: row.date,
                data: PartnerDiaryInput {
                    symptoms: Some(serde_json::from_str(&row.symptoms).map_err(|_| AuthError::internal())?),
                    events: Some(serde_json::from_str(&row.events).map_err(|_| AuthError::internal())?),
                    mood: row.mood,
                    pain: Some(row.pain),
                    notes: row.notes,
                },
            })
        })
        .collect()
}

pub(super) async fn change_partner_diary(
    tx: &mut Transaction<'_, Sqlite>,
    pair: &str,
    date: &str,
    input: Option<PartnerDiaryInput>,
) -> Result<(), AuthError> {
    if let Some(input) = input {
        let symptoms = input.symptoms.as_ref().map(serde_json::to_string)
            .transpose().map_err(|_| AuthError::internal())?;
        let events = input.events.as_ref().map(serde_json::to_string)
            .transpose().map_err(|_| AuthError::internal())?;
        // A previous client may still send only mood and notes.
        let result = sqlx::query(
            "INSERT INTO partner_diary_entries(pair_id,date,symptoms,events,mood,pain,notes)
             SELECT ?,?,COALESCE(?,'[]'),COALESCE(?,'[]'),?,COALESCE(?,0),?
             WHERE EXISTS(SELECT 1 FROM partner_diary_entries WHERE pair_id=? AND date=?)
                OR (SELECT COUNT(*) FROM partner_diary_entries WHERE pair_id=?) < 1000
             ON CONFLICT(pair_id,date) DO UPDATE SET
                symptoms=COALESCE(?,partner_diary_entries.symptoms),
                events=COALESCE(?,partner_diary_entries.events),
                pain=COALESCE(?,partner_diary_entries.pain),
                mood=excluded.mood,notes=excluded.notes"
        )
        .bind(pair)
        .bind(date)
        .bind(symptoms.as_deref())
        .bind(events.as_deref())
        .bind(input.mood)
        .bind(input.pain)
        .bind(input.notes)
        .bind(pair)
        .bind(date)
        .bind(pair)
        .bind(symptoms.as_deref())
        .bind(events.as_deref())
        .bind(input.pain)
        .execute(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?;
        if result.rows_affected() == 0 {
            return Err(AuthError::conflict());
        }
    } else {
        sqlx::query("DELETE FROM partner_diary_entries WHERE pair_id=? AND date=?")
            .bind(pair).bind(date).execute(&mut **tx).await.map_err(|_| AuthError::internal())?;
    }
    Ok(())
}
