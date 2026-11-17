CREATE TABLE partner_invites (
    token_hash TEXT PRIMARY KEY NOT NULL,
    tracker_id TEXT NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL
);
CREATE TRIGGER pair_consumes_pending_invitation AFTER INSERT ON pairs
BEGIN
    DELETE FROM partner_invites WHERE tracker_id=NEW.tracker_id;
END;
CREATE TRIGGER account_change_revokes_invitation AFTER UPDATE OF password_hash,is_active ON users
BEGIN
    DELETE FROM partner_invites WHERE tracker_id=NEW.id;
END;
