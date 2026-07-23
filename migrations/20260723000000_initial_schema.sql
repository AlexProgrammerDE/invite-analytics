CREATE TABLE IF NOT EXISTS guilds (
    id TEXT PRIMARY KEY,
    default_channel_id TEXT,
    log_channel_id TEXT,
    max_links INTEGER NOT NULL DEFAULT 130,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS tracked_invites (
    id SERIAL PRIMARY KEY,
    guild_id TEXT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    invite_code TEXT NOT NULL UNIQUE,
    channel_id TEXT NOT NULL,
    primary_source TEXT NOT NULL,
    secondary_source TEXT NOT NULL,
    created_by TEXT NOT NULL,
    uses INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS invite_roles (
    id SERIAL PRIMARY KEY,
    tracked_invite_id INTEGER NOT NULL REFERENCES tracked_invites(id) ON DELETE CASCADE,
    role_id TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS invite_uses (
    id SERIAL PRIMARY KEY,
    tracked_invite_id INTEGER NOT NULL REFERENCES tracked_invites(id) ON DELETE CASCADE,
    guild_id TEXT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    joined_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id SERIAL PRIMARY KEY,
    guild_id TEXT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    performed_by TEXT NOT NULL,
    details JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS tracked_invites_guild_id_idx
    ON tracked_invites(guild_id);

CREATE INDEX IF NOT EXISTS invite_roles_tracked_invite_id_idx
    ON invite_roles(tracked_invite_id);

CREATE INDEX IF NOT EXISTS invite_uses_guild_id_joined_at_idx
    ON invite_uses(guild_id, joined_at DESC);

CREATE INDEX IF NOT EXISTS invite_uses_tracked_invite_id_joined_at_idx
    ON invite_uses(tracked_invite_id, joined_at DESC);

CREATE INDEX IF NOT EXISTS audit_logs_guild_id_created_at_idx
    ON audit_logs(guild_id, created_at DESC);
