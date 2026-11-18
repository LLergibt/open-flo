use super::*;

async fn public_pair(pool: &SqlitePool, record: PairRecord) -> Result<Pair, AuthError> {
    let users = sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE id=? OR id=?")
        .bind(&record.tracker_id)
        .bind(&record.partner_id)
        .fetch_all(pool)
        .await
        .map_err(|_| AuthError::internal())?;
    let tracker = users
        .iter()
        .find(|user| user.id == record.tracker_id)
        .ok_or_else(AuthError::internal)?
        .public();
    let partner = users
        .iter()
        .find(|user| user.id == record.partner_id)
        .ok_or_else(AuthError::internal)?
        .public();
    Ok(Pair {
        id: record.id,
        tracker,
        partner,
        created_at: record.created_at,
    })
}

pub(super) async fn pairs(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    let rows = sqlx::query_as::<_, PairRecord>("SELECT * FROM pairs ORDER BY created_at,id")
        .fetch_all(&state.pool)
        .await
        .map_err(|_| AuthError::internal())?;
    let mut pairs = Vec::with_capacity(rows.len());
    for row in rows {
        pairs.push(public_pair(&state.pool, row).await?);
    }
    Ok(Json(serde_json::json!({"pairs": pairs})))
}

pub(super) async fn create_pair(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<CreatePairInput>,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    check_origin(&headers, &state.config, false)?;
    valid_id(&input.tracker_id)?;
    valid_id(&input.partner_id)?;
    if input.tracker_id == input.partner_id {
        return Err(AuthError::invalid());
    }
    let mut transaction = actor_transaction(&state, &actor, true).await?;
    let pair = account_service::create_pair(&mut transaction, &input.tracker_id, &input.partner_id)
        .await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    Ok((StatusCode::CREATED, Json(pair)).into_response())
}

pub(super) async fn my_pair(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    let row = sqlx::query_as::<_, PairRecord>("SELECT p.* FROM pairs p JOIN users t ON t.id=p.tracker_id JOIN users u ON u.id=p.partner_id WHERE (p.tracker_id=? OR p.partner_id=?) AND t.is_active=1 AND u.is_active=1")
        .bind(&actor.user.id).bind(&actor.user.id).fetch_optional(&state.pool).await.map_err(|_| AuthError::internal())?;
    let pair = match row {
        Some(row) => Some(public_pair(&state.pool, row).await?),
        None => None,
    };
    Ok(Json(serde_json::json!({"pair": pair})))
}

pub(super) async fn unlink_my_pair(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    check_origin(&headers, &state.config, false)?;
    let mut transaction = actor_transaction(&state, &actor, false).await?;
    sqlx::query("DELETE FROM pairs WHERE tracker_id=? OR partner_id=?")
        .bind(&actor.user.id)
        .bind(&actor.user.id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn unlink_pair(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    check_origin(&headers, &state.config, false)?;
    valid_id(&id)?;
    let mut transaction = actor_transaction(&state, &actor, true).await?;
    sqlx::query("DELETE FROM pairs WHERE id=?")
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(StatusCode::NO_CONTENT)
}
