use anyhow::Context as _;
use chrono::Utc;
use poise::serenity_prelude as serenity;

use crate::csv_transfer;
use crate::embeds;
use crate::{Context, Error};

/// Export all tracked invites as a CSV file.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn export(ctx: Context<'_>) -> Result<(), Error> {
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

    let invite_count = invites.len();
    let csv = csv_transfer::write_export(&invites)?;
    let filename = format!(
        "invites-{guild_id}-{}.csv",
        Utc::now().format("%Y%m%d-%H%M%S")
    );
    ctx.send(
        poise::CreateReply::default()
            .content(format!("Exported **{invite_count}** tracked invites."))
            .attachment(serenity::CreateAttachment::bytes(csv, filename))
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
