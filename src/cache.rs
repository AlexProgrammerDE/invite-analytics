use std::collections::HashMap;

use anyhow::Context as _;
use chrono::Utc;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;

use crate::models::PaginationState;

const PAGINATION_TTL_SECONDS: u64 = 300;
const RECENTLY_DELETED_TTL_SECONDS: u64 = 60;
const SNAPSHOT_READY_FIELD: &str = "__invite_analytics_snapshot_ready";

#[derive(Clone)]
pub struct AppCache {
    connection: ConnectionManager,
}

impl AppCache {
    pub fn new(connection: ConnectionManager) -> Self {
        Self { connection }
    }

    pub async fn ping(&self) -> anyhow::Result<()> {
        let mut connection = self.connection.clone();
        redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
            .context("failed to ping Redis")?;
        Ok(())
    }

    pub async fn invite_snapshot(&self, guild_id: &str) -> anyhow::Result<HashMap<String, u64>> {
        let mut connection = self.connection.clone();
        let mut snapshot: HashMap<String, u64> = redis::cmd("HGETALL")
            .arg(invite_key(guild_id))
            .query_async(&mut connection)
            .await
            .context("failed to read the invite cache")?;
        snapshot.remove(SNAPSHOT_READY_FIELD);
        Ok(snapshot)
    }

    pub async fn replace_invite_snapshot(
        &self,
        guild_id: &str,
        invites: &HashMap<String, u64>,
    ) -> anyhow::Result<()> {
        let key = invite_key(guild_id);
        let mut pipeline = redis::pipe();
        pipeline.atomic().del(&key).ignore();
        pipeline.hset(&key, SNAPSHOT_READY_FIELD, 1_u8).ignore();
        for (code, uses) in invites {
            pipeline.hset(&key, code, *uses).ignore();
        }

        let mut connection = self.connection.clone();
        pipeline
            .query_async::<()>(&mut connection)
            .await
            .context("failed to replace the invite cache")
    }

    pub async fn invite_snapshot_initialized(&self, guild_id: &str) -> anyhow::Result<bool> {
        let mut connection = self.connection.clone();
        connection
            .hexists(invite_key(guild_id), SNAPSHOT_READY_FIELD)
            .await
            .context("failed to inspect the invite cache")
    }

    pub async fn set_invite(&self, guild_id: &str, code: &str, uses: u64) -> anyhow::Result<()> {
        let mut connection = self.connection.clone();
        connection
            .hset::<_, _, _, ()>(invite_key(guild_id), code, uses)
            .await
            .context("failed to update the invite cache")
    }

    pub async fn remove_invite(&self, guild_id: &str, code: &str) -> anyhow::Result<()> {
        let mut connection = self.connection.clone();
        connection
            .hdel::<_, _, ()>(invite_key(guild_id), code)
            .await
            .context("failed to remove an invite from the cache")
    }

    pub async fn remember_deleted_invite(&self, guild_id: &str, code: &str) -> anyhow::Result<()> {
        let invite_key = invite_key(guild_id);
        let deleted_key = recently_deleted_key(guild_id);
        let mut connection = self.connection.clone();
        let uses = connection
            .hget::<_, _, Option<u64>>(&invite_key, code)
            .await
            .context("failed to read a deleted invite from the cache")?;

        let mut pipeline = redis::pipe();
        pipeline.atomic().hdel(invite_key, code).ignore();
        if let Some(uses) = uses {
            pipeline
                .hset(
                    &deleted_key,
                    code,
                    deleted_invite_value(uses, Utc::now().timestamp()),
                )
                .ignore();
            pipeline
                .expire(&deleted_key, RECENTLY_DELETED_TTL_SECONDS.cast_signed())
                .ignore();
        }
        pipeline
            .query_async::<()>(&mut connection)
            .await
            .context("failed to remember a deleted invite")
    }

    pub async fn recently_deleted_invites(
        &self,
        guild_id: &str,
    ) -> anyhow::Result<HashMap<String, u64>> {
        let key = recently_deleted_key(guild_id);
        let mut connection = self.connection.clone();
        let values: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&key)
            .query_async(&mut connection)
            .await
            .context("failed to read recently deleted invites")?;
        let now = Utc::now().timestamp();
        let mut active = HashMap::new();
        let mut stale = Vec::new();
        for (code, value) in values {
            if let Some(uses) = parse_deleted_invite_value(&value, now) {
                active.insert(code, uses);
            } else {
                stale.push(code);
            }
        }
        if !stale.is_empty() {
            let mut pipeline = redis::pipe();
            for code in stale {
                pipeline.hdel(&key, code).ignore();
            }
            pipeline
                .query_async::<()>(&mut connection)
                .await
                .context("failed to prune expired deleted invites")?;
        }
        Ok(active)
    }

    pub async fn clear_recently_deleted_invite(
        &self,
        guild_id: &str,
        code: &str,
    ) -> anyhow::Result<()> {
        let mut connection = self.connection.clone();
        connection
            .hdel::<_, _, ()>(recently_deleted_key(guild_id), code)
            .await
            .context("failed to clear a recently deleted invite")
    }

    pub async fn save_pagination(
        &self,
        message_id: u64,
        state: &PaginationState,
    ) -> anyhow::Result<()> {
        let serialized =
            serde_json::to_string(state).context("failed to serialize pagination state")?;
        let mut connection = self.connection.clone();
        connection
            .set_ex::<_, _, ()>(
                pagination_key(message_id),
                serialized,
                PAGINATION_TTL_SECONDS,
            )
            .await
            .context("failed to save pagination state")
    }

    pub async fn pagination(&self, message_id: u64) -> anyhow::Result<Option<PaginationState>> {
        let mut connection = self.connection.clone();
        let serialized: Option<String> = connection
            .get(pagination_key(message_id))
            .await
            .context("failed to read pagination state")?;
        serialized
            .map(|value| {
                serde_json::from_str(&value).context("pagination state in Redis is invalid")
            })
            .transpose()
    }
}

fn invite_key(guild_id: &str) -> String {
    format!("guild:{guild_id}:invites")
}

fn recently_deleted_key(guild_id: &str) -> String {
    format!("guild:{guild_id}:recently-deleted-invites")
}

fn pagination_key(message_id: u64) -> String {
    format!("pagination:{message_id}")
}

fn deleted_invite_value(uses: u64, deleted_at: i64) -> String {
    format!("{uses}:{deleted_at}")
}

fn parse_deleted_invite_value(value: &str, now: i64) -> Option<u64> {
    let (uses, deleted_at) = value.split_once(':')?;
    let uses = uses.parse().ok()?;
    let deleted_at = deleted_at.parse::<i64>().ok()?;
    let age = now.saturating_sub(deleted_at);
    (age >= 0 && age <= RECENTLY_DELETED_TTL_SECONDS.cast_signed()).then_some(uses)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anyhow::Context as _;

    use super::{
        AppCache, SNAPSHOT_READY_FIELD, deleted_invite_value, invite_key, pagination_key,
        parse_deleted_invite_value, recently_deleted_key,
    };

    #[test]
    fn cache_keys_are_stable() {
        assert_eq!(invite_key("42"), "guild:42:invites");
        assert_eq!(SNAPSHOT_READY_FIELD, "__invite_analytics_snapshot_ready");
        assert_eq!(
            recently_deleted_key("42"),
            "guild:42:recently-deleted-invites"
        );
        assert_eq!(pagination_key(7), "pagination:7");
    }

    #[test]
    fn deleted_invites_expire_independently() {
        let value = deleted_invite_value(4, 1_000);
        assert_eq!(parse_deleted_invite_value(&value, 1_060), Some(4));
        assert_eq!(parse_deleted_invite_value(&value, 1_061), None);
    }

    #[tokio::test]
    #[ignore = "requires a local Redis server"]
    async fn redis_snapshot_and_deleted_invite_lifecycle_is_consistent() -> anyhow::Result<()> {
        let redis_url = std::env::var("REDIS_URL").context("REDIS_URL is required")?;
        let client = redis::Client::open(redis_url)?;
        let connection = redis::aio::ConnectionManager::new(client).await?;
        let cache = AppCache::new(connection);
        let guild_id = "invite-analytics-integration-test";
        clear_test_keys(&cache, guild_id).await?;

        cache
            .replace_invite_snapshot(guild_id, &HashMap::new())
            .await?;
        assert!(cache.invite_snapshot_initialized(guild_id).await?);
        assert!(cache.invite_snapshot(guild_id).await?.is_empty());

        cache.set_invite(guild_id, "limited", 0).await?;
        cache.remember_deleted_invite(guild_id, "limited").await?;
        assert!(cache.invite_snapshot(guild_id).await?.is_empty());
        assert_eq!(
            cache.recently_deleted_invites(guild_id).await?["limited"],
            0
        );

        cache
            .clear_recently_deleted_invite(guild_id, "limited")
            .await?;
        assert!(cache.recently_deleted_invites(guild_id).await?.is_empty());
        clear_test_keys(&cache, guild_id).await
    }

    async fn clear_test_keys(cache: &AppCache, guild_id: &str) -> anyhow::Result<()> {
        let mut connection = cache.connection.clone();
        redis::cmd("DEL")
            .arg(invite_key(guild_id))
            .arg(recently_deleted_key(guild_id))
            .query_async::<usize>(&mut connection)
            .await?;
        Ok(())
    }
}
