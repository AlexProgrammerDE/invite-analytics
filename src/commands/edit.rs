use anyhow::Context as _;
use poise::serenity_prelude as serenity;
use serde_json::json;

use crate::audit::send_log_message;
use crate::commands::support::unique_role_ids;
use crate::embeds;
use crate::invite_tracking::normalize_invite_code;
use crate::{Context, Error};

/// Edit source labels or managed fallback roles.
#[poise::command(
    slash_command,
    guild_only,
    subcommands("sources", "roles"),
    default_member_permissions = "ADMINISTRATOR"
)]
#[allow(clippy::unused_async)]
pub async fn edit(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Change an invite's source labels without rewriting historical joins.
#[poise::command(slash_command, guild_only)]
pub async fn sources(
    ctx: Context<'_>,
    #[description = "Invite code or URL"] code: String,
    #[description = "New primary source"]
    #[max_length = 100]
    primary_source: String,
    #[description = "New secondary source"]
    #[max_length = 100]
    secondary_source: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    let code = normalize_invite_code(&code);
    let Some(invite) = ctx.data().repository.find_invite(&guild_id, &code).await? else {
        send_not_found(ctx, &code).await?;
        return Ok(());
    };

    ctx.data()
        .repository
        .update_invite_sources(invite.id, &primary_source, &secondary_source)
        .await?;
    ctx.data()
        .repository
        .write_audit_log(
            &guild_id,
            "invite_sources_changed",
            &ctx.author().id.to_string(),
            Some(json!({
                "inviteCode": code,
                "oldPrimarySource": invite.primary_source,
                "oldSecondarySource": invite.secondary_source,
                "primarySource": primary_source,
                "secondarySource": secondary_source,
            })),
        )
        .await?;
    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Invite Sources Updated")
                    .description(format!("`{code}` now uses **{primary_source} → {secondary_source}**.\nHistorical joins keep their original labels.")),
            )
            .ephemeral(true),
    )
    .await?;
    send_log_message(
        &ctx.serenity_context().http,
        &ctx.data().repository,
        &guild_id,
        embeds::log(
            "Invite Sources Updated",
            format!(
                "<@{}> changed `{code}` to **{primary_source} → {secondary_source}**",
                ctx.author().id
            ),
        ),
    )
    .await;
    Ok(())
}

/// Replace managed fallback roles for future joins.
#[poise::command(slash_command, guild_only)]
pub async fn roles(
    ctx: Context<'_>,
    #[description = "Invite code or URL"] code: String,
    #[description = "First managed role"] role: Option<serenity::Role>,
    #[description = "Second managed role"] role_2: Option<serenity::Role>,
    #[description = "Third managed role"] role_3: Option<serenity::Role>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    let code = normalize_invite_code(&code);
    let Some(invite) = ctx.data().repository.find_invite(&guild_id, &code).await? else {
        send_not_found(ctx, &code).await?;
        return Ok(());
    };
    let role_ids = unique_role_ids(&[role, role_2, role_3]);
    ctx.data()
        .repository
        .replace_managed_roles(invite.id, &role_ids)
        .await?;
    ctx.data()
        .repository
        .write_audit_log(
            &guild_id,
            "invite_managed_roles_changed",
            &ctx.author().id.to_string(),
            Some(json!({ "inviteCode": code, "roleIds": role_ids })),
        )
        .await?;

    let roles = if role_ids.is_empty() {
        "No managed fallback roles are configured.".to_owned()
    } else {
        format!(
            "Managed fallback roles: {}",
            role_ids
                .iter()
                .map(|role_id| format!("<@&{role_id}>"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Managed Roles Updated")
                    .description(format!(
                        "{roles}\nNative roles embedded in the Discord invite are unchanged."
                    )),
            )
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
