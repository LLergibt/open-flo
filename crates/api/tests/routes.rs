use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use flo_api::{app, init_database};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

async fn get_json(app: Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn message_reads_current_database_value_and_survives_restart() {
    let directory = TempDir::new().unwrap();
    let url = format!("sqlite://{}/nested/demo.db", directory.path().display());
    let pool = init_database(&url).await.unwrap();

    let (status, original) = get_json(app(pool.clone()), "/api/message").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        original,
        json!({"message": "Привет из SQLite", "source": "sqlite"})
    );

    let changed = "Тест: текст изменён непосредственно в БД";
    sqlx::query("UPDATE demo_messages SET message = ? WHERE id = 1")
        .bind(changed)
        .execute(&pool)
        .await
        .unwrap();
    let (_, updated) = get_json(app(pool.clone()), "/api/message").await;
    assert_eq!(updated["message"], changed);
    pool.close().await;

    let reopened_pool = init_database(&url).await.unwrap();
    let (status, persisted) = get_json(app(reopened_pool.clone()), "/api/message").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(persisted["message"], changed);
    reopened_pool.close().await;
}

#[tokio::test]
async fn health_reflects_database_availability_without_exposing_details() {
    let directory = TempDir::new().unwrap();
    let url = format!("sqlite://{}/health.db", directory.path().display());
    let pool = init_database(&url).await.unwrap();

    assert_eq!(
        get_json(app(pool.clone()), "/health").await,
        (StatusCode::OK, json!({"status": "ok", "database": "ok"}))
    );
    pool.close().await;
    assert_eq!(
        get_json(app(pool), "/health").await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error": "database_unavailable"})
        )
    );
}

#[tokio::test]
async fn missing_message_does_not_return_a_hardcoded_fallback() {
    let directory = TempDir::new().unwrap();
    let url = format!("sqlite://{}/missing.db", directory.path().display());
    let pool = init_database(&url).await.unwrap();
    sqlx::query("DELETE FROM demo_messages")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        get_json(app(pool.clone()), "/api/message").await,
        (
            StatusCode::NOT_FOUND,
            json!({"error": "demo_message_not_found"})
        )
    );
    pool.close().await;
}
