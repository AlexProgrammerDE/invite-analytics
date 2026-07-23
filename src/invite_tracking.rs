use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub struct Attribution {
    pub code: Option<String>,
    pub next_snapshot: HashMap<String, u64>,
}

pub fn attribute_invite(
    cached: &HashMap<String, u64>,
    current: &HashMap<String, u64>,
) -> Attribution {
    let increased = current
        .iter()
        .filter(|(code, uses)| **uses > cached.get(*code).copied().unwrap_or(0))
        .map(|(code, _)| code)
        .collect::<Vec<_>>();

    if let [code] = increased.as_slice() {
        let mut next_snapshot = current.clone();
        let consumed = cached.get(*code).copied().unwrap_or(0) + 1;
        next_snapshot.insert((*code).clone(), consumed);
        return Attribution {
            code: Some((*code).clone()),
            next_snapshot,
        };
    }

    Attribution {
        code: None,
        next_snapshot: current.clone(),
    }
}

pub fn normalize_invite_code(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    trimmed
        .rsplit('/')
        .next()
        .unwrap_or(trimmed)
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{attribute_invite, normalize_invite_code};

    #[test]
    fn attributes_a_single_increase() {
        let cached = HashMap::from([("first".to_owned(), 3), ("second".to_owned(), 5)]);
        let current = HashMap::from([("first".to_owned(), 3), ("second".to_owned(), 6)]);

        let attribution = attribute_invite(&cached, &current);

        assert_eq!(attribution.code.as_deref(), Some("second"));
        assert_eq!(attribution.next_snapshot, current);
    }

    #[test]
    fn consumes_batched_uses_one_join_at_a_time() {
        let current = HashMap::from([("invite".to_owned(), 4)]);
        let first = attribute_invite(&HashMap::from([("invite".to_owned(), 2)]), &current);
        let second = attribute_invite(&first.next_snapshot, &current);

        assert_eq!(first.code.as_deref(), Some("invite"));
        assert_eq!(first.next_snapshot["invite"], 3);
        assert_eq!(second.code.as_deref(), Some("invite"));
        assert_eq!(second.next_snapshot["invite"], 4);
    }

    #[test]
    fn refuses_ambiguous_increases() {
        let cached = HashMap::from([("first".to_owned(), 1), ("second".to_owned(), 1)]);
        let current = HashMap::from([("first".to_owned(), 2), ("second".to_owned(), 2)]);

        let attribution = attribute_invite(&cached, &current);

        assert_eq!(attribution.code, None);
        assert_eq!(attribution.next_snapshot, current);
    }

    #[test]
    fn normalizes_common_invite_inputs() {
        assert_eq!(normalize_invite_code("abc123"), "abc123");
        assert_eq!(
            normalize_invite_code("https://discord.gg/abc123/"),
            "abc123"
        );
        assert_eq!(normalize_invite_code("discord.com/invite/abc123"), "abc123");
    }
}
