use anyhow::Context as _;
use poise::serenity_prelude as serenity;

use crate::chart;
use crate::commands::stats::StatsPeriod;
use crate::embeds;
use crate::models::{DailyCount, SourceCount};
use crate::{Context, Error};

#[derive(Clone, Copy, Debug, poise::ChoiceParameter)]
pub enum GraphDimension {
    #[name = "Primary Sources"]
    Primary,
    #[name = "Secondary Sources"]
    Secondary,
    #[name = "Join Timeline"]
    Timeline,
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
    #[description = "Time period to chart"] period: Option<StatsPeriod>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("command is missing a guild ID")?;
    let guild_id = guild_id.to_string();
    let guild_name = ctx
        .guild()
        .map_or_else(|| "This Server".to_owned(), |guild| guild.name.clone());
    ctx.defer_ephemeral().await?;

    let period = period.unwrap_or(StatsPeriod::ThirtyDays);
    let since = period.since();
    let (title, description, data) = match graph_type {
        GraphDimension::Primary => (
            format!("{guild_name}: Primary Sources ({})", period.label()),
            "Top primary sources ranked by attributed member joins.",
            ctx.data()
                .repository
                .top_primary_sources(&guild_id, since, 10)
                .await?,
        ),
        GraphDimension::Secondary => {
            let title = source_filter.as_ref().map_or_else(
                || format!("{guild_name}: Secondary Sources ({})", period.label()),
                |source| {
                    format!(
                        "{guild_name}: Secondary Sources for {source} ({})",
                        period.label()
                    )
                },
            );
            let data = ctx
                .data()
                .repository
                .top_secondary_sources(&guild_id, since, source_filter.as_deref(), 10)
                .await?;
            (
                title,
                "Top secondary sources ranked by attributed member joins.",
                data,
            )
        }
        GraphDimension::Timeline => {
            let data = ctx.data().repository.joins_by_day(&guild_id, since).await?;
            (
                format!("{guild_name}: Join Timeline ({})", period.label()),
                "Human member joins over time, including unattributed joins.",
                bucket_daily_counts(&data, 10),
            )
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
        .description(description)
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

fn bucket_daily_counts(counts: &[DailyCount], max_buckets: usize) -> Vec<SourceCount> {
    if counts.is_empty() || max_buckets == 0 {
        return Vec::new();
    }
    let bucket_size = counts.len().div_ceil(max_buckets);
    counts
        .chunks(bucket_size)
        .map(|bucket| {
            let first = bucket.first().expect("chunks are never empty");
            let last = bucket.last().expect("chunks are never empty");
            let source = if first.day == last.day {
                first.day.format("%Y-%m-%d").to_string()
            } else {
                format!("{}-{}", first.day.format("%m-%d"), last.day.format("%m-%d"))
            };
            SourceCount {
                source,
                joins: bucket.iter().map(|count| count.joins).sum(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::bucket_daily_counts;
    use crate::models::DailyCount;

    #[test]
    fn timeline_buckets_preserve_join_totals() {
        let counts = (1..=12)
            .map(|day| DailyCount {
                day: NaiveDate::from_ymd_opt(2026, 7, day).unwrap(),
                joins: i64::from(day),
            })
            .collect::<Vec<_>>();

        let buckets = bucket_daily_counts(&counts, 10);

        assert!(buckets.len() <= 10);
        assert_eq!(
            buckets.iter().map(|bucket| bucket.joins).sum::<i64>(),
            counts.iter().map(|count| count.joins).sum::<i64>()
        );
    }
}
