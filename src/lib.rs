mod audit;
mod cache;
mod chart;
mod commands;
mod config;
mod csv_transfer;
mod db;
mod embeds;
mod events;
mod invite_tracking;
mod models;
mod pagination;
mod state;

use std::time::Duration;

use anyhow::Context as _;
use poise::serenity_prelude as serenity;
use redis::aio::ConnectionManager;
use sqlx::postgres::PgPoolOptions;

use crate::cache::AppCache;
use crate::db::Repository;
use crate::state::BotData;

pub use crate::config::Config;

pub type Error = anyhow::Error;
pub type Context<'a> = poise::Context<'a, BotData, Error>;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

/// Start the bot and run it until the Discord gateway stops.
///
/// # Errors
///
/// Returns an error when configuration-backed services cannot be initialized,
/// migrations fail, or the Discord client stops unexpectedly.
pub async fn run(config: Config) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&config.database_url)
        .await
        .context("failed to connect to PostgreSQL")?;

    MIGRATOR
        .run(&pool)
        .await
        .context("failed to apply database migrations")?;

    let redis_client = redis::Client::open(config.redis_url).context("invalid Redis URL")?;
    let redis = ConnectionManager::new(redis_client)
        .await
        .context("failed to connect to Redis")?;

    let attachment_client = reqwest::Client::builder()
        .user_agent(concat!("invite-analytics/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build the attachment HTTP client")?;

    let data = BotData::new(
        Repository::new(pool),
        AppCache::new(redis),
        attachment_client,
    );

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            on_error: |error| Box::pin(events::handle_framework_error(error)),
            event_handler: |ctx, event, _framework, data| {
                Box::pin(events::handle_event(ctx, event, data))
            },
            ..Default::default()
        })
        .setup({
            let data = data.clone();
            move |ctx, ready, framework| {
                let data = data.clone();
                Box::pin(async move {
                    poise::builtins::register_globally(ctx, &framework.options().commands)
                        .await
                        .context("failed to register global Discord commands")?;

                    tracing::info!(
                        user = %ready.user.name,
                        guilds = ready.guilds.len(),
                        "connected to Discord"
                    );

                    events::initialize_guilds(ctx, ready, &data).await;
                    Ok(data)
                })
            }
        })
        .build();

    let intents = serenity::GatewayIntents::GUILDS
        | serenity::GatewayIntents::GUILD_MEMBERS
        | serenity::GatewayIntents::GUILD_INVITES;

    let mut client = serenity::ClientBuilder::new(config.discord_token, intents)
        .framework(framework)
        .await
        .context("failed to construct the Discord client")?;

    tracing::info!("InviteAnalytics started");
    client
        .start_autosharded()
        .await
        .context("Discord client stopped unexpectedly")
}
