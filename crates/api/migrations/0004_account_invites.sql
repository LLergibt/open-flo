CREATE TABLE account_invites (
 token_hash TEXT PRIMARY KEY NOT NULL,
 admin_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 role TEXT NOT NULL CHECK(role IN ('tracker','partner')),
 expires_at INTEGER NOT NULL,
 UNIQUE(admin_id,role)
);
CREATE TRIGGER admin_change_revokes_account_invites AFTER UPDATE OF password_hash,is_active ON users
BEGIN DELETE FROM account_invites WHERE admin_id=NEW.id; END;
