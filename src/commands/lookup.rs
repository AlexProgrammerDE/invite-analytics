use anyhow::Context as _;
use poise::serenity_prelude as serenity;

use crate::embeds;
use crate::invite_tracking::normalize_invite_code;
use crate::{Context, Error};

/// Look up details for a tracked invite.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn lookup(
    ctx: Context<'_>,
    #[description = "Invite code or URL"] code: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let code = normalize_invite_code(&code);
    let Some(invite) = ctx
        .data()
        .repository
        .find_invite(&guild_id.to_string(), &code)
        .await?
    else {
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

    let recent_joins = ctx.data().repository.recent_joins(invite.id, 10).await?;
    let role_ids = ctx.data().repository.invite_role_ids(invite.id).await?;

    let recent_joins = if recent_joins.is_empty() {
        "No joins recorded yet.".to_owned()
    } else {
        recent_joins
            .iter()
            .map(|join| {
                format!(
                    "<@{}> at <t:{}:R>",
                    join.user_id,
                    join.joined_at.and_utc().timestamp()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let roles = if role_ids.is_empty() {
        "None".to_owned()
    } else {
        role_ids
            .iter()
            .map(|role_id| format!("<@&{role_id}>"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let embed = embeds::brand()
        .title(format!("Invite Lookup: {code}"))
        .field("Invite Link", format!("https://discord.gg/{code}"), true)
        .field("Primary Source", invite.primary_source, true)
        .field("Secondary Source", invite.secondary_source, true)
        .field("Total Uses", invite.uses.to_string(), true)
        .field("Channel", format!("<#{}>", invite.channel_id), true)
        .field("Created By", format!("<@{}>", invite.created_by), true)
        .field("Auto-Roles", roles, false)
        .field("Recent Joins", recent_joins, false)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Created {}",
            invite.created_at.format("%Y-%m-%d")
        )));
    ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}
