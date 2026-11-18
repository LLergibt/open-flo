use super::*;

/// Call inside the same write transaction that removes or disables the account.
pub(super) async fn protect_last_admin(
    tx: &mut Transaction<'_, Sqlite>,
    user: &UserRecord,
) -> Result<(), AuthError> {
    if user.role == "admin" && user.is_active {
        let count = store::admin_count(tx, true).await?;
        if count <= 1 {
            return Err(AuthError::conflict());
        }
    }
    Ok(())
}
/// Hashing runs before taking the SQLite write lock. Mutations must recheck this proof inside their transaction.
pub(super) async fn verify_current_password(
    state: &AuthState,
    id: &str,
    password: String,
) -> Result<String, AuthError> {
    if password.len() > 512 {
        return Err(AuthError::invalid());
    }
    let hash = store::password_hash(&state.pool, id).await?;
    if !verify_password(password, hash.clone(), state.hashing.clone()).await? {
        return Err(AuthError::credentials());
    }
    Ok(hash)
}
pub(super) async fn remove(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<(), AuthError> {
    let user = store::user(tx, id).await?;
    protect_last_admin(tx, &user).await?;
    store::delete_pairs(tx, id).await?;
    store::delete_user(tx, id).await?;
    Ok(())
}
pub(super) async fn create_pair(
    tx: &mut Transaction<'_, Sqlite>,
    tracker_id: &str,
    partner_id: &str,
) -> Result<Pair, AuthError> {
    let tracker = store::user(tx, tracker_id).await?;
    let partner = store::user(tx, partner_id).await?;
    if tracker.role != "tracker"
        || partner.role != "partner"
        || !tracker.is_active
        || !partner.is_active
    {
        return Err(AuthError::invalid());
    }
    let pair = Pair {
        id: Uuid::new_v4().to_string(),
        tracker: tracker.public(),
        partner: partner.public(),
        created_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|_| AuthError::internal())?,
    };
    store::insert_pair(tx, &pair).await?;
    Ok(pair)
}

pub(super) async fn create_account(
    state: &AuthState,
    actor: &Authenticated,
    input: CreateUserInput,
) -> Result<User, AuthError> {
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
    let mut transaction = actor_transaction(state, actor, true).await?;
    insert_user(&mut transaction, &user, &hash).await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    Ok(user)
}

pub(super) async fn update_account(
    state: &AuthState,
    actor: &Authenticated,
    id: &str,
    input: PatchUserInput,
) -> Result<User, AuthError> {
    valid_id(id)?;
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
    let mut transaction = actor_transaction(state, actor, true).await?;
    let record = store::user(&mut transaction, id).await?;
    if input.is_active == Some(false) {
        if id == actor.user.id {
            return Err(AuthError::conflict());
        }
        protect_last_admin(&mut transaction, &record).await?;
    }

    if let Some(active) = input.is_active {
        store::set_active(&mut transaction, id, active).await?;
    }
    if let Some(hash) = &hash {
        store::set_password(&mut transaction, id, hash, None).await?;
    }

    if input.is_active == Some(false) || hash.is_some() {
        store::revoke_sessions(&mut transaction, id).await?;
    }

    if input.is_active == Some(false) {
        store::delete_pairs(&mut transaction, id).await?;
    }

    transaction
        .commit()
        .await
        .map_err(|_| AuthError::internal())?;
    let mut user = record.public();
    if let Some(active) = input.is_active {
        user.is_active = active;
    }
    Ok(user)
}

pub(super) async fn change_password(
    state: &AuthState,
    actor: &Authenticated,
    current_password: String,
    new_password: String,
) -> Result<(), AuthError> {
    if !password_valid(&new_password) || current_password.len() > 512 {
        return Err(AuthError::invalid());
    }
    let old = verify_current_password(state, &actor.user.id, current_password).await?;
    let new = hash_password(new_password, state.hashing.clone()).await?;
    let mut tx = actor_transaction(state, actor, false).await?;
    if !store::set_password(&mut tx, &actor.user.id, &new, Some(&old)).await? {
        return Err(AuthError::credentials());
    }
    store::revoke_sessions(&mut tx, &actor.user.id).await?;
    tx.commit().await.map_err(|_| AuthError::internal())?;
    Ok(())
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
    let count = store::admin_count(&mut transaction, false)
        .await
        .map_err(|_| "administrator lookup failed")?;
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
