ALTER TABLE guilds
    ADD COLUMN track_vanity BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN vanity_primary_source TEXT NOT NULL DEFAULT 'Vanity URL',
    ADD COLUMN vanity_secondary_source TEXT NOT NULL DEFAULT 'Server vanity',
    ADD COLUMN last_synced_at TIMESTAMP;

ALTER TABLE tracked_invites
    RENAME COLUMN created_by TO tracked_by;

ALTER TABLE tracked_invites
    RENAME COLUMN uses TO discord_uses;

ALTER TABLE tracked_invites
    RENAME COLUMN created_at TO tracked_at;

ALTER TABLE tracked_invites
    ALTER COLUMN discord_uses TYPE BIGINT USING discord_uses::BIGINT,
    ADD COLUMN channel_type INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN discord_inviter_id TEXT,
    ADD COLUMN discord_created_at TIMESTAMP,
    ADD COLUMN max_uses INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN max_age INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN temporary BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN expires_at TIMESTAMP,
    ADD COLUMN invite_type INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN flags BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN target_type INTEGER,
    ADD COLUMN target_user_id TEXT,
    ADD COLUMN target_application_id TEXT,
    ADD COLUMN scheduled_event_id TEXT,
    ADD COLUMN targeted_user_count INTEGER,
    ADD COLUMN is_vanity BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN tracking_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN discord_active BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN status TEXT NOT NULL DEFAULT 'active',
    ADD COLUMN deleted_at TIMESTAMP,
    ADD COLUMN last_synced_at TIMESTAMP;

ALTER TABLE invite_roles
    ADD COLUMN assignment_mode TEXT NOT NULL DEFAULT 'managed';

DELETE FROM invite_roles AS duplicate
USING invite_roles AS original
WHERE duplicate.id > original.id
  AND duplicate.tracked_invite_id = original.tracked_invite_id
  AND duplicate.role_id = original.role_id
  AND duplicate.assignment_mode = original.assignment_mode;

CREATE UNIQUE INDEX invite_roles_assignment_idx
    ON invite_roles(tracked_invite_id, role_id, assignment_mode);

ALTER TABLE invite_uses
    RENAME TO join_events;

ALTER TABLE join_events
    RENAME COLUMN joined_at TO observed_at;

ALTER TABLE join_events
    ADD COLUMN member_joined_at TIMESTAMP,
    ADD COLUMN account_created_at TIMESTAMP,
    ADD COLUMN left_at TIMESTAMP,
    ADD COLUMN screening_completed_at TIMESTAMP,
    ADD COLUMN invite_code_snapshot TEXT,
    ADD COLUMN primary_source_snapshot TEXT,
    ADD COLUMN secondary_source_snapshot TEXT,
    ADD COLUMN attribution_status TEXT NOT NULL DEFAULT 'attributed',
    ADD COLUMN attribution_reason TEXT,
    ADD COLUMN attribution_confidence TEXT NOT NULL DEFAULT 'inferred',
    ADD COLUMN is_bot BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN is_system BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN member_flags BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN pending BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE join_events
SET member_joined_at = observed_at;

UPDATE join_events AS join_event
SET invite_code_snapshot = tracked_invites.invite_code,
    primary_source_snapshot = tracked_invites.primary_source,
    secondary_source_snapshot = tracked_invites.secondary_source
FROM tracked_invites
WHERE tracked_invites.id = join_event.tracked_invite_id;

ALTER TABLE join_events
    ALTER COLUMN member_joined_at SET NOT NULL,
    ALTER COLUMN tracked_invite_id DROP NOT NULL,
    DROP CONSTRAINT invite_uses_tracked_invite_id_fkey;

ALTER TABLE join_events
    ADD CONSTRAINT join_events_tracked_invite_id_fkey
        FOREIGN KEY (tracked_invite_id)
        REFERENCES tracked_invites(id)
        ON DELETE SET NULL;

DROP INDEX invite_uses_guild_id_joined_at_idx;
DROP INDEX invite_uses_tracked_invite_id_joined_at_idx;

CREATE UNIQUE INDEX join_events_membership_session_idx
    ON join_events(guild_id, user_id, member_joined_at);

CREATE INDEX join_events_guild_id_observed_at_idx
    ON join_events(guild_id, observed_at DESC);

CREATE INDEX join_events_tracked_invite_id_observed_at_idx
    ON join_events(tracked_invite_id, observed_at DESC);

CREATE INDEX join_events_guild_id_attribution_status_idx
    ON join_events(guild_id, attribution_status);

CREATE INDEX tracked_invites_guild_active_idx
    ON tracked_invites(guild_id, tracking_enabled, discord_active);
