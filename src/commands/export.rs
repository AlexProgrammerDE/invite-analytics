use anyhow::Context as _;
use chrono::Utc;
use poise::serenity_prelude as serenity;

use crate::csv_transfer;
use crate::embeds;
use crate::{Context, Error};

/// Export invite configuration or raw membership sessions.
#[poise::command(
    slash_command,
    guild_only,
    subcommands("invites", "joins"),
    default_member_permissions = "ADMINISTRATOR"
)]
#[allow(clippy::unused_async)]
pub async fn export(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Export all tracked invite metadata as CSV.
#[poise::command(slash_command, guild_only)]
pub async fn invites(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    ctx.defer_ephemeral().await?;

    let invites = ctx.data().repository.export_invites(&guild_id).await?;
    if invites.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error("No tracked invites are available to export."))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let count = invites.len();
    let csv = csv_transfer::write_invite_export(&invites)?;
    send_export(ctx, &guild_id, "invites", count, csv).await
}

/// Export attributed and unattributed membership sessions as CSV.
#[poise::command(slash_command, guild_only)]
pub async fn joins(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    ctx.defer_ephemeral().await?;

    let joins = ctx.data().repository.export_joins(&guild_id).await?;
    if joins.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(
                    "No membership sessions are available to export.",
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let count = joins.len();
    let csv = csv_transfer::write_join_export(&joins)?;
    send_export(ctx, &guild_id, "joins", count, csv).await
}

async fn send_export(
    ctx: Context<'_>,
    guild_id: &str,
    kind: &str,
    count: usize,
    bytes: Vec<u8>,
) -> Result<(), Error> {
    let filename = format!(
        "{kind}-{guild_id}-{}.csv",
        Utc::now().format("%Y%m%d-%H%M%S")
    );
    ctx.send(
        poise::CreateReply::default()
            .content(format!("Exported **{count}** {kind}."))
            .attachment(serenity::CreateAttachment::bytes(bytes, filename))
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
