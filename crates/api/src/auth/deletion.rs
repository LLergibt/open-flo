use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Confirmation {
    current_password: String,
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
    account_service::remove(&mut tx, &id).await?;
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
    limited(&state, &format!("delete:{}", actor.user.id))?;
    let hash =
        account_service::verify_current_password(&state, &actor.user.id, input.current_password)
            .await?;
    let mut tx = actor_transaction(&state, &actor, false).await?;
    let current = store::user(&mut tx, &actor.user.id).await?.password_hash;
    if hash != current {
        return Err(AuthError::credentials());
    }
    account_service::remove(&mut tx, &actor.user.id).await?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_cookie(&mut response, &state.config, "flo_access", "", 0);
    append_cookie(&mut response, &state.config, "flo_refresh", "", 0);
    Ok(response)
}
