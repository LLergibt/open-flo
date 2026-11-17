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

fn unavailable() -> AuthError {
    AuthError {
        status: StatusCode::NOT_FOUND,
        error: "invite_unavailable",
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
    kind: String,
    role: String,
}
async fn invitation(
    tx: &mut Transaction<'_, Sqlite>,
    token: &str,
) -> Result<Invitation, AuthError> {
    let hash = token_hash(token).map_err(|_| unavailable())?;
    sqlx::query_as::<_,Invitation>("SELECT i.tracker_id,i.expires_at,'pair' AS kind,'partner' AS role FROM partner_invites i JOIN users u ON u.id=i.tracker_id WHERE i.token_hash=? AND i.expires_at>? AND u.is_active=1 AND u.role='tracker' AND NOT EXISTS(SELECT 1 FROM pairs WHERE tracker_id=u.id) UNION ALL SELECT i.admin_id AS tracker_id,i.expires_at,'account' AS kind,i.role FROM account_invites i JOIN users u ON u.id=i.admin_id WHERE i.token_hash=? AND i.expires_at>? AND u.is_active=1 AND u.role='admin'")
        .bind(&hash).bind(now()).bind(&hash).bind(now()).fetch_optional(&mut **tx).await.map_err(|_|AuthError::internal())?.ok_or_else(unavailable)
}
async fn attach(
    tx: &mut Transaction<'_, Sqlite>,
    invite: Invitation,
    partner: &User,
) -> Result<Pair, AuthError> {
    if invite.kind != "pair" {
        return Err(unavailable());
    }
    account_service::create_pair(tx, &invite.tracker_id, &partner.id).await
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
        serde_json::json!({"tracker_display_name":name,"expires_at":expiry(invite.expires_at)?,"kind":invite.kind,"role":invite.role}),
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
        role: invite.role.clone(),
        is_active: true,
    };
    insert_user(&mut tx, &user, &hash).await?;
    let pair = if invite.kind == "pair" {
        Some(attach(&mut tx, invite, &user).await?)
    } else {
        sqlx::query("DELETE FROM account_invites WHERE token_hash=?")
            .bind(token_digest)
            .execute(&mut *tx)
            .await
            .map_err(|_| AuthError::internal())?;
        None
    };
    tx.commit().await.map_err(conflict_or_internal)?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"user":user,"pair":pair})),
    )
        .into_response())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AccountInviteInput {
    role: String,
}

pub(super) async fn create_account(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<AccountInviteInput>,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    check_origin(&headers, &state.config, false)?;
    if !["tracker", "partner"].contains(&input.role.as_str()) {
        return Err(AuthError::invalid());
    }
    limited(&state, &format!("account-invite:{}", actor.user.id))?;
    let mut tx = actor_transaction(&state, &actor, true).await?;
    let token = random_token();
    let expires = now() + 24 * 60 * 60;
    sqlx::query("DELETE FROM account_invites WHERE admin_id=? AND role=?")
        .bind(&actor.user.id)
        .bind(&input.role)
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthError::internal())?;
    sqlx::query("INSERT INTO account_invites(token_hash,admin_id,role,expires_at) VALUES(?,?,?,?)")
        .bind(token_hash(&token)?)
        .bind(&actor.user.id)
        .bind(&input.role)
        .bind(expires)
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthError::internal())?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    let url = state
        .config
        .public_app_url
        .as_ref()
        .map(|base| format!("{base}/#invite={token}"));
    Ok((StatusCode::CREATED,Json(serde_json::json!({"token":token,"url":url,"expires_at":expiry(expires)?,"role":input.role}))).into_response())
}
pub(super) async fn revoke_account(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path(role): Path<String>,
) -> Result<StatusCode, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    check_origin(&headers, &state.config, false)?;
    if !["tracker", "partner"].contains(&role.as_str()) {
        return Err(AuthError::invalid());
    }
    let mut tx = actor_transaction(&state, &actor, true).await?;
    sqlx::query("DELETE FROM account_invites WHERE admin_id=? AND role=?")
        .bind(&actor.user.id)
        .bind(role)
        .execute(&mut *tx)
        .await
        .map_err(|_| AuthError::internal())?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(StatusCode::NO_CONTENT)
}
