use anyhow::Context as _;
use poise::serenity_prelude as serenity;
use serde_json::json;

use crate::commands::support::download_target_users;
use crate::embeds;
use crate::invite_tracking::normalize_invite_code;
use crate::{Context, Error};

/// Manage the users allowed to accept a targeted invite.
#[poise::command(
    slash_command,
    guild_only,
    subcommands("update", "status", "export"),
    default_member_permissions = "ADMINISTRATOR"
)]
#[allow(clippy::unused_async)]
pub async fn target_users(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Replace an invite's target-user allowlist.
#[poise::command(slash_command, guild_only)]
pub async fn update(
    ctx: Context<'_>,
    #[description = "Invite code or URL"] code: String,
    #[description = "CSV containing one Discord user ID per line"]
    #[rename = "file"]
    attachment: serenity::Attachment,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
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

    let (file, user_count) = download_target_users(ctx, &attachment).await?;
    ctx.data()
        .discord_api
        .update_target_users(&code, file)
        .await?;
    ctx.data()
        .repository
        .update_targeted_user_count(invite.id, user_count)
        .await?;
    ctx.data()
        .repository
        .write_audit_log(
            &guild_id,
            "invite_target_users_changed",
            &ctx.author().id.to_string(),
            Some(json!({ "inviteCode": code, "targetedUserCount": user_count })),
        )
        .await?;
    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Target Users Submitted")
                    .description(format!(
                        "Discord is processing **{user_count}** allowed users for `{code}`. \
                         Use `/target_users status` to check progress."
                    )),
            )
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

/// Check Discord's target-user processing job.
#[poise::command(slash_command, guild_only)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "Invite code or URL"] code: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    let code = normalize_invite_code(&code);
    let Some(invite) = ctx.data().repository.find_invite(&guild_id, &code).await? else {
        send_not_found(ctx, &code).await?;
        return Ok(());
    };
    let status = ctx.data().discord_api.target_users_status(&code).await?;
    if let Ok(total_users) = i32::try_from(status.total_users) {
        ctx.data()
            .repository
            .update_targeted_user_count(invite.id, total_users)
            .await?;
    }
    let status_name = match status.status {
        1 => "Processing",
        2 => "Completed",
        3 => "Failed",
        _ => "Unspecified",
    };
    let mut embed = embeds::brand()
        .title("Target User Job")
        .field("Invite", format!("`{code}`"), true)
        .field("Status", status_name, true)
        .field("Total Users", status.total_users.to_string(), true)
        .field("Processed", status.processed_users.to_string(), true)
        .field(
            "Created",
            format!("<t:{}:R>", status.created_at.timestamp()),
            true,
        );
    if let Some(completed_at) = status.completed_at {
        embed = embed.field(
            "Completed",
            format!("<t:{}:R>", completed_at.timestamp()),
            true,
        );
    }
    if let Some(error) = status.error_message {
        embed = embed.field("Error", error, false);
    }
    ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}

/// Export the users allowed to accept a targeted invite.
#[poise::command(slash_command, guild_only)]
pub async fn export(
    ctx: Context<'_>,
    #[description = "Invite code or URL"] code: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    let code = normalize_invite_code(&code);
    if ctx
        .data()
        .repository
        .find_invite(&guild_id, &code)
        .await?
        .is_none()
    {
        send_not_found(ctx, &code).await?;
        return Ok(());
    }

    let csv = ctx.data().discord_api.target_users_csv(&code).await?;
    ctx.send(
        poise::CreateReply::default()
            .content(format!("Exported the target-user allowlist for `{code}`."))
            .attachment(serenity::CreateAttachment::bytes(
                csv,
                format!("target-users-{code}.csv"),
            ))
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

async fn send_not_found(ctx: Context<'_>, code: &str) -> Result<(), Error> {
    ctx.send(
        poise::CreateReply::default()
            .embed(embeds::error(format!(
                "No tracked invite was found with code `{code}`."
            )))
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
