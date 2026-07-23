use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};

use crate::models::{
    AnalyticsCounts, DailyCount, ExportInvite, ExportJoin, GuildConfig, InviteSync, InviteUse,
    NewJoinEvent, NewTrackedInvite, RetentionCount, RoleAssignmentMode, SourceCount, TrackedInvite,
};

#[derive(Clone)]
pub struct Repository {
    pool: PgPool,
}

impl Repository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn ping(&self) -> sqlx::Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn ensure_guild(&self, guild_id: &str) -> sqlx::Result<()> {
        sqlx::query("INSERT INTO guilds (id) VALUES ($1) ON CONFLICT (id) DO NOTHING")
            .bind(guild_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn guild_ids(&self) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar("SELECT id FROM guilds ORDER BY id")
            .fetch_all(&self.pool)
            .await
    }

    pub async fn guild_config(&self, guild_id: &str) -> sqlx::Result<Option<GuildConfig>> {
        sqlx::query_as::<_, GuildConfig>(
            "SELECT id, default_channel_id, log_channel_id, max_links, track_vanity, \
             vanity_primary_source, vanity_secondary_source, last_synced_at, \
             created_at, updated_at FROM guilds WHERE id = $1",
        )
        .bind(guild_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn set_log_channel(&self, guild_id: &str, channel_id: &str) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE guilds SET log_channel_id = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(guild_id)
        .bind(channel_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_default_channel(&self, guild_id: &str, channel_id: &str) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE guilds SET default_channel_id = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(guild_id)
        .bind(channel_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_max_links(&self, guild_id: &str, max_links: i32) -> sqlx::Result<()> {
        sqlx::query("UPDATE guilds SET max_links = $2, updated_at = NOW() WHERE id = $1")
            .bind(guild_id)
            .bind(max_links)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_vanity_tracking(
        &self,
        guild_id: &str,
        enabled: bool,
        primary_source: &str,
        secondary_source: &str,
    ) -> sqlx::Result<()> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE guilds SET track_vanity = $2, vanity_primary_source = $3, \
             vanity_secondary_source = $4, updated_at = NOW() WHERE id = $1",
        )
        .bind(guild_id)
        .bind(enabled)
        .bind(primary_source)
        .bind(secondary_source)
        .execute(&mut *transaction)
        .await?;
        if !enabled {
            sqlx::query(
                "UPDATE tracked_invites SET tracking_enabled = FALSE, \
                 status = 'untracked', deleted_at = COALESCE(deleted_at, NOW()) \
                 WHERE guild_id = $1 AND is_vanity AND tracking_enabled",
            )
            .bind(guild_id)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await
    }

    pub async fn mark_guild_synced(&self, guild_id: &str) -> sqlx::Result<()> {
        sqlx::query("UPDATE guilds SET last_synced_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(guild_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn log_channel_id(&self, guild_id: &str) -> sqlx::Result<Option<String>> {
        let value = sqlx::query_scalar::<_, Option<String>>(
            "SELECT log_channel_id FROM guilds WHERE id = $1",
        )
        .bind(guild_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(value.flatten())
    }

    pub async fn count_invites(&self, guild_id: &str) -> sqlx::Result<i64> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM tracked_invites \
             WHERE guild_id = $1 AND tracking_enabled",
        )
        .bind(guild_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_invites_page(
        &self,
        guild_id: &str,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<TrackedInvite>> {
        sqlx::query_as::<_, TrackedInvite>(
            "SELECT id, guild_id, invite_code, channel_id, channel_type, primary_source, \
             secondary_source, tracked_by, discord_inviter_id, discord_created_at, \
             discord_uses, max_uses, max_age, temporary, \
             expires_at, invite_type, flags, target_type, target_user_id, \
             target_application_id, scheduled_event_id, targeted_user_count, is_vanity, \
             tracking_enabled, discord_active, status, deleted_at, last_synced_at, tracked_at \
             FROM tracked_invites WHERE guild_id = $1 AND tracking_enabled \
             ORDER BY tracked_at, id LIMIT $2 OFFSET $3",
        )
        .bind(guild_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_invite(
        &self,
        guild_id: &str,
        code: &str,
    ) -> sqlx::Result<Option<TrackedInvite>> {
        sqlx::query_as::<_, TrackedInvite>(
            "SELECT id, guild_id, invite_code, channel_id, channel_type, primary_source, \
             secondary_source, tracked_by, discord_inviter_id, discord_created_at, \
             discord_uses, max_uses, max_age, temporary, \
             expires_at, invite_type, flags, target_type, target_user_id, \
             target_application_id, scheduled_event_id, targeted_user_count, is_vanity, \
             tracking_enabled, discord_active, status, deleted_at, last_synced_at, tracked_at \
             FROM tracked_invites WHERE guild_id = $1 AND invite_code = $2",
        )
        .bind(guild_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn invite_exists(&self, code: &str) -> sqlx::Result<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM tracked_invites \
             WHERE invite_code = $1 AND tracking_enabled)",
        )
        .bind(code)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn insert_invite(&self, invite: &NewTrackedInvite) -> sqlx::Result<TrackedInvite> {
        let mut transaction = self.pool.begin().await?;
        let inserted = sqlx::query_as::<_, TrackedInvite>(
            "INSERT INTO tracked_invites \
             (guild_id, invite_code, channel_id, channel_type, primary_source, secondary_source, \
              tracked_by, discord_inviter_id, discord_created_at, discord_uses, max_uses, max_age, \
              temporary, expires_at, invite_type, flags, target_type, target_user_id, \
              target_application_id, scheduled_event_id, targeted_user_count, is_vanity, \
              tracked_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
                     $15, $16, $17, $18, $19, $20, $21, $22, COALESCE($23, NOW())) \
             ON CONFLICT (invite_code) DO UPDATE SET \
                 channel_id = EXCLUDED.channel_id, \
                 channel_type = EXCLUDED.channel_type, \
                 primary_source = EXCLUDED.primary_source, \
                 secondary_source = EXCLUDED.secondary_source, \
                 tracked_by = EXCLUDED.tracked_by, \
                 discord_inviter_id = EXCLUDED.discord_inviter_id, \
                 discord_created_at = EXCLUDED.discord_created_at, \
                 discord_uses = EXCLUDED.discord_uses, \
                 max_uses = EXCLUDED.max_uses, \
                 max_age = EXCLUDED.max_age, \
                 temporary = EXCLUDED.temporary, \
                 expires_at = EXCLUDED.expires_at, \
                 invite_type = EXCLUDED.invite_type, \
                 flags = EXCLUDED.flags, \
                 target_type = EXCLUDED.target_type, \
                 target_user_id = EXCLUDED.target_user_id, \
                 target_application_id = EXCLUDED.target_application_id, \
                 scheduled_event_id = EXCLUDED.scheduled_event_id, \
                 targeted_user_count = EXCLUDED.targeted_user_count, \
                 is_vanity = EXCLUDED.is_vanity, \
                 tracking_enabled = TRUE, discord_active = TRUE, status = 'active', \
                 deleted_at = NULL, last_synced_at = NOW() \
             WHERE tracked_invites.guild_id = EXCLUDED.guild_id \
             RETURNING id, guild_id, invite_code, channel_id, channel_type, primary_source, \
             secondary_source, tracked_by, discord_inviter_id, discord_created_at, \
             discord_uses, max_uses, max_age, temporary, \
             expires_at, invite_type, flags, target_type, target_user_id, \
             target_application_id, scheduled_event_id, targeted_user_count, is_vanity, \
             tracking_enabled, discord_active, status, deleted_at, last_synced_at, tracked_at",
        )
        .bind(&invite.guild_id)
        .bind(&invite.invite_code)
        .bind(&invite.channel_id)
        .bind(invite.channel_type)
        .bind(&invite.primary_source)
        .bind(&invite.secondary_source)
        .bind(&invite.tracked_by)
        .bind(&invite.discord_inviter_id)
        .bind(invite.discord_created_at)
        .bind(invite.discord_uses)
        .bind(invite.max_uses)
        .bind(invite.max_age)
        .bind(invite.temporary)
        .bind(invite.expires_at)
        .bind(invite.invite_type)
        .bind(invite.flags)
        .bind(invite.target_type)
        .bind(&invite.target_user_id)
        .bind(&invite.target_application_id)
        .bind(&invite.scheduled_event_id)
        .bind(invite.targeted_user_count)
        .bind(invite.is_vanity)
        .bind(invite.tracked_at)
        .fetch_one(&mut *transaction)
        .await?;

        replace_roles(
            &mut transaction,
            inserted.id,
            &invite.role_ids,
            invite.role_assignment_mode,
        )
        .await?;
        transaction.commit().await?;
        Ok(inserted)
    }

    pub async fn sync_invite(&self, guild_id: &str, invite: &InviteSync) -> sqlx::Result<bool> {
        let result = sqlx::query(
            "UPDATE tracked_invites SET channel_id = $3, channel_type = $4, \
             discord_inviter_id = $5, discord_created_at = $6, \
             discord_uses = $7, max_uses = $8, max_age = $9, temporary = $10, \
             expires_at = $11, invite_type = $12, flags = $13, target_type = $14, \
             target_user_id = $15, target_application_id = $16, scheduled_event_id = $17, \
             discord_active = TRUE, \
             status = CASE WHEN tracking_enabled THEN 'active' ELSE status END, \
             deleted_at = CASE WHEN tracking_enabled THEN NULL ELSE deleted_at END, \
             last_synced_at = NOW() \
             WHERE guild_id = $1 AND invite_code = $2",
        )
        .bind(guild_id)
        .bind(&invite.invite_code)
        .bind(&invite.channel_id)
        .bind(invite.channel_type)
        .bind(&invite.discord_inviter_id)
        .bind(invite.discord_created_at)
        .bind(invite.discord_uses)
        .bind(invite.max_uses)
        .bind(invite.max_age)
        .bind(invite.temporary)
        .bind(invite.expires_at)
        .bind(invite.invite_type)
        .bind(invite.flags)
        .bind(invite.target_type)
        .bind(&invite.target_user_id)
        .bind(&invite.target_application_id)
        .bind(&invite.scheduled_event_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() > 0 {
            let mut transaction = self.pool.begin().await?;
            let invite_id = sqlx::query_scalar(
                "SELECT id FROM tracked_invites WHERE guild_id = $1 AND invite_code = $2",
            )
            .bind(guild_id)
            .bind(&invite.invite_code)
            .fetch_one(&mut *transaction)
            .await?;
            replace_roles_for_mode(
                &mut transaction,
                invite_id,
                &invite.role_ids,
                RoleAssignmentMode::Native,
            )
            .await?;
            transaction.commit().await?;
        }

        Ok(result.rows_affected() > 0)
    }

    pub async fn upsert_vanity_invite(
        &self,
        guild_id: &str,
        code: &str,
        uses: i64,
        primary_source: &str,
        secondary_source: &str,
    ) -> sqlx::Result<TrackedInvite> {
        sqlx::query(
            "UPDATE tracked_invites SET tracking_enabled = FALSE, discord_active = FALSE, \
             status = 'replaced', deleted_at = NOW() \
             WHERE guild_id = $1 AND is_vanity AND invite_code <> $2 AND tracking_enabled",
        )
        .bind(guild_id)
        .bind(code)
        .execute(&self.pool)
        .await?;

        self.insert_invite(&NewTrackedInvite {
            guild_id: guild_id.to_owned(),
            invite_code: code.to_owned(),
            channel_id: "vanity".to_owned(),
            channel_type: 0,
            primary_source: primary_source.to_owned(),
            secondary_source: secondary_source.to_owned(),
            tracked_by: "system".to_owned(),
            discord_inviter_id: None,
            discord_created_at: None,
            discord_uses: uses,
            max_uses: 0,
            max_age: 0,
            temporary: false,
            expires_at: None,
            invite_type: 0,
            flags: 0,
            target_type: None,
            target_user_id: None,
            target_application_id: None,
            scheduled_event_id: None,
            targeted_user_count: None,
            is_vanity: true,
            tracked_at: None,
            role_ids: Vec::new(),
            role_assignment_mode: RoleAssignmentMode::Native,
        })
        .await
    }

    pub async fn mark_missing_invites(
        &self,
        guild_id: &str,
        active_codes: &[String],
    ) -> sqlx::Result<u64> {
        let result = sqlx::query(
            "UPDATE tracked_invites SET discord_active = FALSE, tracking_enabled = FALSE, \
             status = CASE \
                 WHEN max_uses > 0 AND discord_uses >= max_uses THEN 'exhausted' \
                 WHEN expires_at IS NOT NULL AND expires_at <= NOW() THEN 'expired' \
                 ELSE 'deleted' \
             END, deleted_at = COALESCE(deleted_at, NOW()), last_synced_at = NOW() \
             WHERE guild_id = $1 AND NOT is_vanity AND discord_active \
               AND NOT (invite_code = ANY($2::TEXT[]))",
        )
        .bind(guild_id)
        .bind(active_codes)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_invite_deleted(&self, guild_id: &str, code: &str) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE tracked_invites SET tracking_enabled = FALSE, discord_active = FALSE, \
             status = CASE \
                 WHEN max_uses > 0 AND discord_uses >= max_uses THEN 'exhausted' \
                 ELSE 'deleted' \
             END, deleted_at = COALESCE(deleted_at, NOW()), last_synced_at = NOW() \
             WHERE guild_id = $1 AND invite_code = $2",
        )
        .bind(guild_id)
        .bind(code)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn stop_tracking(&self, invite_id: i32, discord_revoked: bool) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE tracked_invites SET tracking_enabled = FALSE, \
             discord_active = CASE WHEN $2 THEN FALSE ELSE discord_active END, \
             status = CASE WHEN $2 THEN 'revoked' ELSE 'untracked' END, \
             deleted_at = NOW() WHERE id = $1",
        )
        .bind(invite_id)
        .bind(discord_revoked)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_invite_uses(
        &self,
        guild_id: &str,
        code: &str,
        uses: i64,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE tracked_invites SET discord_uses = GREATEST(discord_uses, $3), \
             status = CASE \
                 WHEN max_uses > 0 AND GREATEST(discord_uses, $3) >= max_uses \
                     THEN 'exhausted' \
                 ELSE status \
             END, \
             last_synced_at = NOW() WHERE guild_id = $1 AND invite_code = $2",
        )
        .bind(guild_id)
        .bind(code)
        .bind(uses)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_invite_sources(
        &self,
        invite_id: i32,
        primary_source: &str,
        secondary_source: &str,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE tracked_invites SET primary_source = $2, secondary_source = $3 \
             WHERE id = $1",
        )
        .bind(invite_id)
        .bind(primary_source)
        .bind(secondary_source)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_targeted_user_count(
        &self,
        invite_id: i32,
        targeted_user_count: i32,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE tracked_invites SET targeted_user_count = $2, last_synced_at = NOW() \
             WHERE id = $1",
        )
        .bind(invite_id)
        .bind(targeted_user_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn replace_managed_roles(
        &self,
        invite_id: i32,
        role_ids: &[String],
    ) -> sqlx::Result<()> {
        let mut transaction = self.pool.begin().await?;
        replace_roles_for_mode(
            &mut transaction,
            invite_id,
            role_ids,
            RoleAssignmentMode::Managed,
        )
        .await?;
        transaction.commit().await
    }

    pub async fn native_invite_role_ids(&self, invite_id: i32) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar(
            "SELECT role_id FROM invite_roles \
             WHERE tracked_invite_id = $1 AND assignment_mode = 'native' ORDER BY id",
        )
        .bind(invite_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn managed_invite_role_ids(&self, invite_id: i32) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar(
            "SELECT role_id FROM invite_roles \
             WHERE tracked_invite_id = $1 AND assignment_mode = 'managed' ORDER BY id",
        )
        .bind(invite_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn recent_joins(&self, invite_id: i32, limit: i64) -> sqlx::Result<Vec<InviteUse>> {
        sqlx::query_as::<_, InviteUse>(
            "SELECT user_id, member_joined_at, left_at FROM join_events \
             WHERE tracked_invite_id = $1 ORDER BY member_joined_at DESC LIMIT $2",
        )
        .bind(invite_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count_invite_joins(&self, invite_id: i32) -> sqlx::Result<i64> {
        sqlx::query_scalar("SELECT COUNT(*) FROM join_events WHERE tracked_invite_id = $1")
            .bind(invite_id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn record_join(&self, join: &NewJoinEvent) -> sqlx::Result<bool> {
        let result = sqlx::query(
            "INSERT INTO join_events \
             (tracked_invite_id, guild_id, user_id, member_joined_at, account_created_at, \
              invite_code_snapshot, primary_source_snapshot, secondary_source_snapshot, \
              attribution_status, attribution_reason, attribution_confidence, is_bot, is_system, \
              member_flags, pending) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) \
             ON CONFLICT (guild_id, user_id, member_joined_at) DO NOTHING",
        )
        .bind(join.tracked_invite_id)
        .bind(&join.guild_id)
        .bind(&join.user_id)
        .bind(join.member_joined_at)
        .bind(join.account_created_at)
        .bind(&join.invite_code_snapshot)
        .bind(&join.primary_source_snapshot)
        .bind(&join.secondary_source_snapshot)
        .bind(&join.attribution_status)
        .bind(&join.attribution_reason)
        .bind(&join.attribution_confidence)
        .bind(join.is_bot)
        .bind(join.is_system)
        .bind(join.member_flags)
        .bind(join.pending)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn record_member_left(&self, guild_id: &str, user_id: &str) -> sqlx::Result<bool> {
        let result = sqlx::query(
            "UPDATE join_events SET left_at = NOW() WHERE id = ( \
                 SELECT id FROM join_events \
                 WHERE guild_id = $1 AND user_id = $2 AND left_at IS NULL \
                 ORDER BY member_joined_at DESC LIMIT 1 \
             )",
        )
        .bind(guild_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn record_screening_completed(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> sqlx::Result<bool> {
        let result = sqlx::query(
            "UPDATE join_events SET pending = FALSE, screening_completed_at = NOW() \
             WHERE id = (SELECT id FROM join_events WHERE guild_id = $1 AND user_id = $2 \
             AND left_at IS NULL AND pending ORDER BY member_joined_at DESC LIMIT 1)",
        )
        .bind(guild_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn latest_tracked_invite_for_member(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> sqlx::Result<Option<i32>> {
        sqlx::query_scalar(
            "SELECT tracked_invite_id FROM join_events \
             WHERE guild_id = $1 AND user_id = $2 AND left_at IS NULL \
             ORDER BY member_joined_at DESC LIMIT 1",
        )
        .bind(guild_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map(Option::flatten)
    }

    pub async fn write_audit_log(
        &self,
        guild_id: &str,
        action: &str,
        performed_by: &str,
        details: Option<Value>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO audit_logs (guild_id, action, performed_by, details) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(guild_id)
        .bind(action)
        .bind(performed_by)
        .bind(details)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn analytics_counts(
        &self,
        guild_id: &str,
        since: Option<NaiveDateTime>,
    ) -> sqlx::Result<AnalyticsCounts> {
        sqlx::query_as::<_, AnalyticsCounts>(
            "SELECT \
                 COUNT(*) FILTER (WHERE NOT is_bot AND NOT is_system) AS total, \
                 COUNT(*) FILTER (WHERE NOT is_bot AND NOT is_system \
                     AND attribution_status = 'attributed') \
                     AS attributed, \
                 COUNT(*) FILTER (WHERE NOT is_bot AND NOT is_system \
                     AND attribution_status <> 'attributed') \
                     AS unattributed, \
                 COUNT(*) FILTER (WHERE is_bot OR is_system) AS bots \
             FROM join_events \
             WHERE guild_id = $1 AND ($2::TIMESTAMP IS NULL OR member_joined_at >= $2)",
        )
        .bind(guild_id)
        .bind(since)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn count_joins(
        &self,
        guild_id: &str,
        since: Option<NaiveDateTime>,
    ) -> sqlx::Result<i64> {
        Ok(self.analytics_counts(guild_id, since).await?.attributed)
    }

    pub async fn top_primary_sources(
        &self,
        guild_id: &str,
        since: Option<NaiveDateTime>,
        limit: i64,
    ) -> sqlx::Result<Vec<SourceCount>> {
        sqlx::query_as::<_, SourceCount>(
            "SELECT primary_source_snapshot AS source, COUNT(*) AS joins \
             FROM join_events \
             WHERE guild_id = $1 AND attribution_status = 'attributed' \
               AND NOT is_bot AND NOT is_system \
               AND primary_source_snapshot IS NOT NULL \
               AND ($2::TIMESTAMP IS NULL OR member_joined_at >= $2) \
             GROUP BY primary_source_snapshot \
             ORDER BY joins DESC, source ASC LIMIT $3",
        )
        .bind(guild_id)
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn top_secondary_sources(
        &self,
        guild_id: &str,
        since: Option<NaiveDateTime>,
        primary_source: Option<&str>,
        limit: i64,
    ) -> sqlx::Result<Vec<SourceCount>> {
        sqlx::query_as::<_, SourceCount>(
            "SELECT secondary_source_snapshot AS source, COUNT(*) AS joins \
             FROM join_events \
             WHERE guild_id = $1 AND attribution_status = 'attributed' \
               AND NOT is_bot AND NOT is_system \
               AND secondary_source_snapshot IS NOT NULL \
               AND ($2::TIMESTAMP IS NULL OR member_joined_at >= $2) \
               AND ($3::TEXT IS NULL OR primary_source_snapshot = $3) \
             GROUP BY secondary_source_snapshot \
             ORDER BY joins DESC, source ASC LIMIT $4",
        )
        .bind(guild_id)
        .bind(since)
        .bind(primary_source)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn joins_by_day(
        &self,
        guild_id: &str,
        since: Option<NaiveDateTime>,
    ) -> sqlx::Result<Vec<DailyCount>> {
        sqlx::query_as::<_, DailyCount>(
            "SELECT member_joined_at::DATE AS day, COUNT(*) AS joins \
             FROM join_events WHERE guild_id = $1 AND NOT is_bot AND NOT is_system \
               AND ($2::TIMESTAMP IS NULL OR member_joined_at >= $2) \
             GROUP BY member_joined_at::DATE ORDER BY day",
        )
        .bind(guild_id)
        .bind(since)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn retention_by_primary_source(
        &self,
        guild_id: &str,
        since: Option<NaiveDateTime>,
        limit: i64,
    ) -> sqlx::Result<Vec<RetentionCount>> {
        sqlx::query_as::<_, RetentionCount>(
            "SELECT COALESCE(primary_source_snapshot, 'Unattributed') AS source, \
                    COUNT(*) FILTER (WHERE NOT is_bot AND NOT is_system) AS joined, \
                    COUNT(*) FILTER (WHERE NOT is_bot AND NOT is_system \
                        AND left_at IS NULL) AS active \
             FROM join_events \
             WHERE guild_id = $1 AND ($2::TIMESTAMP IS NULL OR member_joined_at >= $2) \
             GROUP BY COALESCE(primary_source_snapshot, 'Unattributed') \
             ORDER BY joined DESC, source ASC LIMIT $3",
        )
        .bind(guild_id)
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn export_invites(&self, guild_id: &str) -> sqlx::Result<Vec<ExportInvite>> {
        sqlx::query_as::<_, ExportInvite>(
            "SELECT tracked_invites.invite_code, tracked_invites.channel_id, \
                    tracked_invites.channel_type, tracked_invites.primary_source, \
                    tracked_invites.secondary_source, tracked_invites.tracked_by, \
                    tracked_invites.discord_inviter_id, tracked_invites.discord_created_at, \
                    tracked_invites.discord_uses, \
                    (SELECT COUNT(*) FROM join_events \
                     WHERE join_events.tracked_invite_id = tracked_invites.id) AS attributed_joins, \
                    tracked_invites.max_uses, tracked_invites.max_age, \
                    tracked_invites.temporary, tracked_invites.expires_at, \
                    tracked_invites.invite_type, tracked_invites.flags, \
                    tracked_invites.target_type, tracked_invites.target_user_id, \
                    tracked_invites.target_application_id, tracked_invites.scheduled_event_id, \
                    tracked_invites.targeted_user_count, tracked_invites.is_vanity, \
                    tracked_invites.tracking_enabled, tracked_invites.discord_active, \
                    tracked_invites.status, tracked_invites.deleted_at, \
                    tracked_invites.last_synced_at, tracked_invites.tracked_at, \
                    COALESCE( \
                        ARRAY_AGG(invite_roles.role_id ORDER BY invite_roles.id) \
                            FILTER (WHERE invite_roles.assignment_mode = 'native'), \
                        ARRAY[]::TEXT[] \
                    ) AS native_role_ids, \
                    COALESCE( \
                        ARRAY_AGG(invite_roles.role_id ORDER BY invite_roles.id) \
                            FILTER (WHERE invite_roles.assignment_mode = 'managed'), \
                        ARRAY[]::TEXT[] \
                    ) AS managed_role_ids \
             FROM tracked_invites \
             LEFT JOIN invite_roles \
               ON invite_roles.tracked_invite_id = tracked_invites.id \
             WHERE tracked_invites.guild_id = $1 \
             GROUP BY tracked_invites.id \
             ORDER BY tracked_invites.tracked_at, tracked_invites.id",
        )
        .bind(guild_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn export_joins(&self, guild_id: &str) -> sqlx::Result<Vec<ExportJoin>> {
        sqlx::query_as::<_, ExportJoin>(
            "SELECT user_id, observed_at, member_joined_at, account_created_at, left_at, \
                    screening_completed_at, \
                    invite_code_snapshot AS invite_code, \
                    primary_source_snapshot AS primary_source, \
                    secondary_source_snapshot AS secondary_source, \
                    attribution_status, attribution_reason, attribution_confidence, \
                    is_bot, is_system, member_flags, pending \
             FROM join_events WHERE guild_id = $1 ORDER BY member_joined_at, id",
        )
        .bind(guild_id)
        .fetch_all(&self.pool)
        .await
    }
}

async fn replace_roles(
    transaction: &mut Transaction<'_, Postgres>,
    invite_id: i32,
    role_ids: &[String],
    assignment_mode: RoleAssignmentMode,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM invite_roles WHERE tracked_invite_id = $1")
        .bind(invite_id)
        .execute(&mut **transaction)
        .await?;

    insert_roles(transaction, invite_id, role_ids, assignment_mode).await
}

async fn replace_roles_for_mode(
    transaction: &mut Transaction<'_, Postgres>,
    invite_id: i32,
    role_ids: &[String],
    assignment_mode: RoleAssignmentMode,
) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM invite_roles \
         WHERE tracked_invite_id = $1 AND assignment_mode = $2",
    )
    .bind(invite_id)
    .bind(assignment_mode.as_str())
    .execute(&mut **transaction)
    .await?;

    insert_roles(transaction, invite_id, role_ids, assignment_mode).await
}

async fn insert_roles(
    transaction: &mut Transaction<'_, Postgres>,
    invite_id: i32,
    role_ids: &[String],
    assignment_mode: RoleAssignmentMode,
) -> sqlx::Result<()> {
    for role_id in role_ids {
        sqlx::query(
            "INSERT INTO invite_roles (tracked_invite_id, role_id, assignment_mode) \
             VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(invite_id)
        .bind(role_id)
        .bind(assignment_mode.as_str())
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate};
    use sqlx::PgPool;

    use super::Repository;
    use crate::models::{InviteSync, NewJoinEvent, NewTrackedInvite, RoleAssignmentMode};

    const TEST_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

    #[sqlx::test(migrator = "TEST_MIGRATOR")]
    #[ignore = "requires a local PostgreSQL server"]
    async fn membership_sessions_are_idempotent_and_survive_invite_deletion(
        pool: PgPool,
    ) -> sqlx::Result<()> {
        let repository = Repository::new(pool);
        repository.ensure_guild("1").await?;
        let invite = repository.insert_invite(&test_invite()).await?;
        let join = test_join(invite.id);

        assert!(repository.record_join(&join).await?);
        assert!(!repository.record_join(&join).await?);
        repository.stop_tracking(invite.id, true).await?;

        assert_eq!(repository.count_invite_joins(invite.id).await?, 1);
        let retained = repository.find_invite("1", "launch").await?.unwrap();
        assert_eq!(retained.status, "revoked");
        sqlx::query("DELETE FROM tracked_invites WHERE id = $1")
            .bind(invite.id)
            .execute(&repository.pool)
            .await?;

        let exported = repository.export_joins("1").await?;
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].invite_code.as_deref(), Some("launch"));
        Ok(())
    }

    #[sqlx::test(migrator = "TEST_MIGRATOR")]
    #[ignore = "requires a local PostgreSQL server"]
    async fn source_edits_do_not_rewrite_historical_attribution(pool: PgPool) -> sqlx::Result<()> {
        let repository = Repository::new(pool);
        repository.ensure_guild("1").await?;
        let invite = repository.insert_invite(&test_invite()).await?;
        repository.record_join(&test_join(invite.id)).await?;

        repository
            .update_invite_sources(invite.id, "Changed", "Later")
            .await?;

        let exported = repository.export_joins("1").await?;
        assert_eq!(exported[0].primary_source.as_deref(), Some("Social"));
        assert_eq!(exported[0].secondary_source.as_deref(), Some("Launch"));
        Ok(())
    }

    #[sqlx::test(migrator = "TEST_MIGRATOR")]
    #[ignore = "requires a local PostgreSQL server"]
    async fn metadata_sync_replaces_native_roles_without_touching_managed_roles(
        pool: PgPool,
    ) -> sqlx::Result<()> {
        let repository = Repository::new(pool);
        repository.ensure_guild("1").await?;
        let mut new_invite = test_invite();
        new_invite.role_ids = vec!["native-old".to_owned()];
        let invite = repository.insert_invite(&new_invite).await?;
        repository
            .replace_managed_roles(invite.id, &["managed".to_owned()])
            .await?;

        let mut sync = test_sync();
        sync.role_ids = vec!["native-new".to_owned()];
        repository.sync_invite("1", &sync).await?;
        assert_eq!(
            repository.native_invite_role_ids(invite.id).await?,
            vec!["native-new"]
        );
        assert_eq!(
            repository.managed_invite_role_ids(invite.id).await?,
            vec!["managed"]
        );

        sync.role_ids.clear();
        repository.sync_invite("1", &sync).await?;
        assert!(
            repository
                .native_invite_role_ids(invite.id)
                .await?
                .is_empty()
        );
        assert_eq!(
            repository.managed_invite_role_ids(invite.id).await?,
            vec!["managed"]
        );
        Ok(())
    }

    #[sqlx::test(migrator = "TEST_MIGRATOR")]
    #[ignore = "requires a local PostgreSQL server"]
    async fn disabling_vanity_tracking_closes_the_active_vanity_source(
        pool: PgPool,
    ) -> sqlx::Result<()> {
        let repository = Repository::new(pool);
        repository.ensure_guild("1").await?;
        repository
            .upsert_vanity_invite("1", "vanity", 10, "Direct", "Vanity")
            .await?;

        repository
            .set_vanity_tracking("1", false, "Direct", "Vanity")
            .await?;

        let invite = repository.find_invite("1", "vanity").await?.unwrap();
        assert!(!invite.tracking_enabled);
        assert_eq!(invite.status, "untracked");
        Ok(())
    }

    #[sqlx::test(migrator = "TEST_MIGRATOR")]
    #[ignore = "requires a local PostgreSQL server"]
    async fn automated_members_do_not_pollute_human_analytics(pool: PgPool) -> sqlx::Result<()> {
        let repository = Repository::new(pool);
        repository.ensure_guild("1").await?;
        let invite = repository.insert_invite(&test_invite()).await?;
        repository.record_join(&test_join(invite.id)).await?;

        let mut system_join = test_join(invite.id);
        system_join.user_id = "system".to_owned();
        system_join.member_joined_at += Duration::seconds(1);
        system_join.is_system = true;
        system_join.attribution_status = "not_applicable".to_owned();
        repository.record_join(&system_join).await?;

        let counts = repository.analytics_counts("1", None).await?;
        assert_eq!(counts.total, 1);
        assert_eq!(counts.attributed, 1);
        assert_eq!(counts.bots, 1);
        assert_eq!(
            repository.top_primary_sources("1", None, 10).await?[0].joins,
            1
        );
        Ok(())
    }

    fn test_invite() -> NewTrackedInvite {
        NewTrackedInvite {
            guild_id: "1".to_owned(),
            invite_code: "launch".to_owned(),
            channel_id: "2".to_owned(),
            channel_type: 0,
            primary_source: "Social".to_owned(),
            secondary_source: "Launch".to_owned(),
            tracked_by: "3".to_owned(),
            discord_inviter_id: Some("4".to_owned()),
            discord_created_at: None,
            discord_uses: 0,
            max_uses: 0,
            max_age: 0,
            temporary: false,
            expires_at: None,
            invite_type: 0,
            flags: 0,
            target_type: None,
            target_user_id: None,
            target_application_id: None,
            scheduled_event_id: None,
            targeted_user_count: None,
            is_vanity: false,
            tracked_at: None,
            role_ids: Vec::new(),
            role_assignment_mode: RoleAssignmentMode::Native,
        }
    }

    fn test_join(invite_id: i32) -> NewJoinEvent {
        let joined_at = NaiveDate::from_ymd_opt(2026, 7, 23)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        NewJoinEvent {
            tracked_invite_id: Some(invite_id),
            guild_id: "1".to_owned(),
            user_id: "5".to_owned(),
            member_joined_at: joined_at,
            account_created_at: joined_at,
            invite_code_snapshot: Some("launch".to_owned()),
            primary_source_snapshot: Some("Social".to_owned()),
            secondary_source_snapshot: Some("Launch".to_owned()),
            attribution_status: "attributed".to_owned(),
            attribution_reason: Some("single_counter_increase".to_owned()),
            attribution_confidence: "high".to_owned(),
            is_bot: false,
            is_system: false,
            member_flags: 0,
            pending: false,
        }
    }

    fn test_sync() -> InviteSync {
        InviteSync {
            invite_code: "launch".to_owned(),
            channel_id: "2".to_owned(),
            channel_type: 0,
            discord_inviter_id: Some("4".to_owned()),
            discord_created_at: NaiveDate::from_ymd_opt(2026, 7, 23)
                .unwrap()
                .and_hms_opt(11, 0, 0)
                .unwrap(),
            discord_uses: 0,
            max_uses: 0,
            max_age: 0,
            temporary: false,
            expires_at: None,
            invite_type: 0,
            flags: 0,
            target_type: None,
            target_user_id: None,
            target_application_id: None,
            scheduled_event_id: None,
            role_ids: Vec::new(),
        }
    }
}
