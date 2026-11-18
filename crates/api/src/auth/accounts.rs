use super::*;

pub(super) async fn users(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    let rows = store::users(&state.pool).await?;
    Ok(Json(
        serde_json::json!({"users": rows.iter().map(UserRecord::public).collect::<Vec<_>>()}),
    ))
}

pub(super) async fn create_user(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<CreateUserInput>,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    check_origin(&headers, &state.config, false)?;
    let user = account_service::create_account(&state, &actor, input).await?;
    Ok((StatusCode::CREATED, Json(user)).into_response())
}

pub(super) async fn patch_user(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    ApiJson(input): ApiJson<PatchUserInput>,
) -> Result<Json<User>, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    check_origin(&headers, &state.config, false)?;
    Ok(Json(
        account_service::update_account(&state, &actor, &id, input).await?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PasswordInput {
    current_password: String,
    new_password: String,
}

pub(super) async fn password(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<PasswordInput>,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    check_origin(&headers, &state.config, false)?;
    limited(&state, &format!("password-change:{}", actor.user.id))?;
    account_service::change_password(&state, &actor, input.current_password, input.new_password)
        .await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_cookie(&mut response, &state.config, "flo_access", "", 0);
    append_cookie(&mut response, &state.config, "flo_refresh", "", 0);
    Ok(response)
}
