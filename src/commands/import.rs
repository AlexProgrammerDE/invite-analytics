use std::collections::HashMap;

use anyhow::Context as _;
use poise::serenity_prelude as serenity;
use serde_json::json;

use crate::audit::send_log_message;
use crate::csv_transfer;
use crate::discord_api::DiscordInvite;
use crate::embeds;
use crate::invite_tracking::normalize_invite_code;
use crate::models::{NewTrackedInvite, RoleAssignmentMode};
use crate::{Context, Error};

const MAX_CSV_BYTES: u32 = 2 * 1024 * 1024;
const MAX_SKIP_DETAILS: usize = 10;

/// Import existing Discord invites.
#[poise::command(
    slash_command,
    guild_only,
    subcommands("single", "csv"),
    default_member_permissions = "ADMINISTRATOR"
)]
#[allow(clippy::unused_async)]
pub async fn import(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Import one existing Discord invite with its current Discord metadata.
#[poise::command(slash_command, guild_only)]
#[allow(clippy::too_many_lines)]
pub async fn single(
    ctx: Context<'_>,
    #[description = "Existing invite code or URL"] code: String,
    #[description = "Primary source"]
    #[max_length = 100]
    primary_source: String,
    #[description = "Secondary source"]
    #[max_length = 100]
    secondary_source: String,
    #[description = "Optional managed role for future joins"] role: Option<serenity::Role>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id_string = guild_id.to_string();
    ctx.defer_ephemeral().await?;
    let code = normalize_invite_code(&code);
    ctx.data().repository.ensure_guild(&guild_id_string).await?;
    let config = ctx
        .data()
        .repository
        .guild_config(&guild_id_string)
        .await?
        .context("guild configuration disappeared after initialization")?;
    let current_count = ctx
        .data()
        .repository
        .count_invites(&guild_id_string)
        .await?;
    if current_count >= i64::from(config.max_links) {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(format!(
                    "You have reached the limit of **{}** tracked links.",
                    config.max_links
                )))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }
    if ctx.data().repository.invite_exists(&code).await? {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(format!(
                    "Invite `{code}` is already being tracked."
                )))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let Some(discord_invite) = ctx
        .data()
        .discord_api
        .guild_invites(guild_id.get())
        .await?
        .into_iter()
        .find(|invite| invite.code == code)
    else {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(format!(
                    "Discord does not list an active invite with code `{code}` in this server."
                )))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };

    let tracked = new_tracked_invite(
        &guild_id_string,
        &primary_source,
        &secondary_source,
        &ctx.author().id.to_string(),
        &discord_invite,
        None,
    );
    let inserted = ctx.data().repository.insert_invite(&tracked).await?;
    if let Some(role) = role {
        ctx.data()
            .repository
            .replace_managed_roles(inserted.id, &[role.id.to_string()])
            .await?;
    }
    ctx.data()
        .cache
        .set_invite(
            &guild_id_string,
            &code,
            u64::try_from(discord_invite.uses).unwrap_or_default(),
        )
        .await?;
    ctx.data()
        .repository
        .write_audit_log(
            &guild_id_string,
            "invite_imported",
            &ctx.author().id.to_string(),
            Some(json!({
                "inviteCode": code,
                "primarySource": primary_source,
                "secondarySource": secondary_source,
                "discordUses": discord_invite.uses,
            })),
        )
        .await?;

    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::success()
                    .title("Invite Imported")
                    .description(format!(
                        "Now tracking `https://discord.gg/{code}`.\n\
                         **{primary_source}** → **{secondary_source}**"
                    )),
            )
            .ephemeral(true),
    )
    .await?;
    send_log_message(
        &ctx.serenity_context().http,
        &ctx.data().repository,
        &guild_id_string,
        embeds::log(
            "Invite Imported",
            format!(
                "<@{}> imported `{code}`\n**{primary_source}** → **{secondary_source}**",
                ctx.author().id
            ),
        ),
    )
    .await;
    Ok(())
}

/// Import active Discord invites from an `InviteAnalytics` CSV export.
#[poise::command(slash_command, guild_only)]
#[allow(clippy::too_many_lines)]
pub async fn csv(
    ctx: Context<'_>,
    #[description = "CSV file to import"]
    #[rename = "file"]
    attachment: serenity::Attachment,
) -> Result<(), Error> {
    if !attachment.filename.to_ascii_lowercase().ends_with(".csv") {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(
                    "The uploaded file must use the `.csv` extension.",
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }
    if attachment.size > MAX_CSV_BYTES {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error("CSV files must be 2 MiB or smaller."))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id_string = guild_id.to_string();
    ctx.defer_ephemeral().await?;
    let bytes = ctx
        .data()
        .attachment_client
        .get(&attachment.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if bytes.len() > usize::try_from(MAX_CSV_BYTES)? {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error("CSV files must be 2 MiB or smaller."))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let rows = csv_transfer::parse_invite_import(&bytes)?;
    if rows.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(
                    "The CSV file does not contain any invite rows.",
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    ctx.data().repository.ensure_guild(&guild_id_string).await?;
    let config = ctx
        .data()
        .repository
        .guild_config(&guild_id_string)
        .await?
        .context("guild configuration disappeared after initialization")?;
    let mut tracked_count = ctx
        .data()
        .repository
        .count_invites(&guild_id_string)
        .await?;
    let discord_invites = ctx
        .data()
        .discord_api
        .guild_invites(guild_id.get())
        .await?
        .into_iter()
        .map(|invite| (invite.code.clone(), invite))
        .collect::<HashMap<_, _>>();

    let total = rows.len();
    let mut imported = 0_usize;
    let mut skipped = 0_usize;
    let mut errors = 0_usize;
    let mut skip_details = Vec::new();

    for row in rows {
        if tracked_count >= i64::from(config.max_links) {
            skipped += 1;
            push_skip_detail(
                &mut skip_details,
                format!("`{}`: link limit reached", row.invite_code),
            );
            continue;
        }
        if ctx
            .data()
            .repository
            .invite_exists(&row.invite_code)
            .await?
        {
            skipped += 1;
            push_skip_detail(
                &mut skip_details,
                format!("`{}`: already tracked", row.invite_code),
            );
            continue;
        }
        let Some(discord_invite) = discord_invites.get(&row.invite_code) else {
            skipped += 1;
            push_skip_detail(
                &mut skip_details,
                format!("`{}`: not active in this server", row.invite_code),
            );
            continue;
        };

        let new_invite = new_tracked_invite(
            &guild_id_string,
            &row.primary_source,
            &row.secondary_source,
            &row.tracked_by,
            discord_invite,
            row.tracked_at,
        );
        match ctx.data().repository.insert_invite(&new_invite).await {
            Ok(inserted) => {
                if !row.role_ids.is_empty()
                    && let Err(error) = ctx
                        .data()
                        .repository
                        .replace_managed_roles(inserted.id, &row.role_ids)
                        .await
                {
                    tracing::warn!(
                        %error,
                        code = %row.invite_code,
                        "failed to restore managed invite roles"
                    );
                }
                ctx.data()
                    .cache
                    .set_invite(
                        &guild_id_string,
                        &new_invite.invite_code,
                        u64::try_from(discord_invite.uses).unwrap_or_default(),
                    )
                    .await?;
                imported += 1;
                tracked_count += 1;
            }
            Err(error) => {
                errors += 1;
                tracing::warn!(
                    %error,
                    code = %new_invite.invite_code,
                    "failed to import an invite"
                );
            }
        }
    }

    ctx.data()
        .repository
        .write_audit_log(
            &guild_id_string,
            "bulk_import",
            &ctx.author().id.to_string(),
            Some(json!({
                "total": total,
                "imported": imported,
                "skipped": skipped,
                "errors": errors,
            })),
        )
        .await?;

    let mut embed = embeds::success()
        .title("CSV Import Complete")
        .field("Rows", total.to_string(), true)
        .field("Imported", imported.to_string(), true)
        .field("Skipped", skipped.to_string(), true)
        .field("Errors", errors.to_string(), true);
    if !skip_details.is_empty() {
        embed = embed.field("Skipped Rows", skip_details.join("\n"), false);
    }
    ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}

fn new_tracked_invite(
    guild_id: &str,
    primary_source: &str,
    secondary_source: &str,
    tracked_by: &str,
    invite: &DiscordInvite,
    tracked_at: Option<chrono::NaiveDateTime>,
) -> NewTrackedInvite {
    NewTrackedInvite {
        guild_id: guild_id.to_owned(),
        invite_code: invite.code.clone(),
        channel_id: invite.channel.id.clone(),
        channel_type: invite.channel.kind,
        primary_source: primary_source.to_owned(),
        secondary_source: secondary_source.to_owned(),
        tracked_by: tracked_by.to_owned(),
        discord_inviter_id: invite.inviter.as_ref().map(|user| user.id.clone()),
        discord_created_at: Some(invite.created_at.naive_utc()),
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
        targeted_user_count: None,
        is_vanity: false,
        tracked_at,
        role_ids: invite.roles.iter().map(|role| role.id.clone()).collect(),
        role_assignment_mode: RoleAssignmentMode::Native,
    }
}

fn push_skip_detail(details: &mut Vec<String>, value: String) {
    if details.len() < MAX_SKIP_DETAILS {
        details.push(value);
    }
}
