use super::*;

pub(super) async fn insert_user(
    transaction: &mut Transaction<'_, Sqlite>,
    user: &User,
    hash: &str,
) -> Result<(), AuthError> {
    sqlx::query("INSERT INTO users(id,username,display_name,role,password_hash,created_at) VALUES(?,?,?,?,?,?)")
        .bind(&user.id).bind(&user.username).bind(&user.display_name).bind(&user.role).bind(hash).bind(now())
        .execute(&mut **transaction).await.map_err(conflict_or_internal)?;
    Ok(())
}

pub(super) async fn user(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<UserRecord, AuthError> {
    sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?
        .ok_or_else(AuthError::invalid)
}
pub(super) async fn revoke_sessions(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<(), AuthError> {
    sqlx::query("UPDATE sessions SET revoked_at=? WHERE user_id=?")
        .bind(now())
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(())
}
pub(super) async fn delete_pairs(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<(), AuthError> {
    sqlx::query("DELETE FROM pairs WHERE tracker_id=? OR partner_id=?")
        .bind(id)
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(())
}
pub(super) async fn insert_pair(
    tx: &mut Transaction<'_, Sqlite>,
    pair: &Pair,
) -> Result<(), AuthError> {
    sqlx::query("INSERT INTO pairs(id,tracker_id,partner_id,created_at) VALUES(?,?,?,?)")
        .bind(&pair.id)
        .bind(&pair.tracker.id)
        .bind(&pair.partner.id)
        .bind(&pair.created_at)
        .execute(&mut **tx)
        .await
        .map_err(conflict_or_internal)?;
    Ok(())
}

pub(super) async fn admin_count(
    tx: &mut Transaction<'_, Sqlite>,
    active_only: bool,
) -> Result<i64, AuthError> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role='admin' AND (?=0 OR is_active=1)")
        .bind(active_only)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())
}
pub(super) async fn password_hash(pool: &SqlitePool, id: &str) -> Result<String, AuthError> {
    sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|_| AuthError::internal())
}
pub(super) async fn delete_user(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<(), AuthError> {
    sqlx::query("DELETE FROM users WHERE id=?")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(())
}
pub(super) async fn set_active(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    active: bool,
) -> Result<(), AuthError> {
    sqlx::query("UPDATE users SET is_active=? WHERE id=?")
        .bind(active)
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(())
}
pub(super) async fn set_password(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    hash: &str,
    expected: Option<&str>,
) -> Result<bool, AuthError> {
    let rows = sqlx::query(
        "UPDATE users SET password_hash=? WHERE id=? AND (? IS NULL OR password_hash=?)",
    )
    .bind(hash)
    .bind(id)
    .bind(expected)
    .bind(expected)
    .execute(&mut **tx)
    .await
    .map_err(|_| AuthError::internal())?
    .rows_affected();
    Ok(rows == 1)
}
pub(super) async fn users(pool: &SqlitePool) -> Result<Vec<UserRecord>, AuthError> {
    sqlx::query_as::<_, UserRecord>("SELECT * FROM users ORDER BY created_at,id")
        .fetch_all(pool)
        .await
        .map_err(|_| AuthError::internal())
}
