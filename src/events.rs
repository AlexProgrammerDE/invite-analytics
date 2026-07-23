use std::collections::HashMap;

use poise::serenity_prelude as serenity;
use serde_json::json;

use crate::Error;
use crate::audit::send_log_message;
use crate::commands::links::render_links_page;
use crate::embeds;
use crate::invite_tracking::{Attribution, attribute_invite};
use crate::pagination::{PageAction, controls, next_page};
use crate::state::BotData;

pub async fn initialize_guilds(ctx: &serenity::Context, ready: &serenity::Ready, data: &BotData) {
    for guild in &ready.guilds {
        let guild_id = guild.id.to_string();
        if let Err(error) = data.repository.ensure_guild(&guild_id).await {
            tracing::warn!(
                %error,
                %guild_id,
                "failed to initialize guild configuration"
            );
            continue;
        }
        if let Err(error) = cache_guild_invites(ctx, data, guild.id).await {
            tracing::warn!(
                %error,
                %guild_id,
                "failed to initialize guild invite cache"
            );
        }
    }

    tracing::info!("invite cache initialized");
}

pub async fn handle_event(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    data: &BotData,
) -> Result<(), Error> {
    let result = match event {
        serenity::FullEvent::GuildCreate { guild, .. } => {
            handle_guild_create(ctx, data, guild).await
        }
        serenity::FullEvent::GuildMemberAddition { new_member } => {
            handle_member_join(ctx, data, new_member).await
        }
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

async fn handle_guild_create(
    ctx: &serenity::Context,
    data: &BotData,
    guild: &serenity::Guild,
) -> anyhow::Result<()> {
    let guild_id = guild.id.to_string();
    tracing::info!(%guild_id, name = %guild.name, "joined a guild");
    data.repository.ensure_guild(&guild_id).await?;
    cache_guild_invites(ctx, data, guild.id).await
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
        tracing::debug!(%guild_id, code = %invite.code, "invite deleted");
        data.cache
            .remove_invite(&guild_id.to_string(), &invite.code)
            .await?;
    }
    Ok(())
}

async fn cache_guild_invites(
    ctx: &serenity::Context,
    data: &BotData,
    guild_id: serenity::GuildId,
) -> anyhow::Result<()> {
    let invites = guild_id.invites(&ctx.http).await?;
    let snapshot = invites
        .into_iter()
        .map(|invite| (invite.code, invite.uses))
        .collect::<HashMap<_, _>>();
    data.cache
        .replace_invite_snapshot(&guild_id.to_string(), &snapshot)
        .await
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

    let cached = data.cache.invite_snapshot(&guild_id_string).await?;
    let current = guild_id
        .invites(&ctx.http)
        .await?
        .into_iter()
        .map(|invite| (invite.code, invite.uses))
        .collect::<HashMap<_, _>>();

    let Attribution {
        code,
        next_snapshot,
    } = attribute_invite(&cached, &current);
    data.cache
        .replace_invite_snapshot(&guild_id_string, &next_snapshot)
        .await?;

    let Some(code) = code else {
        tracing::debug!(
            %guild_id,
            %user_id,
            "could not determine which invite was used"
        );
        return Ok(());
    };

    let Some(invite) = data.repository.find_invite(&guild_id_string, &code).await? else {
        tracing::debug!(%guild_id, %code, "used invite is not tracked");
        return Ok(());
    };

    data.repository
        .record_join(invite.id, &guild_id_string, &user_id)
        .await?;

    let role_ids = data.repository.invite_role_ids(invite.id).await?;
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
                "failed to assign an invite role"
            );
        } else {
            assigned_roles.push(role_id);
        }
    }

    data.repository
        .write_audit_log(
            &guild_id_string,
            "member_joined",
            &user_id,
            Some(json!({
                "inviteCode": code,
                "primarySource": invite.primary_source,
                "secondarySource": invite.secondary_source,
                "rolesAssigned": assigned_roles,
            })),
        )
        .await?;

    let role_text = if assigned_roles.is_empty() {
        String::new()
    } else {
        format!(
            "\n**Roles assigned:** {}",
            assigned_roles
                .iter()
                .map(|role_id| format!("<@&{role_id}>"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let avatar = member
        .user
        .avatar_url()
        .unwrap_or_else(|| member.user.default_avatar_url());
    let embed = embeds::brand()
        .title("Member Joined")
        .description(format!(
            "<@{user_id}> joined via `{code}`\n**Source:** {} → {}{role_text}",
            invite.primary_source, invite.secondary_source
        ))
        .thumbnail(avatar);
    send_log_message(&ctx.http, &data.repository, &guild_id_string, embed).await;

    Ok(())
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
