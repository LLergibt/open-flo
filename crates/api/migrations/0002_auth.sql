CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    display_name TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'tracker', 'partner')),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    password_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    revoked_at INTEGER,
    refresh_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX sessions_user_idx ON sessions(user_id);
CREATE INDEX sessions_expiry_idx ON sessions(expires_at);

CREATE TABLE refresh_tokens (
    token_hash TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL,
    used_at INTEGER
);
CREATE INDEX refresh_session_idx ON refresh_tokens(session_id);

CREATE TABLE pairs (
    id TEXT PRIMARY KEY NOT NULL,
    tracker_id TEXT NOT NULL UNIQUE REFERENCES users(id),
    partner_id TEXT NOT NULL UNIQUE REFERENCES users(id),
    created_at TEXT NOT NULL,
    CHECK (tracker_id != partner_id)
);

CREATE TRIGGER pairs_roles_before_insert
BEFORE INSERT ON pairs
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM users WHERE id = NEW.tracker_id AND role = 'tracker' AND is_active = 1
    ) OR NOT EXISTS (
        SELECT 1 FROM users WHERE id = NEW.partner_id AND role = 'partner' AND is_active = 1
    ) THEN RAISE(ABORT, 'invalid pair members') END;
END;
