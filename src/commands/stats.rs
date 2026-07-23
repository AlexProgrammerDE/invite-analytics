use anyhow::Context as _;
use chrono::{Duration, Utc};

use crate::embeds;
use crate::models::SourceCount;
use crate::{Context, Error};

#[derive(Clone, Copy, Debug, poise::ChoiceParameter)]
pub enum StatsPeriod {
    #[name = "Last 7 days"]
    SevenDays,
    #[name = "Last 30 days"]
    ThirtyDays,
    #[name = "All time"]
    AllTime,
}

impl StatsPeriod {
    fn days(self) -> Option<i64> {
        match self {
            Self::SevenDays => Some(7),
            Self::ThirtyDays => Some(30),
            Self::AllTime => None,
        }
    }
}

/// View server-wide invite analytics.
#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR"
)]
pub async fn stats(
    ctx: Context<'_>,
    #[description = "Time period for the statistics"] period: Option<StatsPeriod>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id_string = guild_id.to_string();
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

    let period = period.unwrap_or(StatsPeriod::ThirtyDays);
    let since = period
        .days()
        .map(|days| Utc::now().naive_utc() - Duration::days(days));
    let total_joins = ctx
        .data()
        .repository
        .count_joins(&guild_id_string, since)
        .await?;
    let top_primary = ctx
        .data()
        .repository
        .top_primary_sources(&guild_id_string, since, 5)
        .await?;
    let top_secondary = ctx
        .data()
        .repository
        .top_secondary_sources(&guild_id_string, since, None, 5)
        .await?;

    let guild_name = ctx
        .guild()
        .map_or_else(|| "this server".to_owned(), |guild| guild.name.clone());
    let period_label = period
        .days()
        .map_or_else(|| "All time".to_owned(), |days| format!("Last {days} days"));
    let embed = embeds::brand()
        .title(format!("Invite Stats for {guild_name}"))
        .description(format!("Showing statistics for **{period_label}**"))
        .field("Total Tracked Invites", total_invites.to_string(), true)
        .field("Total Joins", total_joins.to_string(), true)
        .field("\u{200b}", "\u{200b}", true)
        .field("Top Primary Sources", format_sources(&top_primary), false)
        .field(
            "Top Secondary Sources",
            format_sources(&top_secondary),
            false,
        );
    ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}

fn format_sources(sources: &[SourceCount]) -> String {
    if sources.is_empty() {
        return "No data yet".to_owned();
    }
    sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            format!(
                "{}. **{}**: {} joins",
                index + 1,
                source.source,
                source.joins
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
