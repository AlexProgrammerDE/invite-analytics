use anyhow::Context as _;
use poise::serenity_prelude as serenity;

use crate::chart;
use crate::embeds;
use crate::{Context, Error};

#[derive(Clone, Copy, Debug, poise::ChoiceParameter)]
pub enum GraphDimension {
    #[name = "Primary Sources"]
    Primary,
    #[name = "Secondary Sources"]
    Secondary,
}

/// Generate a chart of the top invite sources.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn graph(
    ctx: Context<'_>,
    #[description = "Source level to chart"]
    #[rename = "type"]
    graph_type: GraphDimension,
    #[description = "Only include secondary sources under this primary source"]
    #[max_length = 100]
    #[rename = "source"]
    source_filter: Option<String>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    let guild_name = ctx
        .guild()
        .map_or_else(|| "This Server".to_owned(), |guild| guild.name.clone());
    ctx.defer_ephemeral().await?;

    let (title, data) = match graph_type {
        GraphDimension::Primary => (
            format!("{guild_name}: Primary Invite Sources"),
            ctx.data()
                .repository
                .top_primary_sources(&guild_id, None, 10)
                .await?,
        ),
        GraphDimension::Secondary => {
            let title = source_filter.as_ref().map_or_else(
                || format!("{guild_name}: Secondary Invite Sources"),
                |source| format!("{guild_name}: Secondary Sources for {source}"),
            );
            let data = ctx
                .data()
                .repository
                .top_secondary_sources(&guild_id, None, source_filter.as_deref(), 10)
                .await?;
            (title, data)
        }
    };

    if data.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(
                    "There are no recorded joins for that selection yet.",
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let chart_title = title.clone();
    let png = tokio::task::spawn_blocking(move || chart::render_bar_chart(&chart_title, &data))
        .await
        .context("chart rendering task stopped unexpectedly")??;
    let attachment = serenity::CreateAttachment::bytes(png, "chart.png");
    let embed = embeds::brand()
        .title(title)
        .description("Top sources ranked by attributed member joins.")
        .image("attachment://chart.png");
    ctx.send(
        poise::CreateReply::default()
            .embed(embed)
            .attachment(attachment)
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
