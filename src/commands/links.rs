use anyhow::Context as _;
use poise::serenity_prelude as serenity;

use crate::db::Repository;
use crate::embeds;
use crate::models::PaginationState;
use crate::pagination::controls;
use crate::{Context, Error};

const ITEMS_PER_PAGE: i64 = 6;

/// View all tracked invite links.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn links(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id_string = guild_id.to_string();
    let guild_name = ctx
        .guild()
        .map_or_else(|| "this server".to_owned(), |guild| guild.name.clone());
    let total_invites = ctx
        .data()
        .repository
        .count_invites(&guild_id_string)
        .await?;

    if total_invites == 0 {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(
                    "No invites are tracked yet. Use `/create` to get started.",
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let total_pages = u32::try_from((total_invites + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE)?;
    let embed = render_links_page(&ctx.data().repository, 1, &guild_id_string, &guild_name).await?;
    let mut reply = poise::CreateReply::default().embed(embed).ephemeral(true);
    if total_pages > 1 {
        reply = reply.components(vec![controls(1, total_pages)]);
    }

    let handle = ctx.send(reply).await?;
    if total_pages > 1 {
        let message = handle.message().await?;
        ctx.data()
            .cache
            .save_pagination(
                message.id.get(),
                &PaginationState {
                    current_page: 1,
                    total_pages,
                    guild_id: guild_id_string,
                    guild_name,
                    command_name: "links".to_owned(),
                },
            )
            .await?;
    }

    Ok(())
}

pub(crate) async fn render_links_page(
    repository: &Repository,
    page: u32,
    guild_id: &str,
    guild_name: &str,
) -> anyhow::Result<serenity::CreateEmbed> {
    let total_invites = repository.count_invites(guild_id).await?;
    let config = repository.guild_config(guild_id).await?;
    let offset = i64::from(page.saturating_sub(1)) * ITEMS_PER_PAGE;
    let invites = repository
        .list_invites_page(guild_id, ITEMS_PER_PAGE, offset)
        .await?;
    let max_links = config.map_or(130, |value| value.max_links);

    let mut lines = vec![
        format!("• You are using **{total_invites} / {max_links}** tracked links"),
        String::new(),
    ];
    lines.extend(invites.into_iter().map(|invite| {
        format!(
            "**{} → {}**\n.gg/{}",
            invite.primary_source, invite.secondary_source, invite.invite_code
        )
    }));
    lines.extend([
        String::new(),
        "💡 Use `/lookup` to inspect an invite and its recent joins.".to_owned(),
    ]);

    Ok(embeds::brand()
        .title(format!("Invites for {guild_name}"))
        .description(lines.join("\n")))
}
