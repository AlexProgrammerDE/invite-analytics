use anyhow::Context as _;
use invite_analytics::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dotenv_result = dotenvy::dotenv();
    init_tracing();
    if let Err(error) = dotenv_result
        && !error.not_found()
    {
        tracing::warn!(%error, "failed to load .env");
    }

    let config = Config::from_env().context("invalid application configuration")?;
    invite_analytics::run(config).await
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("invite_analytics=info,poise=warn,serenity=warn"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .json()
        .init();
}
