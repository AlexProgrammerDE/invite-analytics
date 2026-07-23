use anyhow::Context as _;
use serde_json::json;

use crate::audit::send_log_message;
use crate::embeds;
use crate::invite_tracking::normalize_invite_code;
use crate::{Context, Error};

/// Stop tracking an invite and optionally revoke it on Discord.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn delete(
    ctx: Context<'_>,
    #[description = "Invite code or URL"] code: String,
    #[description = "Also revoke the invite on Discord"] revoke: Option<bool>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    ctx.defer_ephemeral().await?;
    let code = normalize_invite_code(&code);
    let Some(invite) = ctx.data().repository.find_invite(&guild_id, &code).await? else {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(format!(
                    "No tracked invite was found with code `{code}`."
                )))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };

    let revoke = revoke.unwrap_or(true);
    let revoked = if revoke {
        match ctx.data().discord_api.delete_invite(&code).await {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(
                    %error,
                    %code,
                    "removed invite tracking but could not revoke the Discord invite"
                );
                false
            }
        }
    } else {
        false
    };
    ctx.data()
        .repository
        .stop_tracking(invite.id, revoked)
        .await?;
    if revoked {
        ctx.data().cache.remove_invite(&guild_id, &code).await?;
    }

    ctx.data()
        .repository
        .write_audit_log(
            &guild_id,
            "invite_deleted",
            &ctx.author().id.to_string(),
            Some(json!({
                "inviteCode": code,
                "primarySource": invite.primary_source,
                "secondarySource": invite.secondary_source,
                "revokeRequested": revoke,
                "revoked": revoked,
            })),
        )
        .await?;

    let revoke_status = match (revoke, revoked) {
        (true, true) => "The Discord invite was also revoked.",
        (true, false) => {
            "Tracking was removed, but Discord did not allow the invite to be revoked."
        }
        (false, _) => "The Discord invite remains active.",
    };
    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Invite Deleted")
                    .description(format!(
                        "Stopped tracking `{code}` without deleting its analytics history.\n\
                         {revoke_status}"
                    )),
            )
            .ephemeral(true),
    )
    .await?;

    send_log_message(
        &ctx.serenity_context().http,
        &ctx.data().repository,
        &guild_id,
        embeds::log(
            "Invite Deleted",
            format!(
                "<@{}> stopped tracking `{code}`\n**{}** → **{}**",
                ctx.author().id,
                invite.primary_source,
                invite.secondary_source
            ),
        ),
    )
    .await;

    Ok(())
}
