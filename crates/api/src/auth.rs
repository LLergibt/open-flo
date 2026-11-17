mod deletion;
mod invites;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{FromRequest, Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use jsonwebtoken::{
    Algorithm as JwtAlgorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode,
};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::{
    StartupError,
    config::{ACCESS_TTL, AuthConfig, SESSION_TTL},
};

const ISSUER: &str = "open-source-flo";
const AUDIENCE: &str = "flo-clients";
const DUMMY_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$c3ludGhldGljLXNsdA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const MAX_REFRESHES: i64 = 2048;

#[derive(Clone)]
pub(crate) struct AuthState {
    pool: SqlitePool,
    config: AuthConfig,
    hashing: Arc<Semaphore>,
    login_limits: Arc<Mutex<LoginLimits>>,
}

#[derive(Default)]
struct LoginLimits {
    total: VecDeque<Instant>,
    names: HashMap<String, VecDeque<Instant>>,
}

impl LoginLimits {
    fn allow(&mut self, username: &str) -> bool {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(60);
        self.total.retain(|seen| *seen > cutoff);
        self.names.retain(|_, attempts| {
            attempts.retain(|seen| *seen > cutoff);
            !attempts.is_empty()
        });
        if self.total.len() >= 60
            || self
                .names
                .get(username)
                .is_some_and(|attempts| attempts.len() >= 5)
        {
            return false;
        }
        self.total.push_back(now);
        self.names
            .entry(username.to_owned())
            .or_default()
            .push_back(now);
        true
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
}

#[derive(FromRow)]
struct UserRecord {
    id: String,
    username: String,
    display_name: String,
    role: String,
    is_active: bool,
    password_hash: String,
}

impl UserRecord {
    fn public(&self) -> User {
        User {
            id: self.id.clone(),
            username: self.username.clone(),
            display_name: self.display_name.clone(),
            role: self.role.clone(),
            is_active: self.is_active,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pair {
    pub id: String,
    pub tracker: User,
    pub partner: User,
    pub created_at: String,
}

#[derive(FromRow)]
struct PairRecord {
    id: String,
    tracker_id: String,
    partner_id: String,
    created_at: String,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    sid: String,
    iat: i64,
    exp: i64,
    iss: String,
    aud: String,
}

struct Authenticated {
    user: User,
    sid: String,
}

#[derive(Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Client {
    #[default]
    Web,
    Native,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginInput {
    username: String,
    password: String,
    #[serde(default)]
    client: Client,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RefreshInput {
    refresh_token: Option<String>,
    #[serde(default)]
    client: Client,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateUserInput {
    username: String,
    display_name: String,
    password: String,
    role: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchUserInput {
    is_active: Option<bool>,
    password: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePairInput {
    tracker_id: String,
    partner_id: String,
}

#[derive(Serialize)]
struct LoginResponse {
    user: User,
    access_token: String,
    expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
}

#[derive(Debug)]
pub(crate) struct AuthError {
    status: StatusCode,
    error: &'static str,
}

impl AuthError {
    fn invalid() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error: "invalid_input",
        }
    }
    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
        }
    }
    fn credentials() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error: "invalid_credentials",
        }
    }
    fn forbidden() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            error: "forbidden",
        }
    }
    fn conflict() -> Self {
        Self {
            status: StatusCode::CONFLICT,
            error: "conflict",
        }
    }
    fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
        }
    }
    fn limited() -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            error: "rate_limited",
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let mut response =
            (self.status, Json(serde_json::json!({"error": self.error}))).into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if self.status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
        }
        response
    }
}

struct ApiJson<T>(T);
impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AuthError;
    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|_| AuthError::invalid())
    }
}

struct OptionalJson<T>(T);
impl<S, T> FromRequest<S> for OptionalJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Default,
{
    type Rejection = AuthError;
    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let has_json_type = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|kind| kind.trim() == "application/json")
            });
        let bytes = Bytes::from_request(request, state)
            .await
            .map_err(|_| AuthError::invalid())?;
        if bytes.is_empty() {
            return Ok(Self(T::default()));
        }
        if !has_json_type {
            return Err(AuthError::invalid());
        }
        serde_json::from_slice(&bytes)
            .map(Self)
            .map_err(|_| AuthError::invalid())
    }
}

pub(crate) fn router(pool: SqlitePool, config: AuthConfig) -> Router {
    let state = AuthState {
        pool,
        config,
        hashing: Arc::new(Semaphore::new(2)),
        login_limits: Arc::new(Mutex::new(LoginLimits::default())),
    };
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/refresh", post(refresh))
        .route("/api/auth/logout", post(logout))
        .route("/api/me", get(me).delete(deletion::own))
        .route("/api/me/password", post(invites::password))
        .route(
            "/api/me/invite",
            post(invites::create).delete(invites::revoke),
        )
        .route("/api/me/invite/accept", post(invites::accept))
        .route("/api/invites/preview", post(invites::preview))
        .route("/api/invites/register", post(invites::register))
        .route("/api/admin/invites", post(invites::create_account))
        .route("/api/admin/invites/{role}", delete(invites::revoke_account))
        .route("/api/me/pair", get(my_pair).delete(unlink_my_pair))
        .route("/api/admin/users", get(users).post(create_user))
        .route(
            "/api/admin/users/{id}",
            axum::routing::patch(patch_user).delete(deletion::admin),
        )
        .route("/api/admin/pairs", get(pairs).post(create_pair))
        .route("/api/admin/pairs/{id}", delete(unlink_pair))
        .route("/api/setup/status", get(setup_status))
        .layer(axum::extract::DefaultBodyLimit::max(8192))
        .layer(axum::middleware::map_response(no_store))
        .with_state(state)
}

async fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after 1970")
        .as_secs() as i64
}

fn normalized_username(value: &str) -> Result<String, AuthError> {
    let value = value.trim().to_ascii_lowercase();
    if !(3..=64).contains(&value.len())
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte)
        })
    {
        return Err(AuthError::invalid());
    }
    Ok(value)
}

fn password_valid(value: &str) -> bool {
    (12..=128).contains(&value.chars().count()) && value.len() <= 512
}

fn display_name(value: &str) -> Result<String, AuthError> {
    let value = value.trim();
    if !(1..=80).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(AuthError::invalid());
    }
    Ok(value.to_owned())
}

fn valid_id(id: &str) -> Result<(), AuthError> {
    Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| AuthError::invalid())
}

fn hash_password_sync(password: &str) -> Result<String, AuthError> {
    let parameters = Params::new(19456, 2, 1, None).map_err(|_| AuthError::internal())?;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters)
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::internal())
}

async fn hash_password(password: String, semaphore: Arc<Semaphore>) -> Result<String, AuthError> {
    let permit = semaphore
        .try_acquire_owned()
        .map_err(|_| AuthError::limited())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        hash_password_sync(&password)
    })
    .await
    .map_err(|_| AuthError::internal())?
}

async fn verify_password(
    password: String,
    hash: String,
    semaphore: Arc<Semaphore>,
) -> Result<bool, AuthError> {
    let permit = semaphore
        .try_acquire_owned()
        .map_err(|_| AuthError::limited())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let parsed = PasswordHash::new(&hash).map_err(|_| AuthError::internal())?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|_| AuthError::internal())?
}

fn random_token() -> String {
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn token_hash(token: &str) -> Result<String, AuthError> {
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AuthError::unauthorized());
    }
    Ok(Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn access_token(
    config: &AuthConfig,
    user: &User,
    sid: &str,
    expires_at: i64,
) -> Result<String, AuthError> {
    let current = now();
    let claims = Claims {
        sub: user.id.clone(),
        sid: sid.to_owned(),
        iat: current,
        exp: (current + ACCESS_TTL).min(expires_at),
        iss: ISSUER.into(),
        aud: AUDIENCE.into(),
    };
    encode(
        &Header::new(JwtAlgorithm::HS256),
        &claims,
        &EncodingKey::from_secret(&config.secret),
    )
    .map_err(|_| AuthError::internal())
}

fn decode_access(config: &AuthConfig, token: &str) -> Result<Claims, AuthError> {
    if token.len() > 4096 {
        return Err(AuthError::unauthorized());
    }
    let mut validation = Validation::new(JwtAlgorithm::HS256);
    validation.leeway = 0;
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[AUDIENCE]);
    validation.set_required_spec_claims(&["exp", "iat", "iss", "aud", "sub"]);
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(&config.secret),
        &validation,
    )
    .map_err(|_| AuthError::unauthorized())?
    .claims;
    if claims.iat > now()
        || claims.exp <= now()
        || valid_id(&claims.sid).is_err()
        || valid_id(&claims.sub).is_err()
    {
        return Err(AuthError::unauthorized());
    }
    Ok(claims)
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Result<Option<String>, AuthError> {
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        let value = value.to_str().map_err(|_| AuthError::unauthorized())?;
        for part in value.split(';') {
            if let Some((key, value)) = part.trim().split_once('=') {
                if key == name {
                    if found.is_some() {
                        return Err(AuthError::unauthorized());
                    }
                    found = Some(value.to_owned());
                }
            }
        }
    }
    Ok(found)
}

fn check_origin(
    headers: &HeaderMap,
    config: &AuthConfig,
    web_required: bool,
) -> Result<(), AuthError> {
    if headers.get_all(header::ORIGIN).iter().count() > 1 {
        return Err(AuthError::forbidden());
    }
    match headers.get(header::ORIGIN) {
        Some(origin)
            if origin.to_str().ok().is_some_and(|value| {
                config
                    .trusted_origins
                    .iter()
                    .any(|trusted| trusted == value)
            }) =>
        {
            Ok(())
        }
        Some(_) => Err(AuthError::forbidden()),
        None if web_required || headers.contains_key(header::COOKIE) => Err(AuthError::forbidden()),
        None => Ok(()),
    }
}

async fn authenticate(headers: &HeaderMap, state: &AuthState) -> Result<Authenticated, AuthError> {
    let token = if headers.contains_key(header::AUTHORIZATION) {
        if headers.get_all(header::AUTHORIZATION).iter().count() != 1 {
            return Err(AuthError::unauthorized());
        }
        let header = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(AuthError::unauthorized)?;
        let (scheme, token) = header.split_once(' ').ok_or_else(AuthError::unauthorized)?;
        if !scheme.eq_ignore_ascii_case("bearer")
            || token.is_empty()
            || token.contains(char::is_whitespace)
        {
            return Err(AuthError::unauthorized());
        }
        token.to_owned()
    } else {
        cookie_value(headers, "flo_access")?.ok_or_else(AuthError::unauthorized)?
    };
    let claims = decode_access(&state.config, &token)?;
    let user = sqlx::query_as::<_, UserRecord>("SELECT u.* FROM users u JOIN sessions s ON s.user_id=u.id WHERE s.id=? AND u.id=? AND u.is_active=1 AND s.revoked_at IS NULL AND s.expires_at>?")
        .bind(&claims.sid).bind(&claims.sub).bind(now()).fetch_optional(&state.pool).await.map_err(|_| AuthError::internal())?.ok_or_else(AuthError::unauthorized)?;
    Ok(Authenticated {
        user: user.public(),
        sid: claims.sid,
    })
}

fn require_admin(actor: &Authenticated) -> Result<(), AuthError> {
    if actor.user.role != "admin" {
        return Err(AuthError::forbidden());
    }
    Ok(())
}

async fn actor_transaction<'a>(
    state: &'a AuthState,
    actor: &Authenticated,
    admin: bool,
) -> Result<Transaction<'a, Sqlite>, AuthError> {
    let mut transaction = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(conflict_or_internal)?;
    let role = sqlx::query_scalar::<_, String>("SELECT u.role FROM users u JOIN sessions s ON s.user_id=u.id WHERE s.id=? AND u.id=? AND u.is_active=1 AND s.revoked_at IS NULL AND s.expires_at>?")
        .bind(&actor.sid).bind(&actor.user.id).bind(now()).fetch_optional(&mut *transaction).await.map_err(|_| AuthError::internal())?.ok_or_else(AuthError::unauthorized)?;
    if admin && role != "admin" {
        return Err(AuthError::forbidden());
    }
    Ok(transaction)
}

fn append_cookie(
    response: &mut Response,
    config: &AuthConfig,
    name: &str,
    token: &str,
    max_age: i64,
) {
    let secure = if config.cookie_secure { "; Secure" } else { "" };
    let value =
        format!("{name}={token}; HttpOnly; SameSite=Strict; Path=/api; Max-Age={max_age}{secure}");
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("tokens contain only safe ASCII"),
    );
}

fn login_response(
    config: &AuthConfig,
    user: User,
    sid: &str,
    refresh: String,
    client: Client,
    expires_at: i64,
) -> Result<Response, AuthError> {
    let access = access_token(config, &user, sid, expires_at)?;
    let remaining = (expires_at - now()).min(ACCESS_TTL);
    let mut response = Json(LoginResponse {
        user,
        access_token: access.clone(),
        expires_in: remaining,
        refresh_token: (client == Client::Native).then(|| refresh.clone()),
    })
    .into_response();
    if client == Client::Web {
        append_cookie(&mut response, config, "flo_access", &access, remaining);
        append_cookie(
            &mut response,
            config,
            "flo_refresh",
            &refresh,
            expires_at - now(),
        );
    }
    Ok(response)
}

async fn login(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<LoginInput>,
) -> Result<Response, AuthError> {
    check_origin(&headers, &state.config, input.client == Client::Web)?;
    let username = normalized_username(&input.username).unwrap_or_else(|_| "<invalid>".into());
    if !state
        .login_limits
        .lock()
        .map_err(|_| AuthError::internal())?
        .allow(&username)
    {
        return Err(AuthError::limited());
    }
    if !password_valid(&input.password) {
        return Err(AuthError::credentials());
    }
    let record = sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE username=?")
        .bind(&username)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| AuthError::internal())?;
    let hash = record
        .as_ref()
        .map_or(DUMMY_HASH, |record| &record.password_hash)
        .to_owned();
    let verified = verify_password(input.password, hash, state.hashing.clone()).await?;
    let record = record
        .filter(|record| record.is_active && verified)
        .ok_or_else(AuthError::credentials)?;
    let timestamp = now();
    let expires = timestamp + SESSION_TTL;
    let sid = Uuid::new_v4().to_string();
    let refresh = random_token();
    let hash = token_hash(&refresh)?;
    let mut transaction = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| AuthError::internal())?;
    // Reject a password reset/disable that raced with the expensive verification.
    let still_valid = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE id=? AND is_active=1 AND password_hash=?",
    )
    .bind(&record.id)
    .bind(&record.password_hash)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| AuthError::internal())?;
    if still_valid != 1 {
        return Err(AuthError::credentials());
    }
    sqlx::query("DELETE FROM sessions WHERE expires_at<=?")
        .bind(timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?;
    sqlx::query("UPDATE sessions SET revoked_at=? WHERE id IN (SELECT id FROM sessions WHERE user_id=? AND revoked_at IS NULL ORDER BY created_at DESC, id DESC LIMIT -1 OFFSET 31)")
        .bind(timestamp).bind(&record.id).execute(&mut *transaction).await.map_err(|_| AuthError::internal())?;
    sqlx::query("INSERT INTO sessions(id,user_id,created_at,expires_at) VALUES(?,?,?,?)")
        .bind(&sid)
        .bind(&record.id)
        .bind(timestamp)
        .bind(expires)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?;
    sqlx::query("INSERT INTO refresh_tokens(token_hash,session_id,expires_at) VALUES(?,?,?)")
        .bind(hash)
        .bind(&sid)
        .bind(expires)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    login_response(
        &state.config,
        record.public(),
        &sid,
        refresh,
        input.client,
        expires,
    )
}

#[derive(FromRow)]
struct RefreshRecord {
    session_id: String,
    user_id: String,
    expires_at: i64,
    used_at: Option<i64>,
    revoked_at: Option<i64>,
    refresh_count: i64,
}

fn supplied_refresh(headers: &HeaderMap, input: &RefreshInput) -> Result<String, AuthError> {
    if input.client == Client::Native {
        input
            .refresh_token
            .clone()
            .ok_or_else(AuthError::unauthorized)
    } else {
        cookie_value(headers, "flo_refresh")?.ok_or_else(AuthError::unauthorized)
    }
}

async fn refresh(
    State(state): State<AuthState>,
    headers: HeaderMap,
    OptionalJson(input): OptionalJson<RefreshInput>,
) -> Result<Response, AuthError> {
    check_origin(&headers, &state.config, input.client == Client::Web)?;
    let hash = token_hash(&supplied_refresh(&headers, &input)?)?;
    let timestamp = now();
    let mut transaction = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| AuthError::internal())?;
    let token = sqlx::query_as::<_, RefreshRecord>("SELECT r.session_id,s.user_id,s.expires_at,r.used_at,s.revoked_at,s.refresh_count FROM refresh_tokens r JOIN sessions s ON s.id=r.session_id WHERE r.token_hash=?")
        .bind(&hash).fetch_optional(&mut *transaction).await.map_err(|_| AuthError::internal())?.ok_or_else(AuthError::unauthorized)?;
    if token.used_at.is_some() || token.refresh_count >= MAX_REFRESHES {
        sqlx::query("UPDATE sessions SET revoked_at=? WHERE id=?")
            .bind(timestamp)
            .bind(&token.session_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| AuthError::internal())?;
        transaction
            .commit()
            .await
            .map_err(|_| AuthError::internal())?;
        return Err(AuthError::unauthorized());
    }
    if token.revoked_at.is_some() || token.expires_at <= timestamp {
        return Err(AuthError::unauthorized());
    }
    let user = sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE id=? AND is_active=1")
        .bind(&token.user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?
        .ok_or_else(AuthError::unauthorized)?;
    let next = random_token();
    sqlx::query("UPDATE refresh_tokens SET used_at=? WHERE token_hash=?")
        .bind(timestamp)
        .bind(&hash)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?;
    sqlx::query("UPDATE sessions SET refresh_count=refresh_count+1 WHERE id=?")
        .bind(&token.session_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?;
    sqlx::query("INSERT INTO refresh_tokens(token_hash,session_id,expires_at) VALUES(?,?,?)")
        .bind(token_hash(&next)?)
        .bind(&token.session_id)
        .bind(token.expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    login_response(
        &state.config,
        user.public(),
        &token.session_id,
        next,
        input.client,
        token.expires_at,
    )
}

async fn logout(
    State(state): State<AuthState>,
    headers: HeaderMap,
    OptionalJson(input): OptionalJson<RefreshInput>,
) -> Result<Response, AuthError> {
    check_origin(
        &headers,
        &state.config,
        input.client == Client::Web && !headers.contains_key(header::AUTHORIZATION),
    )?;
    let actor = authenticate(&headers, &state).await;
    let sid = match actor {
        Ok(actor) => Some(actor.sid),
        Err(_) if headers.contains_key(header::AUTHORIZATION) => {
            return Err(AuthError::unauthorized());
        }
        Err(_) => match supplied_refresh(&headers, &input).and_then(|token| token_hash(&token)) {
            Ok(hash) => sqlx::query_scalar::<_, String>(
                "SELECT session_id FROM refresh_tokens WHERE token_hash=?",
            )
            .bind(hash)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| AuthError::internal())?,
            Err(_) => None,
        },
    };
    if let Some(sid) = sid {
        sqlx::query("UPDATE sessions SET revoked_at=? WHERE id=?")
            .bind(now())
            .bind(sid)
            .execute(&state.pool)
            .await
            .map_err(|_| AuthError::internal())?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_cookie(&mut response, &state.config, "flo_access", "", 0);
    append_cookie(&mut response, &state.config, "flo_refresh", "", 0);
    Ok(response)
}

async fn me(State(state): State<AuthState>, headers: HeaderMap) -> Result<Json<User>, AuthError> {
    Ok(Json(authenticate(&headers, &state).await?.user))
}

async fn setup_status(
    State(state): State<AuthState>,
) -> Result<Json<serde_json::Value>, AuthError> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE role='admin'")
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(Json(serde_json::json!({"needs_setup": count == 0})))
}

async fn users(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    let rows = sqlx::query_as::<_, UserRecord>("SELECT * FROM users ORDER BY created_at,id")
        .fetch_all(&state.pool)
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(Json(
        serde_json::json!({"users": rows.iter().map(UserRecord::public).collect::<Vec<_>>()}),
    ))
}

fn conflict_or_internal(error: sqlx::Error) -> AuthError {
    if let sqlx::Error::Database(database) = &error {
        if database.is_unique_violation()
            || database
                .code()
                .is_some_and(|code| ["5", "6", "1555", "2067"].contains(&code.as_ref()))
        {
            return AuthError::conflict();
        }
    }
    AuthError::internal()
}

async fn create_user(
    State(state): State<AuthState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<CreateUserInput>,
) -> Result<Response, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    check_origin(&headers, &state.config, false)?;
    let username = normalized_username(&input.username)?;
    let display = display_name(&input.display_name)?;
    if !password_valid(&input.password)
        || !["admin", "tracker", "partner"].contains(&input.role.as_str())
    {
        return Err(AuthError::invalid());
    }
    let hash = hash_password(input.password, state.hashing.clone()).await?;
    let user = User {
        id: Uuid::new_v4().to_string(),
        username,
        display_name: display,
        role: input.role,
        is_active: true,
    };
    let mut transaction = actor_transaction(&state, &actor, true).await?;
    insert_user(&mut transaction, &user, &hash).await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    Ok((StatusCode::CREATED, Json(user)).into_response())
}

async fn insert_user(
    transaction: &mut Transaction<'_, Sqlite>,
    user: &User,
    hash: &str,
) -> Result<(), AuthError> {
    sqlx::query("INSERT INTO users(id,username,display_name,role,password_hash,created_at) VALUES(?,?,?,?,?,?)")
        .bind(&user.id).bind(&user.username).bind(&user.display_name).bind(&user.role).bind(hash).bind(now())
        .execute(&mut **transaction).await.map_err(conflict_or_internal)?;
    Ok(())
}

async fn patch_user(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    ApiJson(input): ApiJson<PatchUserInput>,
) -> Result<Json<User>, AuthError> {
    let actor = authenticate(&headers, &state).await?;
    require_admin(&actor)?;
    check_origin(&headers, &state.config, false)?;
    valid_id(&id)?;
    if input.is_active.is_none() && input.password.is_none() {
        return Err(AuthError::invalid());
    }
    let hash = if let Some(password) = input.password {
        if !password_valid(&password) {
            return Err(AuthError::invalid());
        }
        Some(hash_password(password, state.hashing.clone()).await?)
    } else {
        None
    };
    let mut transaction = actor_transaction(&state, &actor, true).await?;
    let record = sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE id=?")
        .bind(&id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?
        .ok_or_else(AuthError::invalid)?;
    if input.is_active == Some(false) {
        if id == actor.user.id {
            return Err(AuthError::conflict());
        }
        if record.role == "admin" && record.is_active {
            let count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM users WHERE role='admin' AND is_active=1",
            )
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| AuthError::internal())?;
            if count <= 1 {
                return Err(AuthError::conflict());
            }
        }
    }
    if let Some(active) = input.is_active {
        sqlx::query("UPDATE users SET is_active=? WHERE id=?")
            .bind(active)
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| AuthError::internal())?;
    }
    if let Some(hash) = &hash {
        sqlx::query("UPDATE users SET password_hash=? WHERE id=?")
            .bind(hash)
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| AuthError::internal())?;
    }
    if input.is_active == Some(false) || hash.is_some() {
        sqlx::query("UPDATE sessions SET revoked_at=? WHERE user_id=?")
            .bind(now())
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| AuthError::internal())?;
    }
    if input.is_active == Some(false) {
        sqlx::query("DELETE FROM pairs WHERE tracker_id=? OR partner_id=?")
            .bind(&id)
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| AuthError::internal())?;
    }
    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    let mut user = record.public();
    if let Some(active) = input.is_active {
        user.is_active = active;
    }
    Ok(Json(user))
}

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

async fn pairs(
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

async fn create_pair(
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
    let rows = sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE id=? OR id=?")
        .bind(&input.tracker_id)
        .bind(&input.partner_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| AuthError::internal())?;
    let tracker = rows
        .iter()
        .find(|row| row.id == input.tracker_id && row.role == "tracker" && row.is_active)
        .ok_or_else(AuthError::invalid)?
        .public();
    let partner = rows
        .iter()
        .find(|row| row.id == input.partner_id && row.role == "partner" && row.is_active)
        .ok_or_else(AuthError::invalid)?
        .public();
    let pair = Pair {
        id: Uuid::new_v4().to_string(),
        tracker,
        partner,
        created_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|_| AuthError::internal())?,
    };
    sqlx::query("INSERT INTO pairs(id,tracker_id,partner_id,created_at) VALUES(?,?,?,?)")
        .bind(&pair.id)
        .bind(&pair.tracker.id)
        .bind(&pair.partner.id)
        .bind(&pair.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(conflict_or_internal)?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    Ok((StatusCode::CREATED, Json(pair)).into_response())
}

async fn my_pair(
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

async fn unlink_my_pair(
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

async fn unlink_pair(
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

/// Local bootstrap only. The transaction prevents concurrent first-admin creation.
pub async fn bootstrap_admin(
    pool: &SqlitePool,
    username: &str,
    display: &str,
    password: String,
) -> Result<User, StartupError> {
    let username = normalized_username(username)
        .map_err(|_| "username must be 3–64 ASCII characters [a-z0-9_.-]")?;
    let display = display_name(display).map_err(|_| "display name must be 1–80 characters")?;
    if !password_valid(&password) {
        return Err("password must be 12–128 characters and at most 512 bytes".into());
    }
    let hash = hash_password(password, Arc::new(Semaphore::new(1)))
        .await
        .map_err(|_| "password hashing failed")?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE role='admin'")
        .fetch_one(&mut *transaction)
        .await?;
    if count != 0 {
        return Err(
            "an administrator already exists; create further accounts through the admin API".into(),
        );
    }
    let user = User {
        id: Uuid::new_v4().to_string(),
        username,
        display_name: display,
        role: "admin".into(),
        is_active: true,
    };
    insert_user(&mut transaction, &user, &hash)
        .await
        .map_err(|_| "administrator creation failed")?;
    transaction.commit().await?;
    Ok(user)
}
