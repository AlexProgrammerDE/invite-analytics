use anyhow::Context as _;

use crate::embeds;
use crate::invite_sync::synchronize_guild;
use crate::{Context, Error};

/// Reconcile tracked invite lifecycle and Discord counters now.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn sync(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    ctx.defer_ephemeral().await?;
    let summary = synchronize_guild(ctx.data(), guild_id.get()).await?;
    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Invite Synchronization Complete")
                    .field(
                        "Active Discord Invites",
                        summary.active_invites.to_string(),
                        true,
                    )
                    .field(
                        "Tracked Metadata Refreshed",
                        summary.tracked_invites_refreshed.to_string(),
                        true,
                    )
                    .field(
                        "Missing Invites Closed",
                        summary.missing_invites_closed.to_string(),
                        true,
                    )
                    .field(
                        "Vanity URL Tracked",
                        summary.vanity_tracked.to_string(),
                        true,
                    ),
            )
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
