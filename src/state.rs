use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::Mutex;

use crate::cache::AppCache;
use crate::db::Repository;

#[derive(Clone)]
pub struct BotData {
    pub repository: Repository,
    pub cache: AppCache,
    pub attachment_client: reqwest::Client,
    join_locks: Arc<DashMap<u64, Arc<Mutex<()>>>>,
}

impl BotData {
    pub fn new(
        repository: Repository,
        cache: AppCache,
        attachment_client: reqwest::Client,
    ) -> Self {
        Self {
            repository,
            cache,
            attachment_client,
            join_locks: Arc::new(DashMap::new()),
        }
    }

    pub fn join_lock(&self, guild_id: u64) -> Arc<Mutex<()>> {
        self.join_locks
            .entry(guild_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}
