#![warn(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

pub fn normalize_invite_code(value: &str) -> String {
    let trimmed = value
        .trim()
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/');
    let code = trimmed.rsplit('/').next().unwrap_or(trimmed);
    code.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::normalize_invite_code;

    #[test]
    fn normalizes_common_invite_inputs() {
        assert_eq!(normalize_invite_code("abc123"), "abc123");
        assert_eq!(
            normalize_invite_code("https://discord.gg/abc123/"),
            "abc123"
        );
        assert_eq!(
            normalize_invite_code("discord.com/invite/abc123/?event=42"),
            "abc123"
        );
        assert_eq!(normalize_invite_code("discord.gg/abc123#details"), "abc123");
    }
}
