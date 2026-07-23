use anyhow::Context as _;

use crate::commands::stats::StatsPeriod;
use crate::commands::support::format_percentage;
use crate::embeds;
use crate::{Context, Error};

/// Compare retained membership sessions by primary invite source.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn retention(
    ctx: Context<'_>,
    #[description = "Membership cohort period"] period: Option<StatsPeriod>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let period = period.unwrap_or(StatsPeriod::ThirtyDays);
    let sources = ctx
        .data()
        .repository
        .retention_by_primary_source(&guild_id.to_string(), period.since(), 1_000)
        .await?;
    if sources.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .embed(embeds::error(
                    "There are no membership sessions in that period.",
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let total_joined = sources.iter().map(|source| source.joined).sum::<i64>();
    let total_active = sources.iter().map(|source| source.active).sum::<i64>();
    let overall_rate = format_percentage(total_active, total_joined);
    let breakdown = sources
        .iter()
        .take(10)
        .map(|source| {
            format!(
                "**{}:** {} active / {} joined ({})",
                source.source,
                source.active,
                source.joined,
                format_percentage(source.active, source.joined)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    ctx.send(
        poise::CreateReply::default()
            .embed(
                embeds::brand()
                    .title("Invite Source Retention")
                    .description(format!("Membership sessions from **{}**.", period.label()))
                    .field("Joined", total_joined.to_string(), true)
                    .field("Still Present", total_active.to_string(), true)
                    .field("Retention", overall_rate, true)
                    .field("Top Primary Sources", breakdown, false),
            )
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
