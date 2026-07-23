use std::time::Duration;

use anyhow::{Context as _, bail};
use chrono::{DateTime, Utc};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, RETRY_AFTER};
use reqwest::multipart::{Form, Part};
use reqwest::{RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";
const MAX_REQUEST_ATTEMPTS: usize = 4;

#[derive(Clone)]
pub struct DiscordApi {
    client: reqwest::Client,
}

impl DiscordApi {
    pub fn new(token: &str) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        let mut authorization = HeaderValue::from_str(&format!("Bot {token}"))
            .context("Discord token is not valid as an HTTP header")?;
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(concat!("invite-analytics/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("failed to build the Discord API client")?;
        Ok(Self { client })
    }

    pub async fn guild_invites(&self, guild_id: u64) -> anyhow::Result<Vec<DiscordInvite>> {
        self.get(&format!("/guilds/{guild_id}/invites")).await
    }

    pub async fn guild_vanity(&self, guild_id: u64) -> anyhow::Result<VanityInvite> {
        self.get(&format!("/guilds/{guild_id}/vanity-url")).await
    }

    pub async fn create_invite(
        &self,
        channel_id: u64,
        request: &CreateDiscordInvite<'_>,
        target_users: Option<TargetUsersFile>,
    ) -> anyhow::Result<DiscordInvite> {
        let url = format!("{DISCORD_API_BASE}/channels/{channel_id}/invites");
        let response = if let Some(target_users) = target_users {
            let payload = serde_json::to_string(request)
                .context("failed to serialize invite creation parameters")?;
            self.send_with_retry(|| {
                let part = Part::bytes(target_users.bytes.clone())
                    .file_name(target_users.filename.clone())
                    .mime_str("text/csv")
                    .context("failed to prepare the target users file")?;
                Ok(self.client.post(&url).multipart(
                    Form::new()
                        .text("payload_json", payload.clone())
                        .part("target_users_file", part),
                ))
            })
            .await?
        } else {
            self.send_with_retry(|| Ok(self.client.post(&url).json(request)))
                .await?
        };
        decode_response(&response)
    }

    pub async fn update_target_users(
        &self,
        code: &str,
        target_users: TargetUsersFile,
    ) -> anyhow::Result<()> {
        let url = format!("{DISCORD_API_BASE}/invites/{code}/target-users");
        let response = self
            .send_with_retry(|| {
                let part = Part::bytes(target_users.bytes.clone())
                    .file_name(target_users.filename.clone())
                    .mime_str("text/csv")
                    .context("failed to prepare the target users file")?;
                Ok(self
                    .client
                    .put(&url)
                    .multipart(Form::new().part("target_users_file", part)))
            })
            .await?;
        decode_empty_response(&response)
    }

    pub async fn target_users_status(&self, code: &str) -> anyhow::Result<TargetUsersJobStatus> {
        self.get(&format!("/invites/{code}/target-users/job-status"))
            .await
    }

    pub async fn target_users_csv(&self, code: &str) -> anyhow::Result<Vec<u8>> {
        let url = format!("{DISCORD_API_BASE}/invites/{code}/target-users");
        let response = self.send_with_retry(|| Ok(self.client.get(&url))).await?;
        decode_bytes(&response)
    }

    pub async fn delete_invite(&self, code: &str) -> anyhow::Result<()> {
        let url = format!("{DISCORD_API_BASE}/invites/{code}");
        let response = self
            .send_with_retry(|| {
                Ok(self
                    .client
                    .delete(&url)
                    .header("X-Audit-Log-Reason", "Deleted via InviteAnalytics"))
            })
            .await?;
        decode_empty_response(&response)
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{DISCORD_API_BASE}{path}");
        let response = self.send_with_retry(|| Ok(self.client.get(&url))).await?;
        decode_response(&response)
    }

    async fn send_with_retry<F>(&self, mut build: F) -> anyhow::Result<ApiResponse>
    where
        F: FnMut() -> anyhow::Result<RequestBuilder>,
    {
        let mut transient_delay = Duration::from_millis(250);
        for attempt in 1..=MAX_REQUEST_ATTEMPTS {
            let response = match build()?.send().await {
                Ok(response) => response,
                Err(error) if attempt < MAX_REQUEST_ATTEMPTS => {
                    tracing::warn!(%error, attempt, "Discord API request failed; retrying");
                    tokio::time::sleep(transient_delay).await;
                    transient_delay *= 2;
                    continue;
                }
                Err(error) => return Err(error).context("Discord API request failed"),
            };
            let status = response.status();
            let retry_after_header = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_retry_after);
            let body = response.bytes().await?.to_vec();

            if status == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_REQUEST_ATTEMPTS {
                let retry_after = retry_after_header
                    .or_else(|| retry_after_from_body(&body))
                    .unwrap_or(Duration::from_secs(1));
                tracing::warn!(
                    attempt,
                    retry_after_ms = retry_after.as_millis(),
                    "Discord API rate limit reached; retrying"
                );
                tokio::time::sleep(retry_after).await;
                continue;
            }
            if status.is_server_error() && attempt < MAX_REQUEST_ATTEMPTS {
                tracing::warn!(%status, attempt, "Discord API server error; retrying");
                tokio::time::sleep(transient_delay).await;
                transient_delay *= 2;
                continue;
            }

            return Ok(ApiResponse { status, body });
        }

        unreachable!("the Discord request retry loop always returns")
    }
}

struct ApiResponse {
    status: StatusCode,
    body: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiscordInvite {
    #[serde(rename = "type")]
    pub invite_type: i32,
    pub code: String,
    pub channel: DiscordInviteChannel,
    pub inviter: Option<DiscordUser>,
    pub uses: i64,
    pub max_uses: i32,
    pub max_age: i32,
    pub temporary: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub target_type: Option<i32>,
    pub target_user: Option<DiscordUser>,
    pub target_application: Option<DiscordApplication>,
    pub guild_scheduled_event: Option<DiscordScheduledEvent>,
    #[serde(default)]
    pub flags: i64,
    #[serde(default)]
    pub roles: Vec<DiscordRole>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiscordInviteChannel {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiscordUser {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiscordApplication {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiscordScheduledEvent {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiscordRole {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VanityInvite {
    pub code: Option<String>,
    #[serde(default)]
    pub uses: i64,
}

#[derive(Debug, Serialize)]
pub struct CreateDiscordInvite<'a> {
    pub max_age: u32,
    pub max_uses: u8,
    pub temporary: bool,
    pub unique: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub role_ids: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_type: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_user_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_application_id: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct TargetUsersFile {
    pub filename: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TargetUsersJobStatus {
    pub status: i32,
    pub total_users: i64,
    pub processed_users: i64,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

fn decode_response<T: DeserializeOwned>(response: &ApiResponse) -> anyhow::Result<T> {
    if !response.status.is_success() {
        bail!(
            "Discord API returned {}: {}",
            response.status,
            discord_error_message(&response.body)
        );
    }
    serde_json::from_slice(&response.body).context("Discord returned an unexpected response")
}

fn decode_empty_response(response: &ApiResponse) -> anyhow::Result<()> {
    if response.status.is_success() {
        return Ok(());
    }
    bail!(
        "Discord API returned {}: {}",
        response.status,
        discord_error_message(&response.body)
    )
}

fn decode_bytes(response: &ApiResponse) -> anyhow::Result<Vec<u8>> {
    if !response.status.is_success() {
        bail!(
            "Discord API returned {}: {}",
            response.status,
            discord_error_message(&response.body)
        );
    }
    Ok(response.body.clone())
}

fn parse_retry_after(value: &str) -> Option<Duration> {
    value
        .parse::<f64>()
        .ok()
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
}

fn retry_after_from_body(body: &[u8]) -> Option<Duration> {
    #[derive(Deserialize)]
    struct RateLimit {
        retry_after: f64,
    }

    serde_json::from_slice::<RateLimit>(body)
        .ok()
        .and_then(|response| Duration::try_from_secs_f64(response.retry_after).ok())
}

fn discord_error_message(bytes: &[u8]) -> String {
    #[derive(Deserialize)]
    struct DiscordError {
        message: Option<String>,
        code: Option<i64>,
    }

    serde_json::from_slice::<DiscordError>(bytes).map_or_else(
        |_| String::from_utf8_lossy(bytes).chars().take(500).collect(),
        |error| match (error.message, error.code) {
            (Some(message), Some(code)) => format!("{message} ({code})"),
            (Some(message), None) => message,
            (None, Some(code)) => format!("error code {code}"),
            (None, None) => "unknown Discord API error".to_owned(),
        },
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use reqwest::StatusCode;

    use super::{
        ApiResponse, DiscordInvite, decode_response, discord_error_message, parse_retry_after,
        retry_after_from_body,
    };

    #[test]
    fn extracts_discord_error_details() {
        assert_eq!(
            discord_error_message(br#"{"message":"Missing Permissions","code":50013}"#),
            "Missing Permissions (50013)"
        );
    }

    #[test]
    fn parses_discord_rate_limit_delays() {
        assert_eq!(
            parse_retry_after("1.25"),
            Some(Duration::from_millis(1_250))
        );
        assert_eq!(
            retry_after_from_body(br#"{"retry_after":0.5,"global":false}"#),
            Some(Duration::from_millis(500))
        );
    }

    #[test]
    fn rejects_invite_responses_without_usage_metadata() {
        let response = ApiResponse {
            status: StatusCode::OK,
            body: br#"{"type":0,"code":"missing","channel":{"id":"1","type":0}}"#.to_vec(),
        };

        assert!(decode_response::<DiscordInvite>(&response).is_err());
    }
}
