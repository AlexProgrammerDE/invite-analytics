use chrono::Utc;
use poise::serenity_prelude as serenity;
use serde_json::json;

use crate::Error;
use crate::audit::send_log_message;
use crate::commands::links::render_links_page;
use crate::embeds;
use crate::invite_sync::{
    DiscordSnapshot, fetch_discord_snapshot, refresh_tracked_metadata, synchronize_guild,
};
use crate::invite_tracking::attribute_invite;
use crate::models::{NewJoinEvent, TrackedInvite};
use crate::pagination::{PageAction, controls, next_page};
use crate::state::BotData;

pub async fn initialize_guilds(_ctx: &serenity::Context, ready: &serenity::Ready, data: &BotData) {
    for guild in &ready.guilds {
        if let Err(error) = synchronize_guild(data, guild.id.get()).await {
            tracing::warn!(
                %error,
                guild_id = %guild.id,
                "failed to initialize guild invite data"
            );
        }
    }
    tracing::info!("invite data initialized");
}

pub async fn handle_event(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    data: &BotData,
) -> Result<(), Error> {
    let result = match event {
        serenity::FullEvent::GuildCreate { guild, .. } => handle_guild_create(data, guild).await,
        serenity::FullEvent::GuildMemberAddition { new_member } => {
            handle_member_join(ctx, data, new_member).await
        }
        serenity::FullEvent::GuildMemberRemoval { guild_id, user, .. } => {
            handle_member_remove(data, *guild_id, user).await
        }
        serenity::FullEvent::GuildMemberUpdate {
            old_if_available,
            new,
            event,
        } => handle_member_update(ctx, data, old_if_available.as_ref(), new.as_ref(), event).await,
        serenity::FullEvent::InviteCreate { data: invite } => {
            handle_invite_create(data, invite).await
        }
        serenity::FullEvent::InviteDelete { data: invite } => {
            handle_invite_delete(data, invite).await
        }
        serenity::FullEvent::InteractionCreate { interaction } => {
            handle_component(ctx, data, interaction).await
        }
        _ => Ok(()),
    };

    if let Err(error) = result {
        tracing::error!(
            %error,
            event = event.snake_case_name(),
            "Discord event handler failed"
        );
    }
    Ok(())
}

pub async fn handle_framework_error(error: poise::FrameworkError<'_, BotData, Error>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            tracing::error!(
                %error,
                command = %ctx.command().qualified_name,
                "command execution failed"
            );
            if let Err(send_error) = ctx
                .send(
                    poise::CreateReply::default()
                        .embed(embeds::error(
                            "Something went wrong while running this command.",
                        ))
                        .ephemeral(true),
                )
                .await
            {
                tracing::warn!(
                    %send_error,
                    "failed to send the command error response"
                );
            }
        }
        poise::FrameworkError::Setup { error, .. } => {
            tracing::error!(%error, "Discord framework setup failed");
        }
        other => {
            if let Err(error) = poise::builtins::on_error(other).await {
                tracing::error!(%error, "failed to handle a framework error");
            }
        }
    }
}

async fn handle_guild_create(data: &BotData, guild: &serenity::Guild) -> anyhow::Result<()> {
    let guild_id = guild.id.to_string();
    tracing::info!(%guild_id, name = %guild.name, "guild became available");
    synchronize_guild(data, guild.id.get()).await?;
    Ok(())
}

async fn handle_invite_create(
    data: &BotData,
    invite: &serenity::InviteCreateEvent,
) -> anyhow::Result<()> {
    if let Some(guild_id) = invite.guild_id {
        data.cache
            .set_invite(&guild_id.to_string(), &invite.code, invite.uses)
            .await?;
    }
    Ok(())
}

async fn handle_invite_delete(
    data: &BotData,
    invite: &serenity::InviteDeleteEvent,
) -> anyhow::Result<()> {
    if let Some(guild_id) = invite.guild_id {
        let guild_id = guild_id.to_string();
        tracing::debug!(%guild_id, code = %invite.code, "invite deleted");
        data.cache
            .remember_deleted_invite(&guild_id, &invite.code)
            .await?;
        data.repository
            .mark_invite_deleted(&guild_id, &invite.code)
            .await?;
    }
    Ok(())
}

async fn handle_member_join(
    ctx: &serenity::Context,
    data: &BotData,
    member: &serenity::Member,
) -> anyhow::Result<()> {
    let guild_id = member.guild_id;
    let guild_id_string = guild_id.to_string();
    let user_id = member.user.id.to_string();
    let join_lock = data.join_lock(guild_id.get());
    let _guard = join_lock.lock().await;
    let attribution = if member.user.bot || member.user.system {
        ResolvedAttribution {
            code: None,
            tracked_invite: None,
            status: "not_applicable",
            reason: "non_human_join",
            confidence: "none",
        }
    } else {
        resolve_join_attribution(data, guild_id, &guild_id_string, &user_id).await?
    };
    let code = attribution.code;
    let tracked_invite = attribution.tracked_invite;
    let attribution_status = attribution.status;
    let reason = attribution.reason;
    let confidence = attribution.confidence;
    let joined_at = member
        .joined_at
        .map_or_else(|| Utc::now().naive_utc(), |value| value.naive_utc());
    let join = NewJoinEvent {
        tracked_invite_id: tracked_invite.as_ref().map(|invite| invite.id),
        guild_id: guild_id_string.clone(),
        user_id: user_id.clone(),
        member_joined_at: joined_at,
        account_created_at: member.user.created_at().naive_utc(),
        invite_code_snapshot: code.clone(),
        primary_source_snapshot: tracked_invite
            .as_ref()
            .map(|invite| invite.primary_source.clone()),
        secondary_source_snapshot: tracked_invite
            .as_ref()
            .map(|invite| invite.secondary_source.clone()),
        attribution_status: attribution_status.to_owned(),
        attribution_reason: Some(reason.to_owned()),
        attribution_confidence: confidence.to_owned(),
        is_bot: member.user.bot,
        is_system: member.user.system,
        member_flags: i64::from(member.flags.bits()),
        pending: member.pending,
    };

    if !data.repository.record_join(&join).await? {
        tracing::debug!(%guild_id, %user_id, %joined_at, "ignored duplicate member join event");
        return Ok(());
    }

    let assigned_roles = if member.pending || member.user.bot {
        Vec::new()
    } else if let Some(invite) = tracked_invite.as_ref() {
        assign_managed_roles(ctx, data, member, invite.id).await?
    } else {
        Vec::new()
    };

    data.repository
        .write_audit_log(
            &guild_id_string,
            "member_joined",
            &user_id,
            Some(json!({
                "inviteCode": code,
                "attributionStatus": attribution_status,
                "attributionReason": reason,
                "attributionConfidence": confidence,
                "primarySource": tracked_invite.as_ref().map(|value| &value.primary_source),
                "secondarySource": tracked_invite.as_ref().map(|value| &value.secondary_source),
                "rolesAssigned": assigned_roles,
                "isBot": member.user.bot,
                "pending": member.pending,
            })),
        )
        .await?;

    let avatar = member
        .user
        .avatar_url()
        .unwrap_or_else(|| member.user.default_avatar_url());
    let description = join_description(
        &user_id,
        code.as_deref(),
        tracked_invite.as_ref(),
        attribution_status,
        reason,
        confidence,
        &assigned_roles,
    );
    let embed = embeds::brand()
        .title("Member Joined")
        .description(description)
        .thumbnail(avatar);
    send_log_message(&ctx.http, &data.repository, &guild_id_string, embed).await;
    Ok(())
}

struct ResolvedAttribution {
    code: Option<String>,
    tracked_invite: Option<TrackedInvite>,
    status: &'static str,
    reason: &'static str,
    confidence: &'static str,
}

async fn resolve_join_attribution(
    data: &BotData,
    guild_id: serenity::GuildId,
    guild_id_string: &str,
    user_id: &str,
) -> anyhow::Result<ResolvedAttribution> {
    let initialized = data
        .cache
        .invite_snapshot_initialized(guild_id_string)
        .await?;
    let cached = data.cache.invite_snapshot(guild_id_string).await?;
    let (snapshot, fetch_failed) = match fetch_discord_snapshot(data, guild_id.get()).await {
        Ok(snapshot) => (snapshot, false),
        Err(error) => {
            tracing::warn!(
                %error,
                %guild_id,
                %user_id,
                "could not fetch invites while attributing a join"
            );
            (
                DiscordSnapshot {
                    counts: cached.clone(),
                    regular_invites: Vec::new(),
                    vanity_code: None,
                    vanity_fetch_succeeded: false,
                },
                true,
            )
        }
    };
    if !fetch_failed
        && let Err(error) = refresh_tracked_metadata(data, guild_id.get(), &snapshot).await
    {
        tracing::warn!(%error, %guild_id, "failed to refresh tracked invite metadata");
    }

    let recently_deleted = data.cache.recently_deleted_invites(guild_id_string).await?;
    let (code, observed_uses, confidence, reason, next_snapshot) = if fetch_failed {
        (None, None, "none", "discord_invite_fetch_failed", cached)
    } else if !initialized {
        (
            None,
            None,
            "none",
            "invite_cache_not_initialized",
            snapshot.counts,
        )
    } else {
        let mut current_counts = snapshot.counts;
        if !snapshot.vanity_fetch_succeeded {
            for (code, uses) in &cached {
                current_counts.entry(code.clone()).or_insert(*uses);
            }
        }
        let attribution = attribute_invite(&cached, &current_counts, &recently_deleted);
        (
            attribution.code,
            attribution.observed_uses,
            attribution.confidence,
            attribution.reason.as_str(),
            attribution.next_snapshot,
        )
    };
    data.cache
        .replace_invite_snapshot(guild_id_string, &next_snapshot)
        .await?;

    if let Some(code) = code.as_ref() {
        if recently_deleted.contains_key(code) {
            data.cache
                .clear_recently_deleted_invite(guild_id_string, code)
                .await?;
        }
        if let Some(uses) = observed_uses.and_then(|value| i64::try_from(value).ok()) {
            data.repository
                .update_invite_uses(guild_id_string, code, uses)
                .await?;
        }
    }

    let candidate_invite = if let Some(code) = code.as_ref() {
        data.repository.find_invite(guild_id_string, code).await?
    } else {
        None
    };
    let tracked_invite = candidate_invite
        .clone()
        .filter(|invite| is_eligible_source(invite, reason));
    let status = match (&code, &tracked_invite) {
        (Some(_), Some(_)) => "attributed",
        (Some(_), None) if candidate_invite.is_some() => "unattributed",
        (Some(_), None) => "untracked_invite",
        (None, _) => "unattributed",
    };
    Ok(ResolvedAttribution {
        code,
        tracked_invite,
        status,
        reason,
        confidence,
    })
}

fn is_eligible_source(invite: &TrackedInvite, reason: &str) -> bool {
    should_attribute_source(
        invite.tracking_enabled,
        &invite.status,
        invite.max_uses,
        invite.discord_uses,
        invite.expires_at,
        reason,
    )
}

fn should_attribute_source(
    tracking_enabled: bool,
    status: &str,
    max_uses: i32,
    discord_uses: i64,
    expires_at: Option<chrono::NaiveDateTime>,
    reason: &str,
) -> bool {
    if !matches!(reason, "disappeared_invite" | "recently_deleted_invite") {
        return tracking_enabled;
    }
    if matches!(status, "untracked" | "revoked" | "replaced") {
        return false;
    }
    matches!(status, "expired" | "exhausted")
        || max_uses > 0 && discord_uses.saturating_add(1) >= i64::from(max_uses)
        || expires_at.is_some_and(|expires_at| expires_at <= Utc::now().naive_utc())
}

async fn handle_member_remove(
    data: &BotData,
    guild_id: serenity::GuildId,
    user: &serenity::User,
) -> anyhow::Result<()> {
    let guild_id = guild_id.to_string();
    let user_id = user.id.to_string();
    if data
        .repository
        .record_member_left(&guild_id, &user_id)
        .await?
    {
        data.repository
            .write_audit_log(
                &guild_id,
                "member_left",
                &user_id,
                Some(json!({ "isBot": user.bot })),
            )
            .await?;
    }
    Ok(())
}

async fn handle_member_update(
    ctx: &serenity::Context,
    data: &BotData,
    old: Option<&serenity::Member>,
    new: Option<&serenity::Member>,
    event: &serenity::GuildMemberUpdateEvent,
) -> anyhow::Result<()> {
    if event.pending {
        return Ok(());
    }

    let guild_id = event.guild_id.to_string();
    let user_id = event.user.id.to_string();
    let was_pending = data
        .repository
        .record_screening_completed(&guild_id, &user_id)
        .await?;
    if !was_pending && !old.is_some_and(|member| member.pending) {
        return Ok(());
    }
    if let Some(member) = new
        && !member.user.bot
        && let Some(invite_id) = data
            .repository
            .latest_tracked_invite_for_member(&guild_id, &user_id)
            .await?
    {
        assign_managed_roles(ctx, data, member, invite_id).await?;
    }
    Ok(())
}

async fn assign_managed_roles(
    ctx: &serenity::Context,
    data: &BotData,
    member: &serenity::Member,
    invite_id: i32,
) -> anyhow::Result<Vec<String>> {
    let user_id = member.user.id.to_string();
    let role_ids = data.repository.managed_invite_role_ids(invite_id).await?;
    let mut assigned_roles = Vec::new();
    for role_id in role_ids {
        let Ok(role_id_value) = role_id.parse::<u64>() else {
            tracing::warn!(%role_id, %user_id, "stored role ID is invalid");
            continue;
        };
        if let Err(error) = member
            .add_role(&ctx.http, serenity::RoleId::new(role_id_value))
            .await
        {
            tracing::warn!(
                %error,
                %role_id,
                %user_id,
                "failed to assign a managed invite role"
            );
        } else {
            assigned_roles.push(role_id);
        }
    }
    Ok(assigned_roles)
}

fn join_description(
    user_id: &str,
    code: Option<&str>,
    invite: Option<&TrackedInvite>,
    status: &str,
    reason: &str,
    confidence: &str,
    assigned_roles: &[String],
) -> String {
    let mut lines = vec![format!("<@{user_id}> joined.")];
    if let (Some(code), Some(invite)) = (code, invite) {
        lines.push(format!("**Invite:** `{code}`"));
        lines.push(format!(
            "**Source:** {} → {}",
            invite.primary_source, invite.secondary_source
        ));
    } else if let Some(code) = code {
        lines.push(format!("**Invite:** `{code}` (not tracked)"));
    } else {
        lines.push("**Invite:** Could not be determined".to_owned());
    }
    lines.push(format!(
        "**Attribution:** {status} ({reason}, confidence: {confidence})"
    ));
    if !assigned_roles.is_empty() {
        lines.push(format!(
            "**Managed roles assigned:** {}",
            assigned_roles
                .iter()
                .map(|role_id| format!("<@&{role_id}>"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines.join("\n")
}

async fn handle_component(
    ctx: &serenity::Context,
    data: &BotData,
    interaction: &serenity::Interaction,
) -> anyhow::Result<()> {
    let serenity::Interaction::Component(component) = interaction else {
        return Ok(());
    };
    let Some(action) = PageAction::from_custom_id(&component.data.custom_id) else {
        return Ok(());
    };

    let message_id = component.message.id.get();
    let Some(mut state) = data.cache.pagination(message_id).await? else {
        component
            .create_response(
                &ctx.http,
                serenity::CreateInteractionResponse::Message(
                    serenity::CreateInteractionResponseMessage::new()
                        .content("This pagination has expired. Run the command again.")
                        .ephemeral(true),
                ),
            )
            .await?;
        return Ok(());
    };

    let page = next_page(state.current_page, state.total_pages, action);
    if page == state.current_page {
        component
            .create_response(&ctx.http, serenity::CreateInteractionResponse::Acknowledge)
            .await?;
        return Ok(());
    }

    state.current_page = page;
    data.cache.save_pagination(message_id, &state).await?;

    if state.command_name != "links" {
        component
            .create_response(&ctx.http, serenity::CreateInteractionResponse::Acknowledge)
            .await?;
        return Ok(());
    }

    let embed =
        render_links_page(&data.repository, page, &state.guild_id, &state.guild_name).await?;
    component
        .create_response(
            &ctx.http,
            serenity::CreateInteractionResponse::UpdateMessage(
                serenity::CreateInteractionResponseMessage::new()
                    .embed(embed)
                    .components(vec![controls(page, state.total_pages)]),
            ),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_attribute_source;

    #[test]
    fn source_attribution_respects_tracking_lifecycle() {
        assert!(should_attribute_source(
            true,
            "active",
            0,
            10,
            None,
            "single_counter_increase"
        ));
        assert!(should_attribute_source(
            false,
            "exhausted",
            1,
            0,
            None,
            "disappeared_invite"
        ));
        assert!(should_attribute_source(
            false,
            "deleted",
            1,
            0,
            None,
            "recently_deleted_invite"
        ));
        assert!(should_attribute_source(
            true,
            "active",
            1,
            0,
            None,
            "disappeared_invite"
        ));
        assert!(!should_attribute_source(
            false,
            "untracked",
            0,
            10,
            None,
            "single_counter_increase"
        ));
        assert!(!should_attribute_source(
            false,
            "deleted",
            0,
            10,
            None,
            "recently_deleted_invite"
        ));
        assert!(!should_attribute_source(
            true,
            "active",
            0,
            10,
            None,
            "disappeared_invite"
        ));
    }
}
