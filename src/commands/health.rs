use anyhow::Context as _;
use poise::serenity_prelude as serenity;

use crate::embeds;
use crate::{Context, Error};

/// Check storage, Discord API access, synchronization, and bot permissions.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn health(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    ctx.defer_ephemeral().await?;

    let database = check(ctx.data().repository.ping().await.map_err(Into::into));
    let redis = check(ctx.data().cache.ping().await);
    let (invite_api, invite_channel_ids) =
        match ctx.data().discord_api.guild_invites(guild_id.get()).await {
            Ok(invites) => (
                check(Ok(())),
                invites
                    .into_iter()
                    .map(|invite| invite.channel.id)
                    .collect::<Vec<_>>(),
            ),
            Err(error) => (check(Err(error)), Vec::new()),
        };
    let vanity_api = check(
        ctx.data()
            .discord_api
            .guild_vanity(guild_id.get())
            .await
            .map(|_| ()),
    );
    let config = ctx
        .data()
        .repository
        .guild_config(&guild_id.to_string())
        .await?;
    let last_sync = config.and_then(|config| config.last_synced_at).map_or_else(
        || "Never".to_owned(),
        |timestamp| format!("<t:{}:R>", timestamp.and_utc().timestamp()),
    );
    let permissions = bot_permissions(ctx);
    let (event_coverage, event_coverage_ready) = invite_event_coverage(ctx, &invite_channel_ids);
    let healthy = database.starts_with('✅')
        && redis.starts_with('✅')
        && invite_api.starts_with('✅')
        && permissions.manage_guild
        && event_coverage_ready;
    let embed = if healthy {
        embeds::success()
    } else {
        embeds::error("One or more service checks failed.")
    }
    .title(if healthy {
        "InviteAnalytics Is Healthy"
    } else {
        "InviteAnalytics Needs Attention"
    })
    .field("PostgreSQL", database, true)
    .field("Redis", redis, true)
    .field("Guild Invites API", invite_api, true)
    .field("Vanity URL API", vanity_api, true)
    .field("Last Successful Sync", last_sync, true)
    .field(
        "Gateway Members Intent",
        "✅ Connected and receiving commands",
        true,
    )
    .field("Invite Event Coverage", event_coverage, false)
    .field("Bot Permissions", permissions.summary, false);

    ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}

struct PermissionReport {
    summary: String,
    manage_guild: bool,
}

fn bot_permissions(ctx: Context<'_>) -> PermissionReport {
    let bot_id = ctx.serenity_context().cache.current_user().id;
    let Some(guild) = ctx.guild() else {
        return PermissionReport {
            summary: "⚠️ Guild is not available in the local cache".to_owned(),
            manage_guild: false,
        };
    };
    let Some(member) = guild.members.get(&bot_id) else {
        return PermissionReport {
            summary: "⚠️ Bot member is not available in the local cache".to_owned(),
            manage_guild: false,
        };
    };
    let permissions = guild.member_permissions(member);
    let summary = [
        (
            "Create Invite",
            permissions.contains(serenity::Permissions::CREATE_INSTANT_INVITE),
        ),
        (
            "Manage Guild",
            permissions.contains(serenity::Permissions::MANAGE_GUILD),
        ),
        (
            "Manage Channels",
            permissions.contains(serenity::Permissions::MANAGE_CHANNELS),
        ),
        (
            "Manage Roles",
            permissions.contains(serenity::Permissions::MANAGE_ROLES),
        ),
        (
            "View Audit Log",
            permissions.contains(serenity::Permissions::VIEW_AUDIT_LOG),
        ),
    ]
    .into_iter()
    .map(|(name, granted)| format!("{} {name}", if granted { "✅" } else { "❌" }))
    .collect::<Vec<_>>()
    .join("\n");
    PermissionReport {
        summary,
        manage_guild: permissions.contains(serenity::Permissions::MANAGE_GUILD),
    }
}

fn invite_event_coverage(ctx: Context<'_>, channel_ids: &[String]) -> (String, bool) {
    let bot_id = ctx.serenity_context().cache.current_user().id;
    let Some(guild) = ctx.guild() else {
        return (
            "⚠️ Guild is not available in the local cache".to_owned(),
            false,
        );
    };
    let Some(member) = guild.members.get(&bot_id) else {
        return (
            "⚠️ Bot member is not available in the local cache".to_owned(),
            false,
        );
    };
    if channel_ids.is_empty() {
        let ready = guild
            .member_permissions(member)
            .contains(serenity::Permissions::MANAGE_CHANNELS);
        return (
            if ready {
                "✅ Manage Channels is available for future invites"
            } else {
                "❌ Manage Channels is required for invite create/delete events"
            }
            .to_owned(),
            ready,
        );
    }

    let channel_ids = channel_ids.iter().collect::<std::collections::HashSet<_>>();
    let missing = channel_ids
        .iter()
        .filter(|channel_id| {
            let Some(channel_id) = channel_id.parse::<u64>().ok().map(serenity::ChannelId::new)
            else {
                return true;
            };
            guild.channels.get(&channel_id).is_none_or(|channel| {
                !guild
                    .user_permissions_in(channel, member)
                    .contains(serenity::Permissions::MANAGE_CHANNELS)
            })
        })
        .count();
    let total = channel_ids.len();
    if missing == 0 {
        (
            format!("✅ Manage Channels is available in all {total} active invite channels"),
            true,
        )
    } else {
        (
            format!("❌ Manage Channels is missing in {missing} of {total} active invite channels"),
            false,
        )
    }
}

fn check(result: anyhow::Result<()>) -> String {
    result.map_or_else(
        |error| format!("❌ {error}"),
        |()| "✅ Available".to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::check;

    #[test]
    fn health_checks_render_success_and_failure() {
        assert!(check(Ok(())).starts_with('✅'));
        assert!(check(Err(anyhow::anyhow!("offline"))).starts_with('❌'));
    }
}
