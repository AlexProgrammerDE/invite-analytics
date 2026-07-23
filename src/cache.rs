use std::collections::HashMap;

use anyhow::Context as _;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;

use crate::models::PaginationState;

const PAGINATION_TTL_SECONDS: u64 = 300;

#[derive(Clone)]
pub struct AppCache {
    connection: ConnectionManager,
}

impl AppCache {
    pub fn new(connection: ConnectionManager) -> Self {
        Self { connection }
    }

    pub async fn invite_snapshot(&self, guild_id: &str) -> anyhow::Result<HashMap<String, u64>> {
        let mut connection = self.connection.clone();
        redis::cmd("HGETALL")
            .arg(invite_key(guild_id))
            .query_async(&mut connection)
            .await
            .context("failed to read the invite cache")
    }

    pub async fn replace_invite_snapshot(
        &self,
        guild_id: &str,
        invites: &HashMap<String, u64>,
    ) -> anyhow::Result<()> {
        let key = invite_key(guild_id);
        let mut pipeline = redis::pipe();
        pipeline.atomic().del(&key).ignore();
        for (code, uses) in invites {
            pipeline.hset(&key, code, *uses).ignore();
        }

        let mut connection = self.connection.clone();
        pipeline
            .query_async::<()>(&mut connection)
            .await
            .context("failed to replace the invite cache")
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

fn pagination_key(message_id: u64) -> String {
    format!("pagination:{message_id}")
}

#[cfg(test)]
mod tests {
    use super::{invite_key, pagination_key};

    #[test]
    fn cache_keys_are_stable() {
        assert_eq!(invite_key("42"), "guild:42:invites");
        assert_eq!(pagination_key(7), "pagination:7");
    }
}
