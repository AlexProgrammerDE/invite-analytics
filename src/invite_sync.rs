use std::collections::HashMap;
use std::time::Duration;

use crate::discord_api::DiscordInvite;
use crate::models::InviteSync;
use crate::state::BotData;

const RECONCILIATION_INTERVAL: Duration = Duration::from_mins(15);

#[derive(Debug)]
pub struct DiscordSnapshot {
    pub counts: HashMap<String, u64>,
    pub regular_invites: Vec<DiscordInvite>,
    pub vanity_code: Option<String>,
    pub vanity_fetch_succeeded: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct SyncSummary {
    pub active_invites: usize,
    pub tracked_invites_refreshed: usize,
    pub missing_invites_closed: u64,
    pub vanity_tracked: bool,
}

pub async fn fetch_discord_snapshot(
    data: &BotData,
    guild_id: u64,
) -> anyhow::Result<DiscordSnapshot> {
    let regular_invites = data.discord_api.guild_invites(guild_id).await?;
    let mut counts = regular_invites
        .iter()
        .filter_map(|invite| {
            u64::try_from(invite.uses)
                .ok()
                .map(|uses| (invite.code.clone(), uses))
        })
        .collect::<HashMap<_, _>>();

    let vanity = match data.discord_api.guild_vanity(guild_id).await {
        Ok(vanity) => vanity,
        Err(error) => {
            tracing::debug!(%error, %guild_id, "vanity invite is unavailable");
            return Ok(DiscordSnapshot {
                counts,
                regular_invites,
                vanity_code: None,
                vanity_fetch_succeeded: false,
            });
        }
    };
    if let Some(code) = vanity.code.as_ref()
        && let Ok(uses) = u64::try_from(vanity.uses)
    {
        counts.insert(code.clone(), uses);
    }

    Ok(DiscordSnapshot {
        counts,
        regular_invites,
        vanity_code: vanity.code,
        vanity_fetch_succeeded: true,
    })
}

pub async fn synchronize_guild(data: &BotData, guild_id: u64) -> anyhow::Result<SyncSummary> {
    let guild_id_string = guild_id.to_string();
    data.repository.ensure_guild(&guild_id_string).await?;
    let snapshot = fetch_discord_snapshot(data, guild_id).await?;
    let mut refreshed = 0_usize;

    for invite in &snapshot.regular_invites {
        if data
            .repository
            .sync_invite(&guild_id_string, &to_invite_sync(invite))
            .await?
        {
            refreshed += 1;
        }
    }

    let config = data
        .repository
        .guild_config(&guild_id_string)
        .await?
        .ok_or_else(|| anyhow::anyhow!("guild configuration disappeared during synchronization"))?;
    let vanity_tracked = if config.track_vanity {
        if let Some(code) = snapshot.vanity_code.as_ref() {
            let uses = snapshot
                .counts
                .get(code)
                .copied()
                .and_then(|value| i64::try_from(value).ok())
                .unwrap_or_default();
            data.repository
                .upsert_vanity_invite(
                    &guild_id_string,
                    code,
                    uses,
                    &config.vanity_primary_source,
                    &config.vanity_secondary_source,
                )
                .await?;
            true
        } else {
            false
        }
    } else {
        false
    };

    let active_codes = snapshot
        .regular_invites
        .iter()
        .map(|invite| invite.code.clone())
        .collect::<Vec<_>>();
    let missing_invites_closed = data
        .repository
        .mark_missing_invites(&guild_id_string, &active_codes)
        .await?;
    data.cache
        .replace_invite_snapshot(&guild_id_string, &snapshot.counts)
        .await?;
    data.repository.mark_guild_synced(&guild_id_string).await?;

    Ok(SyncSummary {
        active_invites: snapshot.counts.len(),
        tracked_invites_refreshed: refreshed,
        missing_invites_closed,
        vanity_tracked,
    })
}

pub async fn refresh_tracked_metadata(
    data: &BotData,
    guild_id: u64,
    snapshot: &DiscordSnapshot,
) -> anyhow::Result<()> {
    let guild_id_string = guild_id.to_string();
    for invite in &snapshot.regular_invites {
        data.repository
            .sync_invite(&guild_id_string, &to_invite_sync(invite))
            .await?;
    }

    let config = data.repository.guild_config(&guild_id_string).await?;
    if let Some(config) = config
        && config.track_vanity
        && let Some(code) = snapshot.vanity_code.as_ref()
    {
        let uses = snapshot
            .counts
            .get(code)
            .copied()
            .and_then(|value| i64::try_from(value).ok())
            .unwrap_or_default();
        data.repository
            .upsert_vanity_invite(
                &guild_id_string,
                code,
                uses,
                &config.vanity_primary_source,
                &config.vanity_secondary_source,
            )
            .await?;
    }
    Ok(())
}

pub fn start_reconciliation_loop(data: BotData) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(RECONCILIATION_INTERVAL);
        interval.tick().await;
        loop {
            interval.tick().await;
            let guild_ids = match data.repository.guild_ids().await {
                Ok(guild_ids) => guild_ids,
                Err(error) => {
                    tracing::error!(%error, "failed to list guilds for invite reconciliation");
                    continue;
                }
            };

            for guild_id in guild_ids {
                let Ok(guild_id) = guild_id.parse::<u64>() else {
                    tracing::warn!(%guild_id, "stored guild ID is invalid");
                    continue;
                };
                if let Err(error) = synchronize_guild(&data, guild_id).await {
                    tracing::warn!(%error, %guild_id, "periodic invite reconciliation failed");
                }
            }
        }
    });
}

fn to_invite_sync(invite: &DiscordInvite) -> InviteSync {
    InviteSync {
        invite_code: invite.code.clone(),
        channel_id: invite.channel.id.clone(),
        channel_type: invite.channel.kind,
        discord_inviter_id: invite.inviter.as_ref().map(|user| user.id.clone()),
        discord_created_at: invite.created_at.naive_utc(),
        discord_uses: invite.uses,
        max_uses: invite.max_uses,
        max_age: invite.max_age,
        temporary: invite.temporary,
        expires_at: invite.expires_at.map(|value| value.naive_utc()),
        invite_type: invite.invite_type,
        flags: invite.flags,
        target_type: invite.target_type,
        target_user_id: invite.target_user.as_ref().map(|user| user.id.clone()),
        target_application_id: invite
            .target_application
            .as_ref()
            .map(|application| application.id.clone()),
        scheduled_event_id: invite
            .guild_scheduled_event
            .as_ref()
            .map(|event| event.id.clone()),
        role_ids: invite.roles.iter().map(|role| role.id.clone()).collect(),
    }
}
