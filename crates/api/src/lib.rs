use std::{error::Error, path::Path, str::FromStr, time::Duration};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

mod auth;
mod config;
mod cycle;

pub use auth::{Pair, User, bootstrap_admin};
pub use config::AuthConfig;

pub type StartupError = Box<dyn Error + Send + Sync>;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Opens the local database and applies each embedded migration once.
pub async fn init_database(database_url: &str) -> Result<SqlitePool, StartupError> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));

    let filename = options.get_filename();
    if filename != Path::new(":memory:") {
        if let Some(parent) = filename
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            create_private_directory(parent)?;
        }
        let mut file = std::fs::OpenOptions::new();
        file.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            file.mode(0o600);
        }
        match file.open(filename) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await?;

    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

pub(crate) fn create_private_directory(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// No CORS middleware: browser clients use a same-origin reverse proxy.
pub fn app(pool: SqlitePool) -> Router {
    app_with_config(pool, AuthConfig::ephemeral())
}

pub fn app_with_config(pool: SqlitePool, config: AuthConfig) -> Router {
    Router::new()
        .route("/api/message", get(message))
        .route("/health", get(health))
        .with_state(pool.clone())
        .merge(auth::router(pool, config))
        .fallback(not_found)
}

#[derive(Serialize)]
struct MessageResponse {
    message: String,
    source: &'static str,
}

async fn message(State(pool): State<SqlitePool>) -> Result<Response, ApiError> {
    let message = sqlx::query_scalar::<_, String>("SELECT message FROM demo_messages WHERE id = ?")
        .bind(1_i64)
        .fetch_optional(&pool)
        .await
        .map_err(|_| ApiError::database_unavailable())?
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "demo_message_not_found",
        })?;

    let mut response = Json(MessageResponse {
        message,
        source: "sqlite",
    })
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    database: &'static str,
}

async fn health(State(pool): State<SqlitePool>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|_| ApiError::database_unavailable())?;
    Ok(Json(HealthResponse {
        status: "ok",
        database: "ok",
    }))
}

async fn not_found() -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        error: "not_found",
    }
}

struct ApiError {
    status: StatusCode,
    error: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

impl ApiError {
    fn database_unavailable() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            error: "database_unavailable",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorResponse { error: self.error })).into_response()
    }
}
