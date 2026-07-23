use std::env;

use anyhow::{Context as _, bail};

#[derive(Clone, Debug)]
pub struct Config {
    pub discord_token: String,
    pub database_url: String,
    pub redis_url: String,
}

impl Config {
    /// Read and validate all required runtime environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error when a required variable is missing or empty.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            discord_token: required("DISCORD_TOKEN")?,
            database_url: required("DATABASE_URL")?,
            redis_url: required("REDIS_URL")?,
        })
    }
}

fn required(name: &str) -> anyhow::Result<String> {
    let value = env::var(name).with_context(|| format!("{name} is not set"))?;
    if value.trim().is_empty() {
        bail!("{name} cannot be empty");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::required;

    #[test]
    fn rejects_missing_values() {
        let name = "INVITE_ANALYTICS_TEST_MISSING";
        assert!(required(name).is_err());
    }
}
