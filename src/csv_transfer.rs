use anyhow::{Context as _, bail};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::invite_tracking::normalize_invite_code;
use crate::models::{ExportInvite, ExportJoin};

const MAX_IMPORT_ROWS: usize = 1_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedInvite {
    pub invite_code: String,
    pub primary_source: String,
    pub secondary_source: String,
    pub tracked_by: String,
    pub tracked_at: Option<NaiveDateTime>,
    pub role_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct InviteCsvRecord {
    #[serde(rename = "Invite Code")]
    invite_code: String,
    #[serde(rename = "Channel ID", default)]
    channel_id: String,
    #[serde(rename = "Channel Type", default)]
    channel_type: String,
    #[serde(rename = "Primary Source")]
    primary_source: String,
    #[serde(rename = "Secondary Source")]
    secondary_source: String,
    #[serde(rename = "Tracked By", alias = "Link Creator ID")]
    tracked_by: String,
    #[serde(rename = "Discord Inviter ID", default)]
    discord_inviter_id: String,
    #[serde(rename = "Discord Created At", default)]
    discord_created_at: String,
    #[serde(rename = "Discord Uses", default)]
    discord_uses: String,
    #[serde(rename = "Attributed Joins", default)]
    attributed_joins: String,
    #[serde(rename = "Max Uses", default)]
    max_uses: String,
    #[serde(rename = "Max Age Seconds", default)]
    max_age: String,
    #[serde(rename = "Temporary", default)]
    temporary: String,
    #[serde(rename = "Expires At", default)]
    expires_at: String,
    #[serde(rename = "Invite Type", default)]
    invite_type: String,
    #[serde(rename = "Flags", default)]
    flags: String,
    #[serde(rename = "Target Type", default)]
    target_type: String,
    #[serde(rename = "Target User ID", default)]
    target_user_id: String,
    #[serde(rename = "Target Application ID", default)]
    target_application_id: String,
    #[serde(rename = "Scheduled Event ID", default)]
    scheduled_event_id: String,
    #[serde(rename = "Targeted User Count", default)]
    targeted_user_count: String,
    #[serde(rename = "Vanity", default)]
    is_vanity: String,
    #[serde(rename = "Tracking Enabled", default)]
    tracking_enabled: String,
    #[serde(rename = "Discord Active", default)]
    discord_active: String,
    #[serde(rename = "Status", default)]
    status: String,
    #[serde(rename = "Deleted At", default)]
    deleted_at: String,
    #[serde(rename = "Last Synced At", default)]
    last_synced_at: String,
    #[serde(rename = "Tracked At", alias = "Creation Time", default)]
    tracked_at: String,
    #[serde(rename = "Native Roles", default)]
    native_role_ids: String,
    #[serde(
        rename = "Managed Roles",
        alias = "Roles",
        alias = "Roles to Give on join",
        default
    )]
    managed_role_ids: String,
}

#[derive(Debug, Serialize)]
struct JoinCsvRecord {
    #[serde(rename = "User ID")]
    user_id: String,
    #[serde(rename = "Observed At")]
    observed_at: String,
    #[serde(rename = "Member Joined At")]
    member_joined_at: String,
    #[serde(rename = "Account Created At")]
    account_created_at: String,
    #[serde(rename = "Left At")]
    left_at: String,
    #[serde(rename = "Screening Completed At")]
    screening_completed_at: String,
    #[serde(rename = "Invite Code")]
    invite_code: String,
    #[serde(rename = "Primary Source")]
    primary_source: String,
    #[serde(rename = "Secondary Source")]
    secondary_source: String,
    #[serde(rename = "Attribution Status")]
    attribution_status: String,
    #[serde(rename = "Attribution Reason")]
    attribution_reason: String,
    #[serde(rename = "Attribution Confidence")]
    attribution_confidence: String,
    #[serde(rename = "Is Bot")]
    is_bot: bool,
    #[serde(rename = "Is System User")]
    is_system: bool,
    #[serde(rename = "Member Flags")]
    member_flags: i64,
    #[serde(rename = "Pending Screening")]
    pending: bool,
}

pub fn parse_invite_import(bytes: &[u8]) -> anyhow::Result<Vec<ImportedInvite>> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(bytes);
    let mut rows = Vec::new();

    for (index, result) in reader.deserialize::<InviteCsvRecord>().enumerate() {
        if index >= MAX_IMPORT_ROWS {
            bail!("CSV files can contain at most {MAX_IMPORT_ROWS} rows");
        }

        let record = result.with_context(|| format!("invalid CSV record on row {}", index + 2))?;
        let invite_code = normalize_invite_code(&record.invite_code);
        if invite_code.is_empty()
            || record.primary_source.is_empty()
            || record.secondary_source.is_empty()
            || record.tracked_by.is_empty()
        {
            bail!("row {} has a missing required value", index + 2);
        }

        rows.push(ImportedInvite {
            invite_code,
            primary_source: record.primary_source,
            secondary_source: record.secondary_source,
            tracked_by: record.tracked_by,
            tracked_at: parse_date(&record.tracked_at)
                .with_context(|| format!("invalid tracking time on row {}", index + 2))?,
            role_ids: record
                .managed_role_ids
                .split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        });
    }

    Ok(rows)
}

pub fn write_invite_export(invites: &[ExportInvite]) -> anyhow::Result<Vec<u8>> {
    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    for invite in invites {
        writer.serialize(InviteCsvRecord {
            invite_code: invite.invite_code.clone(),
            channel_id: invite.channel_id.clone(),
            channel_type: invite.channel_type.to_string(),
            primary_source: invite.primary_source.clone(),
            secondary_source: invite.secondary_source.clone(),
            tracked_by: invite.tracked_by.clone(),
            discord_inviter_id: invite.discord_inviter_id.clone().unwrap_or_default(),
            discord_created_at: invite
                .discord_created_at
                .map(format_date)
                .unwrap_or_default(),
            discord_uses: invite.discord_uses.to_string(),
            attributed_joins: invite.attributed_joins.to_string(),
            max_uses: invite.max_uses.to_string(),
            max_age: invite.max_age.to_string(),
            temporary: invite.temporary.to_string(),
            expires_at: invite.expires_at.map(format_date).unwrap_or_default(),
            invite_type: invite.invite_type.to_string(),
            flags: invite.flags.to_string(),
            target_type: optional_to_string(invite.target_type),
            target_user_id: invite.target_user_id.clone().unwrap_or_default(),
            target_application_id: invite.target_application_id.clone().unwrap_or_default(),
            scheduled_event_id: invite.scheduled_event_id.clone().unwrap_or_default(),
            targeted_user_count: optional_to_string(invite.targeted_user_count),
            is_vanity: invite.is_vanity.to_string(),
            tracking_enabled: invite.tracking_enabled.to_string(),
            discord_active: invite.discord_active.to_string(),
            status: invite.status.clone(),
            deleted_at: invite.deleted_at.map(format_date).unwrap_or_default(),
            last_synced_at: invite.last_synced_at.map(format_date).unwrap_or_default(),
            tracked_at: format_date(invite.tracked_at),
            native_role_ids: invite.native_role_ids.join(";"),
            managed_role_ids: invite.managed_role_ids.join(";"),
        })?;
    }
    writer
        .into_inner()
        .context("failed to finish the invite CSV export")
}

pub fn write_join_export(joins: &[ExportJoin]) -> anyhow::Result<Vec<u8>> {
    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    for join in joins {
        writer.serialize(JoinCsvRecord {
            user_id: join.user_id.clone(),
            observed_at: format_date(join.observed_at),
            member_joined_at: format_date(join.member_joined_at),
            account_created_at: join.account_created_at.map(format_date).unwrap_or_default(),
            left_at: join.left_at.map(format_date).unwrap_or_default(),
            screening_completed_at: join
                .screening_completed_at
                .map(format_date)
                .unwrap_or_default(),
            invite_code: join.invite_code.clone().unwrap_or_default(),
            primary_source: join.primary_source.clone().unwrap_or_default(),
            secondary_source: join.secondary_source.clone().unwrap_or_default(),
            attribution_status: join.attribution_status.clone(),
            attribution_reason: join.attribution_reason.clone().unwrap_or_default(),
            attribution_confidence: join.attribution_confidence.clone(),
            is_bot: join.is_bot,
            is_system: join.is_system,
            member_flags: join.member_flags,
            pending: join.pending,
        })?;
    }
    writer
        .into_inner()
        .context("failed to finish the join CSV export")
}

fn parse_date(value: &str) -> anyhow::Result<Option<NaiveDateTime>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    if let Ok(value) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
        return Ok(Some(value));
    }
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Ok(Some(value.with_timezone(&Utc).naive_utc()));
    }

    bail!("expected YYYY-MM-DD HH:MM:SS or an RFC 3339 timestamp")
}

fn format_date(value: NaiveDateTime) -> String {
    value.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn optional_to_string<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(String::new, |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{parse_invite_import, write_invite_export};
    use crate::models::ExportInvite;

    #[test]
    fn invite_export_round_trips_quoted_values() {
        let created_at = NaiveDate::from_ymd_opt(2026, 7, 23)
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap();
        let original = ExportInvite {
            invite_code: "summer".to_owned(),
            channel_id: "100".to_owned(),
            channel_type: 0,
            primary_source: "Video, social".to_owned(),
            secondary_source: "\"Launch\" description".to_owned(),
            tracked_by: "123".to_owned(),
            discord_inviter_id: Some("999".to_owned()),
            discord_created_at: Some(created_at),
            discord_uses: 20,
            attributed_joins: 12,
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
            targeted_user_count: Some(100),
            is_vanity: false,
            tracking_enabled: true,
            discord_active: true,
            status: "active".to_owned(),
            deleted_at: None,
            last_synced_at: Some(created_at),
            tracked_at: created_at,
            native_role_ids: vec!["456".to_owned()],
            managed_role_ids: vec!["789".to_owned()],
        };

        let bytes = write_invite_export(std::slice::from_ref(&original)).unwrap();
        let parsed = parse_invite_import(&bytes).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].invite_code, original.invite_code);
        assert_eq!(parsed[0].primary_source, original.primary_source);
        assert_eq!(parsed[0].secondary_source, original.secondary_source);
        assert_eq!(parsed[0].tracked_by, original.tracked_by);
        assert_eq!(parsed[0].tracked_at, Some(original.tracked_at));
        assert_eq!(parsed[0].role_ids, original.managed_role_ids);
    }
}
