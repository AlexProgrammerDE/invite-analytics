use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Clone, Debug, FromRow)]
pub struct GuildConfig {
    pub id: String,
    pub default_channel_id: Option<String>,
    pub log_channel_id: Option<String>,
    pub max_links: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Clone, Debug, FromRow)]
pub struct TrackedInvite {
    pub id: i32,
    pub guild_id: String,
    pub invite_code: String,
    pub channel_id: String,
    pub primary_source: String,
    pub secondary_source: String,
    pub created_by: String,
    pub uses: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Clone, Debug)]
pub struct NewTrackedInvite {
    pub guild_id: String,
    pub invite_code: String,
    pub channel_id: String,
    pub primary_source: String,
    pub secondary_source: String,
    pub created_by: String,
    pub uses: i32,
    pub created_at: Option<NaiveDateTime>,
    pub role_ids: Vec<String>,
}

#[derive(Clone, Debug, FromRow)]
pub struct InviteUse {
    pub user_id: String,
    pub joined_at: NaiveDateTime,
}

#[derive(Clone, Debug, FromRow, PartialEq, Eq)]
pub struct SourceCount {
    pub source: String,
    pub joins: i64,
}

#[derive(Clone, Debug, FromRow)]
pub struct ExportInvite {
    pub invite_code: String,
    pub primary_source: String,
    pub secondary_source: String,
    pub created_by: String,
    pub created_at: NaiveDateTime,
    pub role_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PaginationState {
    pub current_page: u32,
    pub total_pages: u32,
    pub guild_id: String,
    pub guild_name: String,
    pub command_name: String,
}
