use anyhow::{Context as _, bail};
use poise::serenity_prelude as serenity;
use serde_json::json;

use crate::audit::send_log_message;
use crate::embeds;
use crate::models::NewTrackedInvite;
use crate::{Context, Error};

/// Create a new tracked invite link.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
#[allow(clippy::too_many_lines)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "Primary source, such as Instagram, YouTube, or Discord"]
    #[max_length = 100]
    primary_source: String,
    #[description = "Secondary source, such as Bio or Help Embed"]
    #[max_length = 100]
    secondary_source: String,
    #[description = "Channel for the invite; defaults to the configured channel"]
    #[channel_types("Text")]
    channel: Option<serenity::GuildChannel>,
    #[description = "Role to assign when someone joins through this invite"] role: Option<
        serenity::Role,
    >,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id_string = guild_id.to_string();
    ctx.data().repository.ensure_guild(&guild_id_string).await?;
    let config = ctx
        .data()
        .repository
        .guild_config(&guild_id_string)
        .await?
        .context("guild configuration disappeared after initialization")?;

    let channel_id = if let Some(channel) = channel {
        channel.id
    } else {
        let value = config.default_channel_id.context(
            "No channel was provided and no default channel is configured. \
                 Use `/set defaultchannel` first.",
        );
        match value {
            Ok(value) => serenity::ChannelId::new(value.parse()?),
            Err(error) => {
                ctx.send(
                    poise::CreateReply::default()
                        .embed(embeds::error(error.to_string()))
                        .ephemeral(true),
                )
                .await?;
                return Ok(());
            }
        }
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
                     Increase it with `/set maxlinks` or delete an unused link.",
                    config.max_links
                )))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let channel = channel_id.to_channel(&ctx.serenity_context().http).await?;
    let serenity::Channel::Guild(channel) = channel else {
        bail!("the configured invite channel is not a guild channel");
    };
    if channel.kind != serenity::ChannelType::Text {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(
                    "The configured invite channel is not a text channel.",
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let discord_invite = channel
        .create_invite(
            ctx.serenity_context(),
            serenity::CreateInvite::new()
                .max_age(0)
                .max_uses(0)
                .unique(true),
        )
        .await?;
    let role_ids = role
        .as_ref()
        .map(|role| vec![role.id.to_string()])
        .unwrap_or_default();
    let new_invite = NewTrackedInvite {
        guild_id: guild_id_string.clone(),
        invite_code: discord_invite.code.clone(),
        channel_id: channel_id.to_string(),
        primary_source: primary_source.clone(),
        secondary_source: secondary_source.clone(),
        created_by: ctx.author().id.to_string(),
        uses: 0,
        created_at: None,
        role_ids,
    };

    if let Err(error) = ctx.data().repository.insert_invite(&new_invite).await {
        if let Err(cleanup_error) = ctx
            .serenity_context()
            .http
            .delete_invite(
                &discord_invite.code,
                Some("Rolling back a failed InviteAnalytics creation"),
            )
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
        .set_invite(&guild_id_string, &discord_invite.code, 0)
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
                "roleId": role.as_ref().map(|value| value.id.to_string()),
            })),
        )
        .await?;

    let mut details = vec![
        format!(
            "→ **Invite link:** https://discord.gg/{}",
            discord_invite.code
        ),
        String::new(),
        format!("• **Primary source:** {primary_source}"),
        format!("• **Secondary source:** {secondary_source}"),
        format!("• **Channel:** <#{channel_id}>"),
    ];
    if let Some(role) = &role {
        details.push(format!("• **Auto-role:** <@&{}>", role.id));
    }
    details.extend([
        String::new(),
        "Only share this link through the source selected above.".to_owned(),
    ]);

    let embed = embeds::brand()
        .title("Invite Created")
        .description(details.join("\n"))
        .footer(serenity::CreateEmbedFooter::new(format!(
            "{} / {} links",
            current_count + 1,
            config.max_links
        )));
    ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true))
        .await?;

    send_log_message(
        &ctx.serenity_context().http,
        &ctx.data().repository,
        &guild_id_string,
        embeds::log(
            "Invite Created",
            format!(
                "<@{}> created invite `{}`\n**{}** → **{}**",
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
