use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Clone, Debug, FromRow)]
pub struct GuildConfig {
    pub id: String,
    pub default_channel_id: Option<String>,
    pub log_channel_id: Option<String>,
    pub max_links: i32,
    pub track_vanity: bool,
    pub vanity_primary_source: String,
    pub vanity_secondary_source: String,
    pub last_synced_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "independent Discord invite state fields map directly to database columns"
)]
#[derive(Clone, Debug, FromRow)]
pub struct TrackedInvite {
    pub id: i32,
    pub guild_id: String,
    pub invite_code: String,
    pub channel_id: String,
    pub channel_type: i32,
    pub primary_source: String,
    pub secondary_source: String,
    pub tracked_by: String,
    pub discord_inviter_id: Option<String>,
    pub discord_created_at: Option<NaiveDateTime>,
    pub discord_uses: i64,
    pub max_uses: i32,
    pub max_age: i32,
    pub temporary: bool,
    pub expires_at: Option<NaiveDateTime>,
    pub invite_type: i32,
    pub flags: i64,
    pub target_type: Option<i32>,
    pub target_user_id: Option<String>,
    pub target_application_id: Option<String>,
    pub scheduled_event_id: Option<String>,
    pub targeted_user_count: Option<i32>,
    pub is_vanity: bool,
    pub tracking_enabled: bool,
    pub discord_active: bool,
    pub status: String,
    pub deleted_at: Option<NaiveDateTime>,
    pub last_synced_at: Option<NaiveDateTime>,
    pub tracked_at: NaiveDateTime,
}

#[derive(Clone, Debug)]
pub struct NewTrackedInvite {
    pub guild_id: String,
    pub invite_code: String,
    pub channel_id: String,
    pub channel_type: i32,
    pub primary_source: String,
    pub secondary_source: String,
    pub tracked_by: String,
    pub discord_inviter_id: Option<String>,
    pub discord_created_at: Option<NaiveDateTime>,
    pub discord_uses: i64,
    pub max_uses: i32,
    pub max_age: i32,
    pub temporary: bool,
    pub expires_at: Option<NaiveDateTime>,
    pub invite_type: i32,
    pub flags: i64,
    pub target_type: Option<i32>,
    pub target_user_id: Option<String>,
    pub target_application_id: Option<String>,
    pub scheduled_event_id: Option<String>,
    pub targeted_user_count: Option<i32>,
    pub is_vanity: bool,
    pub tracked_at: Option<NaiveDateTime>,
    pub role_ids: Vec<String>,
    pub role_assignment_mode: RoleAssignmentMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleAssignmentMode {
    Managed,
    Native,
}

impl RoleAssignmentMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Native => "native",
        }
    }
}

#[derive(Clone, Debug)]
pub struct InviteSync {
    pub invite_code: String,
    pub channel_id: String,
    pub channel_type: i32,
    pub discord_inviter_id: Option<String>,
    pub discord_created_at: NaiveDateTime,
    pub discord_uses: i64,
    pub max_uses: i32,
    pub max_age: i32,
    pub temporary: bool,
    pub expires_at: Option<NaiveDateTime>,
    pub invite_type: i32,
    pub flags: i64,
    pub target_type: Option<i32>,
    pub target_user_id: Option<String>,
    pub target_application_id: Option<String>,
    pub scheduled_event_id: Option<String>,
    pub role_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct NewJoinEvent {
    pub tracked_invite_id: Option<i32>,
    pub guild_id: String,
    pub user_id: String,
    pub member_joined_at: NaiveDateTime,
    pub account_created_at: NaiveDateTime,
    pub invite_code_snapshot: Option<String>,
    pub primary_source_snapshot: Option<String>,
    pub secondary_source_snapshot: Option<String>,
    pub attribution_status: String,
    pub attribution_reason: Option<String>,
    pub attribution_confidence: String,
    pub is_bot: bool,
    pub is_system: bool,
    pub member_flags: i64,
    pub pending: bool,
}

#[derive(Clone, Debug, FromRow)]
pub struct InviteUse {
    pub user_id: String,
    pub member_joined_at: NaiveDateTime,
    pub left_at: Option<NaiveDateTime>,
}

#[derive(Clone, Debug, FromRow, PartialEq, Eq)]
pub struct SourceCount {
    pub source: String,
    pub joins: i64,
}

#[derive(Clone, Debug, FromRow)]
pub struct DailyCount {
    pub day: chrono::NaiveDate,
    pub joins: i64,
}

#[derive(Clone, Debug, FromRow)]
pub struct AnalyticsCounts {
    pub total: i64,
    pub attributed: i64,
    pub unattributed: i64,
    pub bots: i64,
}

#[derive(Clone, Debug, FromRow)]
pub struct RetentionCount {
    pub source: String,
    pub joined: i64,
    pub active: i64,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "the export mirrors independent persisted invite-state fields"
)]
#[derive(Clone, Debug, FromRow)]
pub struct ExportInvite {
    pub invite_code: String,
    pub channel_id: String,
    pub channel_type: i32,
    pub primary_source: String,
    pub secondary_source: String,
    pub tracked_by: String,
    pub discord_inviter_id: Option<String>,
    pub discord_created_at: Option<NaiveDateTime>,
    pub discord_uses: i64,
    pub attributed_joins: i64,
    pub max_uses: i32,
    pub max_age: i32,
    pub temporary: bool,
    pub expires_at: Option<NaiveDateTime>,
    pub invite_type: i32,
    pub flags: i64,
    pub target_type: Option<i32>,
    pub target_user_id: Option<String>,
    pub target_application_id: Option<String>,
    pub scheduled_event_id: Option<String>,
    pub targeted_user_count: Option<i32>,
    pub is_vanity: bool,
    pub tracking_enabled: bool,
    pub discord_active: bool,
    pub status: String,
    pub deleted_at: Option<NaiveDateTime>,
    pub last_synced_at: Option<NaiveDateTime>,
    pub tracked_at: NaiveDateTime,
    pub native_role_ids: Vec<String>,
    pub managed_role_ids: Vec<String>,
}

#[derive(Clone, Debug, FromRow)]
pub struct ExportJoin {
    pub user_id: String,
    pub observed_at: NaiveDateTime,
    pub member_joined_at: NaiveDateTime,
    pub account_created_at: Option<NaiveDateTime>,
    pub left_at: Option<NaiveDateTime>,
    pub screening_completed_at: Option<NaiveDateTime>,
    pub invite_code: Option<String>,
    pub primary_source: Option<String>,
    pub secondary_source: Option<String>,
    pub attribution_status: String,
    pub attribution_reason: Option<String>,
    pub attribution_confidence: String,
    pub is_bot: bool,
    pub is_system: bool,
    pub member_flags: i64,
    pub pending: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PaginationState {
    pub current_page: u32,
    pub total_pages: u32,
    pub guild_id: String,
    pub guild_name: String,
    pub command_name: String,
}
