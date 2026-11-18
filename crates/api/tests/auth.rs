use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode, header},
};
use flo_api::{AuthConfig, app_with_config, bootstrap_admin, init_database};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tempfile::TempDir;
use tower::ServiceExt;

const SECRET: &str = "0123456789abcdef0123456789abcdef";
const PASSWORD: &str = "Synthetic-test-password-2026";
const ORIGIN: &str = "http://localhost:5173";

struct Fixture {
    pool: SqlitePool,
    app: Router,
    _directory: TempDir,
}

impl Fixture {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let pool = init_database(&format!("sqlite://{}/auth.db", directory.path().display()))
            .await
            .unwrap();
        let config = AuthConfig::new(SECRET, vec![ORIGIN.into()], true).unwrap();
        let app = app_with_config(pool.clone(), config);
        Self {
            pool,
            app,
            _directory: directory,
        }
    }

    async fn admin(&self) -> Value {
        bootstrap_admin(&self.pool, "admin", "Администратор", PASSWORD.into())
            .await
            .unwrap();
        self.login("admin", PASSWORD).await
    }

    async fn login(&self, username: &str, password: &str) -> Value {
        let (status, _, body) = self
            .request(
                "POST",
                "/api/auth/login",
                Some(json!({"username":username,"password":password,"client":"native"})),
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn create(&self, token: &str, username: &str, role: &str) -> Value {
        let (status, _, body) = self.request("POST", "/api/admin/users", Some(json!({"username":username,"display_name":username,"password":PASSWORD,"role":role})), &[("authorization", &format!("Bearer {token}"))]).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        body
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> (StatusCode, HeaderMap, Value) {
        let mut request = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let body = if let Some(body) = body {
            request = request.header("content-type", "application/json");
            body.to_string()
        } else {
            String::new()
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("JSON API response")
        };
        (status, headers, json)
    }
}

fn access(response: &Value) -> &str {
    response["access_token"].as_str().unwrap()
}
fn refresh(response: &Value) -> &str {
    response["refresh_token"].as_str().unwrap()
}
fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}
fn cookie_header(headers: &HeaderMap) -> String {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|cookie| cookie.to_str().unwrap().split(';').next().unwrap())
        .collect::<Vec<_>>()
        .join("; ")
}
fn timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[tokio::test]
async fn bootstrap_password_validation_and_login_errors() {
    let fixture = Fixture::new().await;
    assert_eq!(
        fixture
            .request("GET", "/api/setup/status", None, &[])
            .await
            .2,
        json!({"needs_setup":true})
    );
    let admin = fixture.admin().await;
    assert_eq!(
        fixture
            .request("GET", "/api/setup/status", None, &[])
            .await
            .2,
        json!({"needs_setup":false})
    );
    assert!(
        bootstrap_admin(&fixture.pool, "another-admin", "Другой", PASSWORD.into())
            .await
            .is_err()
    );
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE username='admin'")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
    assert!(!admin.to_string().contains("password_hash"));
    let username = fixture.login(" ADMIN ", PASSWORD).await;
    assert_eq!(username["user"]["username"], "admin");
    for name in ["admin", "not-a-user"] {
        let (status, _, error) = fixture
            .request(
                "POST",
                "/api/auth/login",
                Some(json!({"username":name,"password":"Wrong-password-2026","client":"native"})),
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(error, json!({"error":"invalid_credentials"}));
    }
}

#[tokio::test]
async fn jwt_signature_expiry_issuer_audience_and_bearer_precedence_are_enforced() {
    let fixture = Fixture::new().await;
    let admin = fixture.admin().await;
    let auth = bearer(access(&admin));
    assert_eq!(
        fixture
            .request("GET", "/api/me", None, &[("authorization", &auth)])
            .await
            .0,
        StatusCode::OK
    );
    let sid: String = sqlx::query_scalar("SELECT id FROM sessions LIMIT 1")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    let base = json!({"sub":admin["user"]["id"],"sid":sid,"iat":timestamp(),"exp":timestamp()+900,"iss":"open-source-flo","aud":"flo-clients"});
    for (field, value) in [
        ("exp", json!(timestamp() - 1)),
        ("iss", json!("other")),
        ("aud", json!("other")),
        ("iat", json!(timestamp() + 60)),
    ] {
        let mut claims = base.clone();
        claims[field] = value;
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap();
        assert_eq!(
            fixture
                .request(
                    "GET",
                    "/api/me",
                    None,
                    &[("authorization", &bearer(&token))]
                )
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    let bad_signature = encode(
        &Header::new(Algorithm::HS256),
        &base,
        &EncodingKey::from_secret(b"different-signing-key"),
    )
    .unwrap();
    let wrong_algorithm = encode(
        &Header::new(Algorithm::HS384),
        &base,
        &EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap();
    for token in [
        bad_signature.as_str(),
        wrong_algorithm.as_str(),
        "malformed",
    ] {
        assert_eq!(
            fixture
                .request("GET", "/api/me", None, &[("authorization", &bearer(token))])
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    let cookies = format!("flo_access={}", access(&admin));
    assert_eq!(
        fixture
            .request("GET", "/api/me", None, &[("cookie", &cookies)])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .request(
                "GET",
                "/api/me",
                None,
                &[("cookie", &cookies), ("authorization", "Bearer malformed")]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    sqlx::query("UPDATE sessions SET expires_at=0")
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .request("GET", "/api/me", None, &[("authorization", &auth)])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn refresh_is_hashed_rotates_and_any_earlier_replay_revokes_the_family() {
    let fixture = Fixture::new().await;
    let admin = fixture.admin().await;
    let request = |token: &str| json!({"refresh_token":token,"client":"native"});
    let first = fixture
        .request(
            "POST",
            "/api/auth/refresh",
            Some(request(refresh(&admin))),
            &[],
        )
        .await;
    assert_eq!(first.0, StatusCode::OK);
    assert_ne!(refresh(&first.2), refresh(&admin));
    let second = fixture
        .request(
            "POST",
            "/api/auth/refresh",
            Some(request(refresh(&first.2))),
            &[],
        )
        .await;
    assert_eq!(second.0, StatusCode::OK);
    let hashes: Vec<String> = sqlx::query_scalar("SELECT token_hash FROM refresh_tokens")
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
    assert!(!hashes.iter().any(|hash| hash == refresh(&admin)
        || hash == refresh(&first.2)
        || hash == refresh(&second.2)));
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/auth/refresh",
                Some(request(refresh(&admin))),
                &[]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture
            .request(
                "GET",
                "/api/me",
                None,
                &[("authorization", &bearer(access(&second.2)))]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/auth/refresh",
                Some(request(refresh(&second.2))),
                &[]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn concurrent_refresh_allows_one_rotation_then_revokes_on_replay() {
    let fixture = Fixture::new().await;
    let admin = fixture.admin().await;
    let body = json!({"refresh_token":refresh(&admin),"client":"native"});
    let (first, second) = tokio::join!(
        fixture.request("POST", "/api/auth/refresh", Some(body.clone()), &[]),
        fixture.request("POST", "/api/auth/refresh", Some(body), &[])
    );
    let mut statuses = [first.0.as_u16(), second.0.as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 401]);
    assert_eq!(
        fixture
            .request(
                "GET",
                "/api/me",
                None,
                &[("authorization", &bearer(access(&admin)))]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn web_cookies_csrf_and_logout_with_expired_access() {
    let fixture = Fixture::new().await;
    fixture.admin().await;
    let login = json!({"username":"admin","password":PASSWORD});
    assert_eq!(
        fixture
            .request("POST", "/api/auth/login", Some(login.clone()), &[])
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let native = json!({"username":"admin","password":PASSWORD,"client":"native"});
    for headers in [
        vec![("origin", "https://evil.example")],
        vec![("cookie", "anything=present")],
    ] {
        assert_eq!(
            fixture
                .request("POST", "/api/auth/login", Some(native.clone()), &headers)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let (status, headers, response) = fixture
        .request(
            "POST",
            "/api/auth/login",
            Some(login),
            &[("origin", ORIGIN)],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(response.get("refresh_token").is_none());
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
    for cookie in headers.get_all("set-cookie") {
        let cookie = cookie.to_str().unwrap();
        for flag in ["HttpOnly", "SameSite=Strict", "Path=/api", "Secure"] {
            assert!(cookie.contains(flag));
        }
    }
    let cookies = cookie_header(&headers);
    assert_eq!(
        fixture
            .request("GET", "/api/me", None, &[("cookie", &cookies)])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .request("POST", "/api/auth/refresh", None, &[("cookie", &cookies)])
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/auth/refresh",
                Some(json!({"client":"native","refresh_token":"a".repeat(64)})),
                &[("cookie", &cookies)]
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, refreshed_headers, _) = fixture
        .request(
            "POST",
            "/api/auth/refresh",
            None,
            &[("cookie", &cookies), ("origin", ORIGIN)],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let refreshed = cookie_header(&refreshed_headers);
    let refresh_cookie = refreshed
        .split("; ")
        .find(|value| value.starts_with("flo_refresh="))
        .unwrap();
    let user_id = response["user"]["id"].as_str().unwrap();
    let sid: String = sqlx::query_scalar(
        "SELECT id FROM sessions WHERE user_id=? ORDER BY created_at DESC,rowid DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    let expired = encode(&Header::new(Algorithm::HS256),&json!({"sub":user_id,"sid":sid,"iat":timestamp()-1000,"exp":timestamp()-100,"iss":"open-source-flo","aud":"flo-clients"}),&EncodingKey::from_secret(SECRET.as_bytes())).unwrap();
    let expired_cookies = format!("flo_access={expired}; {refresh_cookie}");
    let (status, cleared, _) = fixture
        .request(
            "POST",
            "/api/auth/logout",
            None,
            &[("cookie", &expired_cookies), ("origin", ORIGIN)],
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        cleared
            .get_all("set-cookie")
            .iter()
            .all(|cookie| cookie.to_str().unwrap().contains("Max-Age=0"))
    );
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/auth/refresh",
                None,
                &[("cookie", &refreshed), ("origin", ORIGIN)]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn native_logout_revokes_access_and_refresh() {
    let fixture = Fixture::new().await;
    let admin = fixture.admin().await;
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/auth/logout",
                None,
                &[("authorization", &bearer(access(&admin)))]
            )
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        fixture
            .request(
                "GET",
                "/api/me",
                None,
                &[("authorization", &bearer(access(&admin)))]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/auth/refresh",
                Some(json!({"client":"native","refresh_token":refresh(&admin)})),
                &[]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn admin_pair_membership_roles_isolation_and_unlink() {
    let fixture = Fixture::new().await;
    let admin = fixture.admin().await;
    let auth = bearer(access(&admin));
    let tracker = fixture.create(access(&admin), "tracker", "tracker").await;
    let partner = fixture.create(access(&admin), "partner", "partner").await;
    let other_tracker = fixture
        .create(access(&admin), "othertracker", "tracker")
        .await;
    let other_partner = fixture
        .create(access(&admin), "otherpartner", "partner")
        .await;
    let tracker_login = fixture.login("tracker", PASSWORD).await;
    let partner_login = fixture.login("partner", PASSWORD).await;
    let other_login = fixture.login("othertracker", PASSWORD).await;
    for login in [&tracker_login, &partner_login] {
        assert_eq!(
            fixture
                .request(
                    "GET",
                    "/api/admin/users",
                    None,
                    &[("authorization", &bearer(access(login)))]
                )
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            fixture
                .request(
                    "GET",
                    "/api/admin/pairs",
                    None,
                    &[("authorization", &bearer(access(login)))]
                )
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        fixture
            .request("GET", "/api/admin/users", None, &[])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let path = "/api/admin/pairs";
    for body in [
        json!({"tracker_id":tracker["id"],"partner_id":tracker["id"]}),
        json!({"tracker_id":partner["id"],"partner_id":tracker["id"]}),
        json!({"tracker_id":uuid::Uuid::new_v4().to_string(),"partner_id":partner["id"]}),
    ] {
        assert_eq!(
            fixture
                .request("POST", path, Some(body), &[("authorization", &auth)])
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let body = json!({"tracker_id":tracker["id"],"partner_id":partner["id"]});
    let created = fixture
        .request(
            "POST",
            path,
            Some(body.clone()),
            &[("authorization", &auth)],
        )
        .await;
    assert_eq!(created.0, StatusCode::CREATED);
    assert!(!created.2.to_string().contains("password_hash"));
    assert_eq!(
        fixture
            .request("POST", path, Some(body), &[("authorization", &auth)])
            .await
            .0,
        StatusCode::CONFLICT
    );
    let another = fixture
        .request(
            "POST",
            path,
            Some(json!({"tracker_id":other_tracker["id"],"partner_id":other_partner["id"]})),
            &[("authorization", &auth)],
        )
        .await;
    assert_eq!(another.0, StatusCode::CREATED);
    let own = fixture
        .request(
            "GET",
            "/api/me/pair",
            None,
            &[("authorization", &bearer(access(&partner_login)))],
        )
        .await;
    assert_eq!(own.2["pair"]["id"], created.2["id"]);
    let other = fixture
        .request(
            "GET",
            "/api/me/pair",
            None,
            &[("authorization", &bearer(access(&other_login)))],
        )
        .await;
    assert_eq!(other.2["pair"]["id"], another.2["id"]);
    assert_eq!(
        fixture
            .request(
                "DELETE",
                "/api/me/pair",
                None,
                &[("authorization", &bearer(access(&partner_login)))]
            )
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        fixture
            .request(
                "GET",
                "/api/me/pair",
                None,
                &[("authorization", &bearer(access(&tracker_login)))]
            )
            .await
            .2,
        json!({"pair":null})
    );
    assert_eq!(
        fixture
            .request(
                "GET",
                "/api/me/pair",
                None,
                &[("authorization", &bearer(access(&other_login)))]
            )
            .await
            .2["pair"]["id"],
        another.2["id"]
    );
}

#[tokio::test]
async fn simultaneous_pair_creation_is_atomic() {
    let fixture = Fixture::new().await;
    let admin = fixture.admin().await;
    let auth = bearer(access(&admin));
    let tracker = fixture.create(access(&admin), "tracker", "tracker").await;
    let first_partner = fixture
        .create(access(&admin), "firstpartner", "partner")
        .await;
    let second_partner = fixture
        .create(access(&admin), "secondpartner", "partner")
        .await;
    let first_body = json!({"tracker_id":tracker["id"],"partner_id":first_partner["id"]});
    let second_body = json!({"tracker_id":tracker["id"],"partner_id":second_partner["id"]});
    let headers = [("authorization", auth.as_str())];
    let (first, second) = tokio::join!(
        fixture.request("POST", "/api/admin/pairs", Some(first_body), &headers),
        fixture.request("POST", "/api/admin/pairs", Some(second_body), &headers)
    );
    let mut statuses = [first.0.as_u16(), second.0.as_u16()];
    statuses.sort();
    assert_eq!(statuses, [201, 409]);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pairs")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn disable_password_reset_revoke_sessions_and_self_disable_is_blocked() {
    let fixture = Fixture::new().await;
    let admin = fixture.admin().await;
    let auth = bearer(access(&admin));
    let tracker = fixture.create(access(&admin), "tracker", "tracker").await;
    let tracker_login = fixture.login("tracker", PASSWORD).await;
    let path = format!("/api/admin/users/{}", tracker["id"].as_str().unwrap());
    let changed = "Another-synthetic-password-2026";
    assert_eq!(
        fixture
            .request(
                "PATCH",
                &path,
                Some(json!({"password":changed})),
                &[("authorization", &auth)]
            )
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .request(
                "GET",
                "/api/me",
                None,
                &[("authorization", &bearer(access(&tracker_login)))]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/auth/refresh",
                Some(json!({"client":"native","refresh_token":refresh(&tracker_login)})),
                &[]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let after_reset = fixture.login("tracker", changed).await;
    assert_eq!(
        fixture
            .request(
                "PATCH",
                &path,
                Some(json!({"is_active":false})),
                &[("authorization", &auth)]
            )
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .request(
                "GET",
                "/api/me",
                None,
                &[("authorization", &bearer(access(&after_reset)))]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/auth/login",
                Some(json!({"username":"tracker","password":changed,"client":"native"})),
                &[]
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let self_path = format!("/api/admin/users/{}", admin["user"]["id"].as_str().unwrap());
    assert_eq!(
        fixture
            .request(
                "PATCH",
                &self_path,
                Some(json!({"is_active":false})),
                &[("authorization", &auth)]
            )
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        fixture
            .request(
                "PATCH",
                &path,
                Some(json!({"role":"admin"})),
                &[("authorization", &auth)]
            )
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn concurrent_admin_disabling_cannot_remove_last_active_administrator() {
    let fixture = Fixture::new().await;
    let first = fixture.admin().await;
    let second_user = fixture.create(access(&first), "secondadmin", "admin").await;
    let second = fixture.login("secondadmin", PASSWORD).await;
    let first_path = format!("/api/admin/users/{}", first["user"]["id"].as_str().unwrap());
    let second_path = format!("/api/admin/users/{}", second_user["id"].as_str().unwrap());
    let first_auth = bearer(access(&first));
    let second_auth = bearer(access(&second));
    let first_headers = [("authorization", first_auth.as_str())];
    let second_headers = [("authorization", second_auth.as_str())];
    let (one, two) = tokio::join!(
        fixture.request(
            "PATCH",
            &second_path,
            Some(json!({"is_active":false})),
            &first_headers
        ),
        fixture.request(
            "PATCH",
            &first_path,
            Some(json!({"is_active":false})),
            &second_headers
        )
    );
    let active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role='admin' AND is_active=1")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(active, 1);
    assert!(one.0 == StatusCode::OK || two.0 == StatusCode::OK);
    assert!(one.0 == StatusCode::UNAUTHORIZED || two.0 == StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn malformed_oversized_input_rate_limit_and_cookie_mutation_checks() {
    let fixture = Fixture::new().await;
    let admin = fixture.admin().await;
    let cookies = format!("flo_access={}", access(&admin));
    assert_eq!(
        fixture
            .request("DELETE", "/api/me/pair", None, &[("cookie", &cookies)])
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        fixture
            .request(
                "DELETE",
                "/api/me/pair",
                None,
                &[("cookie", &cookies), ("origin", "https://evil.example")]
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let response = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from("{".to_owned() + &" ".repeat(9000)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = to_bytes(response.into_body(), 1000).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"error":"invalid_input"})
    );
    for attempt in 0..6 {
        let response = fixture
            .request(
                "POST",
                "/api/auth/login",
                Some(json!({"username":"unknown","password":PASSWORD,"client":"native"})),
                &[],
            )
            .await;
        assert_eq!(
            response.0,
            if attempt < 5 {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
        assert_eq!(response.1.get("cache-control").unwrap(), "no-store");
    }
}
