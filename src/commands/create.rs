use anyhow::{Context as _, bail};
use poise::serenity_prelude as serenity;
use serde_json::json;

use crate::audit::send_log_message;
use crate::commands::support::{download_target_users, unique_role_ids};
use crate::discord_api::CreateDiscordInvite;
use crate::embeds;
use crate::models::{NewTrackedInvite, RoleAssignmentMode};
use crate::{Context, Error};

#[derive(Clone, Copy, Debug, poise::ChoiceParameter)]
pub enum InviteTarget {
    #[name = "Voice stream"]
    Stream,
    #[name = "Embedded application"]
    EmbeddedApplication,
}

impl InviteTarget {
    const fn api_value(self) -> u8 {
        match self {
            Self::Stream => 1,
            Self::EmbeddedApplication => 2,
        }
    }
}

/// Create a tracked invite, optionally with native roles and targeted users.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "Primary source, such as Instagram, YouTube, or Discord"]
    #[max_length = 100]
    primary_source: String,
    #[description = "Secondary source, such as Bio or Help Embed"]
    #[max_length = 100]
    secondary_source: String,
    #[description = "Channel for the invite; defaults to the configured channel"] channel: Option<
        serenity::GuildChannel,
    >,
    #[description = "First role Discord assigns when the invite is accepted"] role: Option<
        serenity::Role,
    >,
    #[description = "Second role Discord assigns when the invite is accepted"] role_2: Option<
        serenity::Role,
    >,
    #[description = "Third role Discord assigns when the invite is accepted"] role_3: Option<
        serenity::Role,
    >,
    #[description = "Hours until expiry, or 0 for no expiry"]
    #[min = 0]
    #[max = 168]
    max_age_hours: Option<i64>,
    #[description = "Maximum uses, or 0 for unlimited"]
    #[min = 0]
    #[max = 100]
    max_uses: Option<i64>,
    #[description = "Grant temporary membership"] temporary: Option<bool>,
    #[description = "Optional voice invite target"] target: Option<InviteTarget>,
    #[description = "Streaming user, required for a voice stream target"] target_user: Option<
        serenity::User,
    >,
    #[description = "Application ID, required for an embedded application target"]
    target_application_id: Option<String>,
    #[description = "CSV of Discord user IDs allowed to accept this invite"] target_users: Option<
        serenity::Attachment,
    >,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id_string = guild_id.to_string();
    ctx.defer_ephemeral().await?;
    ctx.data().repository.ensure_guild(&guild_id_string).await?;
    let config = ctx
        .data()
        .repository
        .guild_config(&guild_id_string)
        .await?
        .context("guild configuration disappeared after initialization")?;

    let channel_id = if let Some(channel) = channel {
        channel.id
    } else if let Some(value) = config.default_channel_id {
        serenity::ChannelId::new(value.parse()?)
    } else {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(
                    "Choose a channel or configure one with `/set defaultchannel`.",
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };

    let current_count = ctx
        .data()
        .repository
        .count_invites(&guild_id_string)
        .await?;
    if current_count >= i64::from(config.max_links) {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(format!(
                    "You have reached the limit of **{}** tracked links. \
                     Increase it with `/set maxlinks` or stop tracking an unused link.",
                    config.max_links
                )))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    validate_target(
        target,
        target_user.as_ref(),
        target_application_id.as_deref(),
    )?;
    if let Some(application_id) = target_application_id.as_deref() {
        application_id
            .parse::<u64>()
            .context("target application ID must be a Discord snowflake")?;
    }

    let role_ids = unique_role_ids(&[role, role_2, role_3]);
    let target_user_id = target_user.as_ref().map(|user| user.id.to_string());
    let max_age = u32::try_from(max_age_hours.unwrap_or(0) * 60 * 60)?;
    let max_uses = u8::try_from(max_uses.unwrap_or(0))?;
    let (target_users_file, targeted_user_count) = if let Some(attachment) = target_users.as_ref() {
        let (file, count) = download_target_users(ctx, attachment).await?;
        (Some(file), Some(count))
    } else {
        (None, None)
    };
    let request = CreateDiscordInvite {
        max_age,
        max_uses,
        temporary: temporary.unwrap_or(false),
        unique: true,
        role_ids: role_ids.iter().map(String::as_str).collect(),
        target_type: target.map(InviteTarget::api_value),
        target_user_id: target_user_id.as_deref(),
        target_application_id: target_application_id.as_deref(),
    };

    let discord_invite = ctx
        .data()
        .discord_api
        .create_invite(channel_id.get(), &request, target_users_file)
        .await?;
    let new_invite = NewTrackedInvite {
        guild_id: guild_id_string.clone(),
        invite_code: discord_invite.code.clone(),
        channel_id: discord_invite.channel.id.clone(),
        channel_type: discord_invite.channel.kind,
        primary_source: primary_source.clone(),
        secondary_source: secondary_source.clone(),
        tracked_by: ctx.author().id.to_string(),
        discord_inviter_id: discord_invite.inviter.as_ref().map(|user| user.id.clone()),
        discord_created_at: Some(discord_invite.created_at.naive_utc()),
        discord_uses: discord_invite.uses,
        max_uses: discord_invite.max_uses,
        max_age: discord_invite.max_age,
        temporary: discord_invite.temporary,
        expires_at: discord_invite.expires_at.map(|value| value.naive_utc()),
        invite_type: discord_invite.invite_type,
        flags: discord_invite.flags,
        target_type: discord_invite.target_type,
        target_user_id: discord_invite
            .target_user
            .as_ref()
            .map(|user| user.id.clone()),
        target_application_id: discord_invite
            .target_application
            .as_ref()
            .map(|application| application.id.clone()),
        scheduled_event_id: discord_invite
            .guild_scheduled_event
            .as_ref()
            .map(|event| event.id.clone()),
        targeted_user_count,
        is_vanity: false,
        tracked_at: None,
        role_ids: role_ids.clone(),
        role_assignment_mode: RoleAssignmentMode::Native,
    };

    if let Err(error) = ctx.data().repository.insert_invite(&new_invite).await {
        if let Err(cleanup_error) = ctx
            .data()
            .discord_api
            .delete_invite(&discord_invite.code)
            .await
        {
            tracing::warn!(
                %cleanup_error,
                code = %discord_invite.code,
                "failed to revoke an invite after database insertion failed"
            );
        }
        return Err(error.into());
    }

    ctx.data()
        .cache
        .set_invite(
            &guild_id_string,
            &discord_invite.code,
            u64::try_from(discord_invite.uses).unwrap_or_default(),
        )
        .await?;
    ctx.data()
        .repository
        .write_audit_log(
            &guild_id_string,
            "invite_created",
            &ctx.author().id.to_string(),
            Some(json!({
                "inviteCode": discord_invite.code,
                "primarySource": primary_source,
                "secondarySource": secondary_source,
                "channelId": channel_id,
                "roleIds": role_ids,
                "maxAge": max_age,
                "maxUses": max_uses,
                "temporary": temporary.unwrap_or(false),
                "targetedUserCount": targeted_user_count,
            })),
        )
        .await?;

    let mut details = vec![
        format!("**Invite:** https://discord.gg/{}", discord_invite.code),
        format!("**Source:** {primary_source} → {secondary_source}"),
        format!("**Channel:** <#{channel_id}>"),
        format_expiry(max_age, max_uses),
    ];
    if !role_ids.is_empty() {
        details.push(format!(
            "**Native roles:** {}",
            role_ids
                .iter()
                .map(|role_id| format!("<@&{role_id}>"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(count) = targeted_user_count {
        details.push(format!("**Targeted users:** {count}"));
    }

    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::brand()
                    .title("Invite Created")
                    .description(details.join("\n"))
                    .footer(serenity::CreateEmbedFooter::new(format!(
                        "{} / {} links",
                        current_count + 1,
                        config.max_links
                    ))),
            )
            .ephemeral(true),
    )
    .await?;
    send_log_message(
        &ctx.serenity_context().http,
        &ctx.data().repository,
        &guild_id_string,
        embeds::log(
            "Invite Created",
            format!(
                "<@{}> created `{}`\n**{}** → **{}**",
                ctx.author().id,
                discord_invite.code,
                primary_source,
                secondary_source
            ),
        ),
    )
    .await;
    Ok(())
}

fn validate_target(
    target: Option<InviteTarget>,
    target_user: Option<&serenity::User>,
    application_id: Option<&str>,
) -> anyhow::Result<()> {
    match target {
        None if target_user.is_some() || application_id.is_some() => {
            bail!("Choose a target type when providing a target user or application.")
        }
        Some(InviteTarget::Stream) if target_user.is_none() => {
            bail!("A streaming user is required for a voice stream invite.")
        }
        Some(InviteTarget::EmbeddedApplication) if application_id.is_none() => {
            bail!("An application ID is required for an embedded application invite.")
        }
        Some(InviteTarget::Stream) if application_id.is_some() => {
            bail!("A voice stream invite cannot also target an application.")
        }
        Some(InviteTarget::EmbeddedApplication) if target_user.is_some() => {
            bail!("An embedded application invite cannot also target a user.")
        }
        _ => Ok(()),
    }
}

fn format_expiry(max_age: u32, max_uses: u8) -> String {
    let age = if max_age == 0 {
        "never".to_owned()
    } else {
        format!("{} hours", max_age / 3600)
    };
    let uses = if max_uses == 0 {
        "unlimited uses".to_owned()
    } else {
        format!("{max_uses} uses")
    };
    format!("**Expiry:** {age}, {uses}")
}
