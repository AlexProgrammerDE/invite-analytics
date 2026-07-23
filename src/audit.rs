use poise::serenity_prelude as serenity;

use crate::db::Repository;

pub async fn send_log_message(
    http: &serenity::Http,
    repository: &Repository,
    guild_id: &str,
    embed: serenity::CreateEmbed,
) {
    let result = async {
        let Some(channel_id) = repository.log_channel_id(guild_id).await? else {
            return Ok::<(), anyhow::Error>(());
        };
        let channel_id = channel_id.parse::<u64>()?;
        serenity::ChannelId::new(channel_id)
            .send_message(http, serenity::CreateMessage::new().embed(embed))
            .await?;
        Ok(())
    }
    .await;

    if let Err(error) = result {
        tracing::warn!(
            %error,
            guild_id,
            "failed to send an activity log message"
        );
    }
}
