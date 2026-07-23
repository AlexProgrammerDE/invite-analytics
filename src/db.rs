use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};

use crate::models::{
    ExportInvite, GuildConfig, InviteUse, NewTrackedInvite, SourceCount, TrackedInvite,
};

#[derive(Clone)]
pub struct Repository {
    pool: PgPool,
}

impl Repository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn ensure_guild(&self, guild_id: &str) -> sqlx::Result<()> {
        sqlx::query("INSERT INTO guilds (id) VALUES ($1) ON CONFLICT (id) DO NOTHING")
            .bind(guild_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn guild_config(&self, guild_id: &str) -> sqlx::Result<Option<GuildConfig>> {
        sqlx::query_as::<_, GuildConfig>(
            "SELECT id, default_channel_id, log_channel_id, max_links, \
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
        sqlx::query_scalar("SELECT COUNT(*) FROM tracked_invites WHERE guild_id = $1")
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
            "SELECT id, guild_id, invite_code, channel_id, primary_source, \
             secondary_source, created_by, uses, created_at \
             FROM tracked_invites WHERE guild_id = $1 \
             ORDER BY created_at, id LIMIT $2 OFFSET $3",
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
            "SELECT id, guild_id, invite_code, channel_id, primary_source, \
             secondary_source, created_by, uses, created_at \
             FROM tracked_invites WHERE guild_id = $1 AND invite_code = $2",
        )
        .bind(guild_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn invite_exists(&self, code: &str) -> sqlx::Result<bool> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tracked_invites WHERE invite_code = $1)")
            .bind(code)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn insert_invite(&self, invite: &NewTrackedInvite) -> sqlx::Result<TrackedInvite> {
        let mut transaction = self.pool.begin().await?;
        let inserted = sqlx::query_as::<_, TrackedInvite>(
            "INSERT INTO tracked_invites \
             (guild_id, invite_code, channel_id, primary_source, \
              secondary_source, created_by, uses, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, COALESCE($8, NOW())) \
             RETURNING id, guild_id, invite_code, channel_id, primary_source, \
             secondary_source, created_by, uses, created_at",
        )
        .bind(&invite.guild_id)
        .bind(&invite.invite_code)
        .bind(&invite.channel_id)
        .bind(&invite.primary_source)
        .bind(&invite.secondary_source)
        .bind(&invite.created_by)
        .bind(invite.uses)
        .bind(invite.created_at)
        .fetch_one(&mut *transaction)
        .await?;

        insert_roles(&mut transaction, inserted.id, &invite.role_ids).await?;
        transaction.commit().await?;
        Ok(inserted)
    }

    pub async fn delete_invite(&self, invite_id: i32) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM tracked_invites WHERE id = $1")
            .bind(invite_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn invite_role_ids(&self, invite_id: i32) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar(
            "SELECT role_id FROM invite_roles \
             WHERE tracked_invite_id = $1 ORDER BY id",
        )
        .bind(invite_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn recent_joins(&self, invite_id: i32, limit: i64) -> sqlx::Result<Vec<InviteUse>> {
        sqlx::query_as::<_, InviteUse>(
            "SELECT user_id, joined_at FROM invite_uses \
             WHERE tracked_invite_id = $1 ORDER BY joined_at DESC LIMIT $2",
        )
        .bind(invite_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn record_join(
        &self,
        invite_id: i32,
        guild_id: &str,
        user_id: &str,
    ) -> sqlx::Result<()> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO invite_uses (tracked_invite_id, guild_id, user_id) \
             VALUES ($1, $2, $3)",
        )
        .bind(invite_id)
        .bind(guild_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;

        sqlx::query("UPDATE tracked_invites SET uses = uses + 1 WHERE id = $1")
            .bind(invite_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await
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

    pub async fn count_joins(
        &self,
        guild_id: &str,
        since: Option<NaiveDateTime>,
    ) -> sqlx::Result<i64> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM invite_uses \
             WHERE guild_id = $1 AND ($2::TIMESTAMP IS NULL OR joined_at >= $2)",
        )
        .bind(guild_id)
        .bind(since)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn top_primary_sources(
        &self,
        guild_id: &str,
        since: Option<NaiveDateTime>,
        limit: i64,
    ) -> sqlx::Result<Vec<SourceCount>> {
        sqlx::query_as::<_, SourceCount>(
            "SELECT tracked_invites.primary_source AS source, COUNT(*) AS joins \
             FROM invite_uses \
             INNER JOIN tracked_invites \
               ON tracked_invites.id = invite_uses.tracked_invite_id \
             WHERE invite_uses.guild_id = $1 \
               AND ($2::TIMESTAMP IS NULL OR invite_uses.joined_at >= $2) \
             GROUP BY tracked_invites.primary_source \
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
            "SELECT tracked_invites.secondary_source AS source, \
                    COUNT(*) AS joins \
             FROM invite_uses \
             INNER JOIN tracked_invites \
               ON tracked_invites.id = invite_uses.tracked_invite_id \
             WHERE invite_uses.guild_id = $1 \
               AND ($2::TIMESTAMP IS NULL OR invite_uses.joined_at >= $2) \
               AND ($3::TEXT IS NULL OR tracked_invites.primary_source = $3) \
             GROUP BY tracked_invites.secondary_source \
             ORDER BY joins DESC, source ASC LIMIT $4",
        )
        .bind(guild_id)
        .bind(since)
        .bind(primary_source)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn export_invites(&self, guild_id: &str) -> sqlx::Result<Vec<ExportInvite>> {
        sqlx::query_as::<_, ExportInvite>(
            "SELECT tracked_invites.invite_code, \
                    tracked_invites.primary_source, \
                    tracked_invites.secondary_source, \
                    tracked_invites.created_by, \
                    tracked_invites.created_at, \
                    COALESCE( \
                        ARRAY_AGG(invite_roles.role_id ORDER BY invite_roles.id) \
                            FILTER (WHERE invite_roles.role_id IS NOT NULL), \
                        ARRAY[]::TEXT[] \
                    ) AS role_ids \
             FROM tracked_invites \
             LEFT JOIN invite_roles \
               ON invite_roles.tracked_invite_id = tracked_invites.id \
             WHERE tracked_invites.guild_id = $1 \
             GROUP BY tracked_invites.id \
             ORDER BY tracked_invites.created_at, tracked_invites.id",
        )
        .bind(guild_id)
        .fetch_all(&self.pool)
        .await
    }
}

async fn insert_roles(
    transaction: &mut Transaction<'_, Postgres>,
    invite_id: i32,
    role_ids: &[String],
) -> sqlx::Result<()> {
    for role_id in role_ids {
        sqlx::query(
            "INSERT INTO invite_roles (tracked_invite_id, role_id) \
             VALUES ($1, $2)",
        )
        .bind(invite_id)
        .bind(role_id)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}
