use anyhow::Context as _;
use poise::serenity_prelude as serenity;
use serde_json::json;

use crate::audit::send_log_message;
use crate::embeds;
use crate::{Context, Error};

/// Configure `InviteAnalytics` settings.
#[poise::command(
    slash_command,
    guild_only,
    subcommands("show", "logchannel", "defaultchannel", "maxlinks", "vanity"),
    default_member_permissions = "ADMINISTRATOR"
)]
#[allow(clippy::unused_async)]
pub async fn r#set(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// View the current server settings.
#[poise::command(slash_command, guild_only)]
pub async fn show(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    ctx.data().repository.ensure_guild(&guild_id).await?;
    let config = ctx
        .data()
        .repository
        .guild_config(&guild_id)
        .await?
        .context("guild configuration disappeared after initialization")?;
    let channel = |channel_id: Option<String>| {
        channel_id.map_or_else(|| "Not configured".to_owned(), |id| format!("<#{id}>"))
    };
    let last_sync = config.last_synced_at.map_or_else(
        || "Never".to_owned(),
        |timestamp| format!("<t:{}:R>", timestamp.and_utc().timestamp()),
    );

    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::brand()
                    .title("InviteAnalytics Settings")
                    .field(
                        "Default Invite Channel",
                        channel(config.default_channel_id),
                        true,
                    )
                    .field("Log Channel", channel(config.log_channel_id), true)
                    .field("Maximum Tracked Links", config.max_links.to_string(), true)
                    .field("Track Vanity URL", config.track_vanity.to_string(), true)
                    .field(
                        "Vanity Source",
                        format!(
                            "{} → {}",
                            config.vanity_primary_source, config.vanity_secondary_source
                        ),
                        true,
                    )
                    .field("Last Successful Sync", last_sync, true),
            )
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

/// Set the channel for invite activity logs.
#[poise::command(slash_command, guild_only)]
pub async fn logchannel(
    ctx: Context<'_>,
    #[description = "Channel for invite activity logs"]
    #[channel_types("Text")]
    channel: serenity::GuildChannel,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    ctx.data().repository.ensure_guild(&guild_id).await?;
    ctx.data()
        .repository
        .set_log_channel(&guild_id, &channel.id.to_string())
        .await?;
    ctx.data()
        .repository
        .write_audit_log(
            &guild_id,
            "settings_changed",
            &ctx.author().id.to_string(),
            Some(json!({
                "setting": "logchannel",
                "value": channel.id.to_string(),
            })),
        )
        .await?;

    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Log Channel Set")
                    .description(format!("→ Channel: <#{}>", channel.id)),
            )
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

/// Set the default channel for creating invites.
#[poise::command(slash_command, guild_only)]
pub async fn defaultchannel(
    ctx: Context<'_>,
    #[description = "Default channel for new invites"]
    #[channel_types("Text")]
    channel: serenity::GuildChannel,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    ctx.data().repository.ensure_guild(&guild_id).await?;
    ctx.data()
        .repository
        .set_default_channel(&guild_id, &channel.id.to_string())
        .await?;
    ctx.data()
        .repository
        .write_audit_log(
            &guild_id,
            "settings_changed",
            &ctx.author().id.to_string(),
            Some(json!({
                "setting": "defaultchannel",
                "value": channel.id.to_string(),
            })),
        )
        .await?;

    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Default Channel Set")
                    .description(format!("→ Channel: <#{}>", channel.id)),
            )
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

/// Set the maximum number of tracked invite links.
#[poise::command(slash_command, guild_only)]
pub async fn maxlinks(
    ctx: Context<'_>,
    #[description = "Maximum number of links"]
    #[min = 1]
    #[max = 1000]
    limit: i64,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    let limit = i32::try_from(limit)?;
    ctx.data().repository.ensure_guild(&guild_id).await?;
    ctx.data()
        .repository
        .set_max_links(&guild_id, limit)
        .await?;
    ctx.data()
        .repository
        .write_audit_log(
            &guild_id,
            "settings_changed",
            &ctx.author().id.to_string(),
            Some(json!({
                "setting": "maxlinks",
                "value": limit,
            })),
        )
        .await?;

    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Maximum Links Updated")
                    .description(format!("→ Limit: **{limit}** links")),
            )
            .ephemeral(true),
    )
    .await?;
    send_log_message(
        &ctx.serenity_context().http,
        &ctx.data().repository,
        &guild_id,
        embeds::log(
            "Settings Changed",
            format!(
                "<@{}> set the maximum number of links to **{limit}**",
                ctx.author().id
            ),
        ),
    )
    .await;
    Ok(())
}

/// Configure vanity URL attribution.
#[poise::command(slash_command, guild_only)]
pub async fn vanity(
    ctx: Context<'_>,
    #[description = "Whether vanity URL tracking is enabled"] enabled: bool,
    #[description = "Primary source for vanity URL joins"]
    #[max_length = 100]
    primary_source: Option<String>,
    #[description = "Secondary source for vanity URL joins"]
    #[max_length = 100]
    secondary_source: Option<String>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id_string = guild_id.to_string();
    ctx.defer_ephemeral().await?;
    ctx.data().repository.ensure_guild(&guild_id_string).await?;
    let current = ctx
        .data()
        .repository
        .guild_config(&guild_id_string)
        .await?
        .context("guild configuration disappeared after initialization")?;
    let primary_source = primary_source.unwrap_or(current.vanity_primary_source);
    let secondary_source = secondary_source.unwrap_or(current.vanity_secondary_source);
    ctx.data()
        .repository
        .set_vanity_tracking(
            &guild_id_string,
            enabled,
            &primary_source,
            &secondary_source,
        )
        .await?;
    ctx.data()
        .repository
        .write_audit_log(
            &guild_id_string,
            "settings_changed",
            &ctx.author().id.to_string(),
            Some(json!({
                "setting": "vanity",
                "enabled": enabled,
                "primarySource": primary_source,
                "secondarySource": secondary_source,
            })),
        )
        .await?;

    let sync_note = match crate::invite_sync::synchronize_guild(ctx.data(), guild_id.get()).await {
        Ok(summary) if summary.vanity_tracked => " The current vanity URL is synchronized.",
        Ok(_) => " This server does not currently expose a vanity URL.",
        Err(error) => {
            tracing::warn!(%error, %guild_id, "vanity synchronization failed");
            " The setting was saved, but synchronization currently fails."
        }
    };
    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Vanity Tracking Updated")
                    .description(format!(
                        "**Enabled:** {enabled}\n**Source:** {primary_source} → \
                         {secondary_source}.{sync_note}"
                    )),
            )
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
