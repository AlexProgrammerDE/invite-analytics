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
    subcommands("logchannel", "defaultchannel", "maxlinks"),
    default_member_permissions = "ADMINISTRATOR"
)]
#[allow(clippy::unused_async)]
pub async fn r#set(_ctx: Context<'_>) -> Result<(), Error> {
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
