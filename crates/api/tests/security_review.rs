use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use flo_api::{AuthConfig, app_with_config, bootstrap_admin, init_database};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tempfile::TempDir;
use tower::ServiceExt;

const SECRET: &str = "independent-review-key-0123456789abcdef";
const PASSWORD: &str = "Synthetic-review-password-2026";
const ORIGIN: &str = "https://flo.example";

struct Fixture {
    app: Router,
    pool: SqlitePool,
    _directory: TempDir,
}

impl Fixture {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let pool = init_database(&format!(
            "sqlite://{}/review.db",
            directory.path().display()
        ))
        .await
        .unwrap();
        bootstrap_admin(&pool, "admin", "Review admin", PASSWORD.into())
            .await
            .unwrap();
        let config = AuthConfig::new(SECRET, vec![ORIGIN.into()], true).unwrap();
        Self {
            app: app_with_config(pool.clone(), config),
            pool,
            _directory: directory,
        }
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        headers: &[(&str, String)],
    ) -> (StatusCode, HeaderMap, Value) {
        let mut request = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        let body = match body {
            Some(body) => {
                request = request.header("content-type", "application/json");
                body.to_string()
            }
            None => String::new(),
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("stable JSON response")
        };
        (status, headers, value)
    }

    async fn login(&self, username: &str) -> Value {
        let (status, _, value) = self
            .request(
                "POST",
                "/api/auth/login",
                Some(json!({"username": username, "password": PASSWORD, "client":"native"})),
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        value
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn bearer(login: &Value) -> String {
    format!("Bearer {}", login["access_token"].as_str().unwrap())
}

#[tokio::test]
async fn ambiguous_authentication_and_origin_headers_fail_closed() {
    let fixture = Fixture::new().await;
    let admin = fixture.login("admin").await;
    let cookie = format!("flo_access={}", admin["access_token"].as_str().unwrap());
    for headers in [
        vec![("cookie", cookie.clone()), ("cookie", cookie.clone())],
        vec![("cookie", format!("{cookie}; {cookie}"))],
        vec![
            ("authorization", bearer(&admin)),
            ("authorization", bearer(&admin)),
        ],
    ] {
        let (status, headers, value) = fixture.request("GET", "/api/me", None, &headers).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(value, json!({"error":"unauthorized"}));
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
    }
    let (status, _, value) = fixture
        .request(
            "DELETE",
            "/api/me/pair",
            None,
            &[
                ("authorization", bearer(&admin)),
                ("origin", ORIGIN.into()),
                ("origin", "https://evil.example".into()),
            ],
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(value, json!({"error":"forbidden"}));
}

#[tokio::test]
async fn session_identity_is_bound_to_subject_and_role_comes_from_database() {
    let fixture = Fixture::new().await;
    let admin = fixture.login("admin").await;
    let (status, _, tracker) = fixture
        .request(
            "POST",
            "/api/admin/users",
            Some(json!({"username":"tracker","display_name":"Review tracker","password":PASSWORD,"role":"tracker"})),
            &[("authorization", bearer(&admin))],
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    fixture.login("tracker").await;
    let admin_sid: String = sqlx::query_scalar("SELECT id FROM sessions WHERE user_id=?")
        .bind(admin["user"]["id"].as_str().unwrap())
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    let tracker_sid: String = sqlx::query_scalar("SELECT id FROM sessions WHERE user_id=?")
        .bind(tracker["id"].as_str().unwrap())
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    for (sid, expected) in [
        (admin_sid, StatusCode::UNAUTHORIZED),
        (tracker_sid, StatusCode::FORBIDDEN),
    ] {
        let claims = json!({
            "sub":tracker["id"], "sid":sid, "iat":now(), "exp":now()+900,
            "iss":"open-source-flo", "aud":"flo-clients", "role":"admin"
        });
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap();
        let (status, _, _) = fixture
            .request(
                "GET",
                "/api/admin/users",
                None,
                &[("authorization", format!("Bearer {token}"))],
            )
            .await;
        assert_eq!(status, expected);
    }
}

#[tokio::test]
async fn refresh_cannot_extend_session_or_outlive_it_in_cookie_or_jwt() {
    let fixture = Fixture::new().await;
    let login = fixture.login("admin").await;
    let expires = now() + 30;
    sqlx::query("UPDATE sessions SET expires_at=?")
        .bind(expires)
        .execute(&fixture.pool)
        .await
        .unwrap();
    let (status, headers, value) = fixture
        .request(
            "POST",
            "/api/auth/refresh",
            None,
            &[
                (
                    "cookie",
                    format!("flo_refresh={}", login["refresh_token"].as_str().unwrap()),
                ),
                ("origin", ORIGIN.into()),
            ],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(value.get("refresh_token").is_none());
    assert!((1..=30).contains(&value["expires_in"].as_i64().unwrap()));
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&["open-source-flo"]);
    validation.set_audience(&["flo-clients"]);
    let claims = decode::<Value>(
        value["access_token"].as_str().unwrap(),
        &DecodingKey::from_secret(SECRET.as_bytes()),
        &validation,
    )
    .unwrap()
    .claims;
    assert!(claims["exp"].as_i64().unwrap() <= expires);
    let cookies = headers.get_all("set-cookie").iter().collect::<Vec<_>>();
    assert_eq!(cookies.len(), 2);
    for cookie in cookies {
        let cookie = cookie.to_str().unwrap();
        let max_age = cookie
            .split(';')
            .find_map(|field| field.trim().strip_prefix("Max-Age="))
            .unwrap()
            .parse::<i64>()
            .unwrap();
        assert!((1..=30).contains(&max_age));
        assert!(cookie.contains("HttpOnly") && cookie.contains("Secure"));
    }
    let persisted: i64 = sqlx::query_scalar("SELECT expires_at FROM sessions LIMIT 1")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(persisted, expires);
}

#[tokio::test]
async fn native_logout_after_access_expiry_uses_refresh_proof_without_bearer() {
    let fixture = Fixture::new().await;
    let login = fixture.login("admin").await;
    let sid: String = sqlx::query_scalar("SELECT id FROM sessions LIMIT 1")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    let expired = encode(
        &Header::new(Algorithm::HS256),
        &json!({
            "sub":login["user"]["id"], "sid":sid, "iat":now()-901,
            "exp":now()-1, "iss":"open-source-flo", "aud":"flo-clients"
        }),
        &EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap();
    let proof = json!({"client":"native", "refresh_token":login["refresh_token"]});
    let (status, _, _) = fixture
        .request(
            "POST",
            "/api/auth/logout",
            Some(proof.clone()),
            &[("authorization", format!("Bearer {expired}"))],
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let revoked: Option<i64> = sqlx::query_scalar("SELECT revoked_at FROM sessions WHERE id=?")
        .bind(&sid)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert!(revoked.is_none());
    // An explicit invalid bearer must not fall back. Native clients use refresh proof only.
    assert_eq!(
        fixture
            .request("POST", "/api/auth/logout", Some(proof.clone()), &[])
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        fixture
            .request("GET", "/api/me", None, &[("authorization", bearer(&login))])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture
            .request("POST", "/api/auth/refresh", Some(proof), &[])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
