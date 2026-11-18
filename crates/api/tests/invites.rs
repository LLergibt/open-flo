use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use flo_api::{AuthConfig, app_with_config, bootstrap_admin, init_database};
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

async fn invitation(f: &Fixture, tracker: &str) -> Value {
    let (status, _, value) = f
        .request(
            "POST",
            "/api/me/invite",
            Some(json!({})),
            &[("authorization", &format!("Bearer {tracker}"))],
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{value}");
    value
}
async fn preview(f: &Fixture, token: &str) -> StatusCode {
    f.request(
        "POST",
        "/api/invites/preview",
        Some(json!({"token":token})),
        &[],
    )
    .await
    .0
}

#[tokio::test]
async fn owner_links_are_hashed_rotated_revocable_and_role_scoped() {
    let f = Fixture::new().await;
    let a = f.admin().await;
    let at = a["access_token"].as_str().unwrap();
    f.create(at, "owner", "tracker").await;
    f.create(at, "partner", "partner").await;
    let o = f.login("owner", PASSWORD).await;
    let ot = o["access_token"].as_str().unwrap();
    let p = f.login("partner", PASSWORD).await;
    assert_eq!(
        f.request(
            "POST",
            "/api/me/invite",
            Some(json!({})),
            &[(
                "authorization",
                &format!("Bearer {}", p["access_token"].as_str().unwrap())
            )]
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let first = invitation(&f, ot).await;
    let token = first["token"].as_str().unwrap();
    assert_eq!(preview(&f, token).await, StatusCode::OK);
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM partner_invites")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_ne!(stored, token);
    assert_eq!(stored.len(), 64);
    let second = invitation(&f, ot).await;
    assert_eq!(preview(&f, token).await, StatusCode::NOT_FOUND);
    assert_eq!(
        f.request(
            "DELETE",
            "/api/me/invite",
            None,
            &[("authorization", &format!("Bearer {ot}"))]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        preview(&f, second["token"].as_str().unwrap()).await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn invitation_registration_is_atomic_one_time_and_always_partner() {
    let f = Fixture::new().await;
    let a = f.admin().await;
    let at = a["access_token"].as_str().unwrap();
    f.create(at, "owner", "tracker").await;
    let owner = f.login("owner", PASSWORD).await;
    let invite = invitation(&f, owner["access_token"].as_str().unwrap()).await;
    let token = invite["token"].as_str().unwrap();
    let forbidden = json!({"token":token,"username":"escalation","display_name":"Test","password":PASSWORD,"client":"native","role":"admin"});
    assert_eq!(
        f.request("POST", "/api/invites/register", Some(forbidden), &[])
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let one = json!({"token":token,"username":"new_one","display_name":"One","password":PASSWORD,"client":"native"});
    let two = json!({"token":token,"username":"new_two","display_name":"Two","password":PASSWORD,"client":"native"});
    let (x, y) = tokio::join!(
        f.request("POST", "/api/invites/register", Some(one), &[]),
        f.request("POST", "/api/invites/register", Some(two), &[])
    );
    let mut statuses = [x.0.as_u16(), y.0.as_u16()];
    statuses.sort();
    assert_eq!(statuses, [201, 404]);
    let success = if x.0 == StatusCode::CREATED { x.2 } else { y.2 };
    assert_eq!(success["user"]["role"], "partner");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE username IN ('new_one','new_two','escalation')",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    assert_eq!(preview(&f, token).await, StatusCode::NOT_FOUND);
    let login = f
        .login(success["user"]["username"].as_str().unwrap(), PASSWORD)
        .await;
    assert!(login["access_token"].is_string());
}

#[tokio::test]
async fn expired_disabled_or_admin_consumed_links_never_revive() {
    let f = Fixture::new().await;
    let a = f.admin().await;
    let at = a["access_token"].as_str().unwrap();
    let user = f.create(at, "owner", "tracker").await;
    let partner = f.create(at, "partner", "partner").await;
    let owner = f.login("owner", PASSWORD).await;
    let ot = owner["access_token"].as_str().unwrap();
    let link = invitation(&f, ot).await;
    let path = format!("/api/admin/users/{}", user["id"].as_str().unwrap());
    for active in [false, true] {
        assert_eq!(
            f.request(
                "PATCH",
                &path,
                Some(json!({"is_active":active})),
                &[("authorization", &format!("Bearer {at}"))]
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        preview(&f, link["token"].as_str().unwrap()).await,
        StatusCode::NOT_FOUND
    );
    let owner = f.login("owner", PASSWORD).await;
    let link = invitation(&f, owner["access_token"].as_str().unwrap()).await;
    let pair = f
        .request(
            "POST",
            "/api/admin/pairs",
            Some(json!({"tracker_id":user["id"],"partner_id":partner["id"]})),
            &[("authorization", &format!("Bearer {at}"))],
        )
        .await;
    assert_eq!(pair.0, StatusCode::CREATED);
    f.request(
        "DELETE",
        &format!("/api/admin/pairs/{}", pair.2["id"].as_str().unwrap()),
        None,
        &[("authorization", &format!("Bearer {at}"))],
    )
    .await;
    assert_eq!(
        preview(&f, link["token"].as_str().unwrap()).await,
        StatusCode::NOT_FOUND
    );
    let link = invitation(&f, owner["access_token"].as_str().unwrap()).await;
    sqlx::query("UPDATE partner_invites SET expires_at=0")
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        preview(&f, link["token"].as_str().unwrap()).await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn own_password_requires_old_password_and_revokes_sessions_and_invites() {
    let f = Fixture::new().await;
    let a = f.admin().await;
    let at = a["access_token"].as_str().unwrap();
    f.create(at, "owner", "tracker").await;
    let owner = f.login("owner", PASSWORD).await;
    let token = owner["access_token"].as_str().unwrap();
    let headers = [("authorization", format!("Bearer {token}"))];
    let borrowed = [("authorization", headers[0].1.as_str())];
    let invite = invitation(&f, token).await;
    let wrong=f.request("POST","/api/me/password",Some(json!({"current_password":"Wrong-password-123","new_password":"Updated-password-2026"})),&borrowed).await;
    assert_eq!(wrong.0, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.2["error"], "invalid_credentials");
    assert_eq!(
        f.request("GET", "/api/me", None, &borrowed).await.0,
        StatusCode::OK
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/me/password",
            Some(json!({"current_password":PASSWORD,"new_password":"Updated-password-2026"})),
            &borrowed
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.request("GET", "/api/me", None, &borrowed).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        preview(&f, invite["token"].as_str().unwrap()).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/auth/refresh",
            Some(json!({"client":"native","refresh_token":owner["refresh_token"]})),
            &[]
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    f.login("owner", "Updated-password-2026").await;
}

#[tokio::test]
async fn existing_partner_accepts_only_after_explicit_post_and_preview_is_non_consuming() {
    let f = Fixture::new().await;
    let a = f.admin().await;
    let at = a["access_token"].as_str().unwrap();
    f.create(at, "owner", "tracker").await;
    f.create(at, "partner", "partner").await;
    let owner = f.login("owner", PASSWORD).await;
    let partner = f.login("partner", PASSWORD).await;
    let link = invitation(&f, owner["access_token"].as_str().unwrap()).await;
    let token = link["token"].as_str().unwrap();
    assert_eq!(preview(&f, token).await, StatusCode::OK);
    assert_eq!(preview(&f, token).await, StatusCode::OK);
    let headers = [(
        "authorization",
        format!("Bearer {}", partner["access_token"].as_str().unwrap()),
    )];
    let borrowed = [("authorization", headers[0].1.as_str())];
    let accepted = f
        .request(
            "POST",
            "/api/me/invite/accept",
            Some(json!({"token":token})),
            &borrowed,
        )
        .await;
    assert_eq!(accepted.0, StatusCode::CREATED);
    assert_eq!(
        f.request(
            "POST",
            "/api/me/invite/accept",
            Some(json!({"token":token})),
            &borrowed
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn failed_registration_keeps_invite_and_cookie_mutations_require_origin() {
    let f = Fixture::new().await;
    let admin = f.admin().await;
    f.create(admin["access_token"].as_str().unwrap(), "owner", "tracker")
        .await;
    let owner = f.login("owner", PASSWORD).await;
    let token = invitation(&f, owner["access_token"].as_str().unwrap()).await["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let body = json!({"token":token,"username":"owner","display_name":"Partner","password":PASSWORD,"client":"native"});
    assert_eq!(
        f.request("POST", "/api/invites/register", Some(body), &[])
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(preview(&f, &token).await, StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pairs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    let (_, headers, _) = f
        .request(
            "POST",
            "/api/auth/login",
            Some(json!({"username":"owner","password":PASSWORD,"client":"web"})),
            &[("origin", ORIGIN)],
        )
        .await;
    let cookies = headers
        .get_all("set-cookie")
        .iter()
        .map(|value| value.to_str().unwrap().split(';').next().unwrap())
        .collect::<Vec<_>>()
        .join("; ");
    for route in ["/api/me/invite", "/api/me/password"] {
        assert_eq!(f.request("POST", route, Some(json!({"current_password":PASSWORD,"new_password":"Another-synthetic-password-2026"})), &[("cookie",&cookies)]).await.0, StatusCode::FORBIDDEN);
    }
    assert_eq!(f.request("POST","/api/invites/register",Some(json!({"token":token,"username":"partner_new","display_name":"Partner","password":PASSWORD,"client":"web"})),&[("origin","https://evil.example")]).await.0,StatusCode::FORBIDDEN);
    assert_eq!(preview(&f, &token).await, StatusCode::OK);
}

#[tokio::test]
async fn account_deletion_checks_permissions_password_and_cascades() {
    let f = Fixture::new().await;
    let admin = f.admin().await;
    let a = admin["access_token"].as_str().unwrap();
    let owner = f.create(a, "delete_owner", "tracker").await;
    let partner = f.create(a, "delete_partner", "partner").await;
    let login = f.login("delete_owner", PASSWORD).await;
    let bearer = format!("Bearer {}", login["access_token"].as_str().unwrap());
    let target = format!("/api/admin/users/{}", partner["id"].as_str().unwrap());
    assert_eq!(
        f.request("DELETE", &target, None, &[("authorization", &bearer)])
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request(
            "DELETE",
            "/api/me",
            Some(json!({"current_password":"wrong"})),
            &[("authorization", &bearer)]
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request("GET", "/api/me", None, &[("authorization", &bearer)])
            .await
            .0,
        StatusCode::OK
    );
    let admin_bearer = format!("Bearer {a}");
    assert_eq!(
        f.request(
            "POST",
            "/api/admin/pairs",
            Some(json!({"tracker_id":owner["id"],"partner_id":partner["id"]})),
            &[("authorization", &admin_bearer)]
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(
        f.request(
            "DELETE",
            "/api/me",
            Some(json!({"current_password":PASSWORD})),
            &[("authorization", &bearer)]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.request("GET", "/api/me", None, &[("authorization", &bearer)])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/auth/refresh",
            Some(json!({"refresh_token":login["refresh_token"],"client":"native"})),
            &[]
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    for table in ["pairs", "sessions", "refresh_tokens"] {
        // The administrator still has its own session and refresh token.
        let expected = if table == "pairs" { 0 } else { 1 };
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&f.pool)
                .await
                .unwrap(),
            expected
        );
    }
    assert_eq!(
        f.request("DELETE", &target, None, &[("authorization", &admin_bearer)])
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        f.request(
            "DELETE",
            "/api/me",
            Some(json!({"current_password":PASSWORD})),
            &[("authorization", &admin_bearer)]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn deleting_owner_revokes_pending_invite_and_admin_can_delete_self_with_successor() {
    let f = Fixture::new().await;
    let admin = f.admin().await;
    let a = admin["access_token"].as_str().unwrap();
    let owner = f.create(a, "gone_owner", "tracker").await;
    let login = f.login("gone_owner", PASSWORD).await;
    let token = invitation(&f, login["access_token"].as_str().unwrap()).await["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let auth = format!("Bearer {a}");
    let route = format!("/api/admin/users/{}", owner["id"].as_str().unwrap());
    assert_eq!(
        f.request("DELETE", &route, None, &[("authorization", &auth)])
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(preview(&f, &token).await, StatusCode::NOT_FOUND);
    f.create(a, "successor", "admin").await;
    assert_eq!(
        f.request(
            "DELETE",
            "/api/me",
            Some(json!({"current_password":PASSWORD})),
            &[("authorization", &auth)]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    f.login("successor", PASSWORD).await;
}

#[tokio::test]
async fn admin_registration_links_fix_role_without_pair_and_are_single_use() {
    let f = Fixture::new().await;
    let admin = f.admin().await;
    let auth = format!("Bearer {}", admin["access_token"].as_str().unwrap());
    assert_eq!(
        f.request(
            "POST",
            "/api/admin/invites",
            Some(json!({"role":"admin"})),
            &[("authorization", &auth)]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    for role in ["tracker", "partner"] {
        let (status, _, link) = f
            .request(
                "POST",
                "/api/admin/invites",
                Some(json!({"role":role})),
                &[("authorization", &auth)],
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        let token = link["token"].as_str().unwrap();
        let (_, _, info) = f
            .request(
                "POST",
                "/api/invites/preview",
                Some(json!({"token":token})),
                &[],
            )
            .await;
        assert_eq!(info["kind"], "account");
        assert_eq!(info["role"], role);
        let body = json!({"token":token,"username":format!("invited_{role}"),"display_name":"New account","password":PASSWORD,"client":"native"});
        let mut bad = body.clone();
        bad["role"] = json!("admin");
        assert_eq!(
            f.request("POST", "/api/invites/register", Some(bad), &[])
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
        let (one, two) = tokio::join!(
            f.request("POST", "/api/invites/register", Some(body.clone()), &[]),
            f.request("POST", "/api/invites/register", Some(body), &[])
        );
        let mut statuses = [one.0.as_u16(), two.0.as_u16()];
        statuses.sort();
        assert_eq!(statuses, [201, 404]);
        let result = if one.0 == StatusCode::CREATED {
            one.2
        } else {
            two.2
        };
        assert_eq!(result["user"]["role"], role);
        assert!(result["pair"].is_null());
        assert_eq!(preview(&f, token).await, StatusCode::NOT_FOUND);
        let login = f.login(&format!("invited_{role}"), PASSWORD).await;
        let bearer = format!("Bearer {}", login["access_token"].as_str().unwrap());
        assert_eq!(
            f.request(
                "POST",
                "/api/admin/invites",
                Some(json!({"role":"tracker"})),
                &[("authorization", &bearer)]
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pairs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn admin_invites_rotate_revoke_expire_and_cannot_be_used_for_pairing() {
    let f = Fixture::new().await;
    let admin = f.admin().await;
    let auth = format!("Bearer {}", admin["access_token"].as_str().unwrap());
    let (_, _, old) = f
        .request(
            "POST",
            "/api/admin/invites",
            Some(json!({"role":"tracker"})),
            &[("authorization", &auth)],
        )
        .await;
    let (_, _, new) = f
        .request(
            "POST",
            "/api/admin/invites",
            Some(json!({"role":"tracker"})),
            &[("authorization", &auth)],
        )
        .await;
    assert_eq!(
        preview(&f, old["token"].as_str().unwrap()).await,
        StatusCode::NOT_FOUND
    );
    f.create(
        admin["access_token"].as_str().unwrap(),
        "existing_partner",
        "partner",
    )
    .await;
    let login = f.login("existing_partner", PASSWORD).await;
    let bearer = format!("Bearer {}", login["access_token"].as_str().unwrap());
    assert_eq!(
        f.request(
            "POST",
            "/api/me/invite/accept",
            Some(json!({"token":new["token"]})),
            &[("authorization", &bearer)]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        preview(&f, new["token"].as_str().unwrap()).await,
        StatusCode::OK
    );
    assert_eq!(
        f.request(
            "DELETE",
            "/api/admin/invites/tracker",
            None,
            &[("authorization", &auth)]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        preview(&f, new["token"].as_str().unwrap()).await,
        StatusCode::NOT_FOUND
    );
    let (_, _, expired) = f
        .request(
            "POST",
            "/api/admin/invites",
            Some(json!({"role":"tracker"})),
            &[("authorization", &auth)],
        )
        .await;
    sqlx::query("UPDATE account_invites SET expires_at=0")
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        preview(&f, expired["token"].as_str().unwrap()).await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn calendar_day_entries_follow_current_pair_and_only_owner_can_write() {
    let f = Fixture::new().await;
    let admin = f.admin().await;
    let admin_token = admin["access_token"].as_str().unwrap();
    let tracker = f.create(admin_token, "calendar_owner", "tracker").await;
    let partner = f.create(admin_token, "calendar_partner", "partner").await;
    let owner_login = f.login("calendar_owner", PASSWORD).await;
    let partner_login = f.login("calendar_partner", PASSWORD).await;
    let owner_bearer = format!("Bearer {}", owner_login["access_token"].as_str().unwrap());
    let partner_bearer = format!("Bearer {}", partner_login["access_token"].as_str().unwrap());
    let admin_bearer = format!("Bearer {admin_token}");
    let owner_headers = [("authorization", owner_bearer.as_str())];
    let partner_headers = [("authorization", partner_bearer.as_str())];
    let path = "/api/diary/2025-11-01";
    let entry = json!({"symptoms":["acne","cramps"],"events":["orgasm","lubricant"],"mood":"calm","flow":"","pain":1,"notes":"Fixture"});
    assert_eq!(
        f.request("PUT", path, Some(entry.clone()), &owner_headers)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.request("GET", "/api/calendar", None, &partner_headers)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, _, _) = f
        .request(
            "POST",
            "/api/admin/pairs",
            Some(json!({"tracker_id":tracker["id"],"partner_id":partner["id"]})),
            &[("authorization", &admin_bearer)],
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, headers, shared) = f
        .request("GET", "/api/calendar", None, &partner_headers)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(shared["access"], "partner");
    assert!(shared["settings"].is_null());
    assert_eq!(shared["entries"][0]["notes"], entry["notes"]);
    assert_eq!(shared["entries"][0]["events"], entry["events"]);
    assert_eq!(
        f.request("PUT", path, Some(entry), &partner_headers)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request("DELETE", path, None, &partner_headers).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request("DELETE", "/api/me/pair", None, &owner_headers)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.request("GET", "/api/calendar", None, &partner_headers)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}
