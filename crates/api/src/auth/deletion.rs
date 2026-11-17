use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Confirmation {
    current_password: String,
}

async fn remove(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<(), AuthError> {
    let user = sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?
        .ok_or_else(AuthError::invalid)?;
    if user.role == "admin" && user.is_active {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role='admin' AND is_active=1")
                .fetch_one(&mut **tx)
                .await
                .map_err(|_| AuthError::internal())?;
        if count <= 1 {
            return Err(AuthError::conflict());
        }
    }
    sqlx::query("DELETE FROM pairs WHERE tracker_id=? OR partner_id=?")
        .bind(id)
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?;
    sqlx::query("DELETE FROM users WHERE id=?")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(())
}

pub(super) async fn admin(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    check_origin(&headers, &state.config, false)?;
    valid_id(&id)?;
    // Own deletion requires the current password through the profile endpoint.
    if id == actor.user.id {
        return Err(AuthError::conflict());
    }
    let mut tx = actor_transaction(&state, &actor, true).await?;
    remove(&mut tx, &id).await?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn own(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<Confirmation>,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    check_origin(&headers, &state.config, false)?;
    if !state
        .login_limits
        .lock()
        .map_err(|_| AuthError::internal())?
        .allow(&format!("delete:{}", actor.user.id))
    {
        return Err(AuthError::limited());
    }
    if input.current_password.len() > 512 {
        return Err(AuthError::invalid());
    }
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
        .bind(&actor.user.id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AuthError::internal())?;
    if !verify_password(input.current_password, hash.clone(), state.hashing.clone()).await? {
        return Err(AuthError::credentials());
    }
    let mut tx = actor_transaction(&state, &actor, false).await?;
    let current: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
        .bind(&actor.user.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| AuthError::internal())?;
    if hash != current {
        return Err(AuthError::credentials());
    }
    remove(&mut tx, &actor.user.id).await?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_cookie(&mut response, &state.config, "flo_access", "", 0);
    append_cookie(&mut response, &state.config, "flo_refresh", "", 0);
    Ok(response)
}
