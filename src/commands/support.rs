use std::collections::HashSet;

use anyhow::{Context as _, bail};
use poise::serenity_prelude as serenity;

use crate::Context;
use crate::discord_api::TargetUsersFile;

const MAX_TARGET_USERS_BYTES: u32 = 2 * 1024 * 1024;

pub async fn download_target_users(
    ctx: Context<'_>,
    attachment: &serenity::Attachment,
) -> anyhow::Result<(TargetUsersFile, i32)> {
    if attachment.size > MAX_TARGET_USERS_BYTES {
        bail!("Target-user CSV files must be 2 MiB or smaller.");
    }

    let bytes = ctx
        .data()
        .attachment_client
        .get(&attachment.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if bytes.len() > usize::try_from(MAX_TARGET_USERS_BYTES)? {
        bail!("Target-user CSV files must be 2 MiB or smaller.");
    }

    let text = std::str::from_utf8(&bytes).context("target-user CSV must be UTF-8")?;
    let mut unique_users = HashSet::new();
    for (index, line) in text.lines().enumerate() {
        let value = line.trim().trim_matches('"');
        if value.is_empty() || index == 0 && value.eq_ignore_ascii_case("user_id") {
            continue;
        }
        value.parse::<u64>().with_context(|| {
            format!(
                "target-user CSV line {} is not a Discord user ID",
                index + 1
            )
        })?;
        unique_users.insert(value);
    }
    if unique_users.is_empty() {
        bail!("Target-user CSV must contain at least one Discord user ID.");
    }
    let user_count = i32::try_from(unique_users.len()).context("target-user list is too large")?;

    Ok((
        TargetUsersFile {
            filename: attachment.filename.clone(),
            bytes: bytes.to_vec(),
        },
        user_count,
    ))
}

pub fn unique_role_ids(roles: &[Option<serenity::Role>]) -> Vec<String> {
    let mut seen = HashSet::new();
    roles
        .iter()
        .flatten()
        .map(|role| role.id.to_string())
        .filter(|role_id| seen.insert(role_id.clone()))
        .collect()
}

pub fn percentage_tenths(numerator: i64, denominator: i64) -> i64 {
    if numerator <= 0 || denominator <= 0 {
        return 0;
    }

    numerator
        .saturating_mul(1_000)
        .saturating_add(denominator / 2)
        / denominator
}

pub fn format_percentage(numerator: i64, denominator: i64) -> String {
    let tenths = percentage_tenths(numerator, denominator);
    format!("{}.{:01}%", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::percentage_tenths;

    #[test]
    fn duplicate_target_users_can_be_deduplicated() {
        let users = ["1", "2", "1"].into_iter().collect::<HashSet<_>>();
        assert_eq!(users.len(), 2);
    }

    #[test]
    fn calculates_rounded_percentages_without_division_by_zero() {
        assert_eq!(percentage_tenths(2, 3), 667);
        assert_eq!(percentage_tenths(0, 0), 0);
    }
}
