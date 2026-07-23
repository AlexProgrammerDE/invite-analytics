use anyhow::Context as _;
use poise::serenity_prelude as serenity;

use crate::embeds;
use crate::invite_tracking::normalize_invite_code;
use crate::models::{InviteUse, TrackedInvite};
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
    let attributed_joins = ctx.data().repository.count_invite_joins(invite.id).await?;
    let native_role_ids = ctx
        .data()
        .repository
        .native_invite_role_ids(invite.id)
        .await?;
    let managed_role_ids = ctx
        .data()
        .repository
        .managed_invite_role_ids(invite.id)
        .await?;

    let embed = lookup_embed(
        &code,
        &invite,
        attributed_joins,
        &recent_joins,
        &native_role_ids,
        &managed_role_ids,
    );
    ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}

fn lookup_embed(
    code: &str,
    invite: &TrackedInvite,
    attributed_joins: i64,
    recent_joins: &[InviteUse],
    native_role_ids: &[String],
    managed_role_ids: &[String],
) -> serenity::CreateEmbed {
    let tracked_by = if invite.tracked_by.parse::<u64>().is_ok() {
        format!("<@{}>", invite.tracked_by)
    } else {
        invite.tracked_by.clone()
    };
    let target = match invite.target_type {
        Some(1) => invite.target_user_id.as_ref().map_or_else(
            || "Voice stream".to_owned(),
            |user_id| format!("Voice stream by <@{user_id}>"),
        ),
        Some(2) => invite.target_application_id.as_ref().map_or_else(
            || "Embedded application".to_owned(),
            |application_id| format!("Embedded application `{application_id}`"),
        ),
        Some(target_type) => format!("Unknown type {target_type}"),
        None => "None".to_owned(),
    };
    let sync_status = format_timestamp(invite.last_synced_at, "R", "Never synchronized");
    let discord_created = format_timestamp(invite.discord_created_at, "F", "Unknown");
    let expires = format_timestamp(invite.expires_at, "R", "Never");
    let lifecycle_end = format_timestamp(invite.deleted_at, "F", "Active");

    embeds::brand()
        .title(format!("Invite Lookup: {code}"))
        .field("Invite Link", format!("https://discord.gg/{code}"), true)
        .field("Primary Source", &invite.primary_source, true)
        .field("Secondary Source", &invite.secondary_source, true)
        .field("Discord Uses", invite.discord_uses.to_string(), true)
        .field("Attributed Joins", attributed_joins.to_string(), true)
        .field("Status", &invite.status, true)
        .field(
            "Channel",
            if invite.is_vanity {
                "Vanity URL".to_owned()
            } else {
                format!("<#{}> (type {})", invite.channel_id, invite.channel_type)
            },
            true,
        )
        .field("Tracked By", tracked_by, true)
        .field(
            "Discord Inviter",
            invite
                .discord_inviter_id
                .as_ref()
                .map_or_else(|| "Unknown".to_owned(), |id| format!("<@{id}>")),
            true,
        )
        .field(
            "Limits",
            format!(
                "{} uses, {} seconds, temporary: {}",
                invite.max_uses, invite.max_age, invite.temporary
            ),
            false,
        )
        .field("Invite Type", invite.invite_type.to_string(), true)
        .field("Flags", invite.flags.to_string(), true)
        .field("Last Synchronized", sync_status, true)
        .field("Discord Created", discord_created, true)
        .field("Expires", expires, true)
        .field("Lifecycle End", lifecycle_end, true)
        .field(
            "Tracking State",
            format!(
                "enabled: {}, Discord active: {}",
                invite.tracking_enabled, invite.discord_active
            ),
            false,
        )
        .field("Target", target, false)
        .field(
            "Scheduled Event",
            invite
                .scheduled_event_id
                .as_ref()
                .map_or_else(|| "None".to_owned(), |event_id| format!("`{event_id}`")),
            true,
        )
        .field(
            "Targeted Users",
            invite
                .targeted_user_count
                .map_or_else(|| "Not configured".to_owned(), |count| count.to_string()),
            true,
        )
        .field("Native Roles", format_roles(native_role_ids), false)
        .field(
            "Managed Fallback Roles",
            format_roles(managed_role_ids),
            false,
        )
        .field("Recent Joins", format_recent_joins(recent_joins), false)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Tracking since {}",
            invite.tracked_at.format("%Y-%m-%d")
        )))
}

fn format_timestamp(
    timestamp: Option<chrono::NaiveDateTime>,
    style: &str,
    fallback: &str,
) -> String {
    timestamp.map_or_else(
        || fallback.to_owned(),
        |timestamp| format!("<t:{}:{style}>", timestamp.and_utc().timestamp()),
    )
}

fn format_recent_joins(joins: &[InviteUse]) -> String {
    if joins.is_empty() {
        return "No joins recorded yet.".to_owned();
    }
    joins
        .iter()
        .map(|join| {
            format!(
                "<@{}> at <t:{}:R>",
                join.user_id,
                join.member_joined_at.and_utc().timestamp()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_roles(role_ids: &[String]) -> String {
    if role_ids.is_empty() {
        return "None".to_owned();
    }
    role_ids
        .iter()
        .map(|role_id| format!("<@&{role_id}>"))
        .collect::<Vec<_>>()
        .join(", ")
}
