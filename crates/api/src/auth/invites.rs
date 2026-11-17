use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TokenInput {
    token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Registration {
    token: String,
    username: String,
    display_name: String,
    password: String,
    #[serde(default)]
    client: Client,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PasswordInput {
    current_password: String,
    new_password: String,
}

fn unavailable() -> AuthError {
    AuthError {
        status: StatusCode::NOT_FOUND,
        error: "invite_unavailable",
    }
}
fn limited(state: &AuthState, key: &str) -> Result<(), AuthError> {
    if state
        .login_limits
        .lock()
        .map_err(|_| AuthError::internal())?
        .allow(key)
    {
        Ok(())
    } else {
        Err(AuthError::limited())
    }
}
fn expiry(value: i64) -> Result<String, AuthError> {
    OffsetDateTime::from_unix_timestamp(value)
        .map_err(|_| AuthError::internal())?
        .format(&Rfc3339)
        .map_err(|_| AuthError::internal())
}
#[derive(FromRow)]
struct Invitation {
    tracker_id: String,
    expires_at: i64,
}
async fn invitation(
    tx: &mut Transaction<'_, Sqlite>,
    token: &str,
) -> Result<Invitation, AuthError> {
    let hash = token_hash(token).map_err(|_| unavailable())?;
    sqlx::query_as::<_,Invitation>("SELECT i.tracker_id,i.expires_at FROM partner_invites i JOIN users u ON u.id=i.tracker_id WHERE i.token_hash=? AND i.expires_at>? AND u.is_active=1 AND u.role='tracker' AND NOT EXISTS(SELECT 1 FROM pairs WHERE tracker_id=u.id)")
        .bind(hash).bind(now()).fetch_optional(&mut **tx).await.map_err(|_|AuthError::internal())?.ok_or_else(unavailable)
}
async fn attach(
    tx: &mut Transaction<'_, Sqlite>,
    invite: Invitation,
    partner: &User,
) -> Result<Pair, AuthError> {
    let tracker = sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE id=?")
        .bind(&invite.tracker_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?
        .public();
    let pair = Pair {
        id: Uuid::new_v4().to_string(),
        tracker,
        partner: partner.clone(),
        created_at: expiry(now())?,
    };
    sqlx::query("INSERT INTO pairs(id,tracker_id,partner_id,created_at) VALUES(?,?,?,?)")
        .bind(&pair.id)
        .bind(&pair.tracker.id)
        .bind(&pair.partner.id)
        .bind(&pair.created_at)
        .execute(&mut **tx)
        .await
        .map_err(conflict_or_internal)?;
    Ok(pair)
}

pub(super) async fn create(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    check_origin(&headers, &state.config, false)?;
    if actor.user.role != "tracker" {
        return Err(AuthError::forbidden());
    }
    limited(&state, &format!("invite-create:{}", actor.user.id))?;
    let mut tx = actor_transaction(&state, &actor, false).await?;
    let occupied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pairs WHERE tracker_id=? OR partner_id=?")
            .bind(&actor.user.id)
            .bind(&actor.user.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AuthError::internal())?;
    if occupied != 0 {
        return Err(AuthError::conflict());
    }
    let token = random_token();
    let expires = now() + 24 * 60 * 60;
    sqlx::query("DELETE FROM partner_invites WHERE tracker_id=?")
        .bind(&actor.user.id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthError::internal())?;
    sqlx::query("INSERT INTO partner_invites(token_hash,tracker_id,expires_at) VALUES(?,?,?)")
        .bind(token_hash(&token)?)
        .bind(&actor.user.id)
        .bind(expires)
        .execute(&mut *tx)
        .await
        .map_err(conflict_or_internal)?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    let url = state
        .config
        .public_app_url
        .as_ref()
        .map(|base| format!("{base}/#invite={token}"));
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"token":token,"url":url,"expires_at":expiry(expires)?})),
    )
        .into_response())
}
pub(super) async fn revoke(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    check_origin(&headers, &state.config, false)?;
    if actor.user.role != "tracker" {
        return Err(AuthError::forbidden());
    }
    let mut tx = actor_transaction(&state, &actor, false).await?;
    sqlx::query("DELETE FROM partner_invites WHERE tracker_id=?")
        .bind(&actor.user.id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthError::internal())?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn preview(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<TokenInput>,
) -> Result<Json<serde_json::Value>, AuthError> {
    check_origin(&headers, &state.config, false)?;
    let hash = token_hash(&input.token).map_err(|_| unavailable())?;
    limited(&state, &format!("invite-preview:{hash}"))?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| AuthError::internal())?;
    let invite = invitation(&mut tx, &input.token).await?;
    let name: String = sqlx::query_scalar("SELECT display_name FROM users WHERE id=?")
        .bind(&invite.tracker_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(Json(
        serde_json::json!({"tracker_display_name":name,"expires_at":expiry(invite.expires_at)?}),
    ))
}
pub(super) async fn accept(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<TokenInput>,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    check_origin(&headers, &state.config, false)?;
    if actor.user.role != "partner" {
        return Err(AuthError::forbidden());
    }
    let mut tx = actor_transaction(&state, &actor, false).await?;
    let invite = invitation(&mut tx, &input.token).await?;
    let pair = attach(&mut tx, invite, &actor.user).await?;
    tx.commit().await.map_err(conflict_or_internal)?;
    Ok((StatusCode::CREATED, Json(pair)).into_response())
}
pub(super) async fn register(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<Registration>,
) -> Result<Response, AuthError> {
    check_origin(&headers, &state.config, input.client == Client::Web)?;
    let token_digest = token_hash(&input.token).map_err(|_| unavailable())?;
    limited(&state, &format!("invite-register:{token_digest}"))?;
    // Reject unavailable invitations before expensive hashing; recheck under the write lock below.
    {
        let mut tx = state
            .pool
            .begin()
            .await
            .map_err(|_| AuthError::internal())?;
        invitation(&mut tx, &input.token).await?;
    }
    let username = normalized_username(&input.username)?;
    let name = display_name(&input.display_name)?;
    if !password_valid(&input.password) {
        return Err(AuthError::invalid());
    }
    let hash = hash_password(input.password, state.hashing.clone()).await?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(conflict_or_internal)?;
    let invite = invitation(&mut tx, &input.token).await?;
    let user = User {
        id: Uuid::new_v4().to_string(),
        username,
        display_name: name,
        role: "partner".into(),
        is_active: true,
    };
    insert_user(&mut tx, &user, &hash).await?;
    let pair = attach(&mut tx, invite, &user).await?;
    tx.commit().await.map_err(conflict_or_internal)?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"user":user,"pair":pair})),
    )
        .into_response())
}
pub(super) async fn password(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<PasswordInput>,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    check_origin(&headers, &state.config, false)?;
    limited(&state, &format!("password-change:{}", actor.user.id))?;
    if !password_valid(&input.new_password) || input.current_password.len() > 512 {
        return Err(AuthError::invalid());
    }
    let old: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
        .bind(&actor.user.id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AuthError::internal())?;
    if !verify_password(input.current_password, old.clone(), state.hashing.clone()).await? {
        return Err(AuthError::credentials());
    }
    let new = hash_password(input.new_password, state.hashing.clone()).await?;
    let mut tx = actor_transaction(&state, &actor, false).await?;
    let update = sqlx::query("UPDATE users SET password_hash=? WHERE id=? AND password_hash=?")
        .bind(new)
        .bind(&actor.user.id)
        .bind(old)
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthError::internal())?;
    if update.rows_affected() != 1 {
        return Err(AuthError::credentials());
    }
    sqlx::query("UPDATE sessions SET revoked_at=? WHERE user_id=?")
        .bind(now())
        .bind(&actor.user.id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthError::internal())?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_cookie(&mut response, &state.config, "flo_access", "", 0);
    append_cookie(&mut response, &state.config, "flo_refresh", "", 0);
    Ok(response)
}
