use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributionReason {
    SingleCounterIncrease,
    BatchedCounterIncrease,
    DisappearedInvite,
    RecentlyDeletedInvite,
    AmbiguousCounterIncrease,
    AmbiguousDeletedInvite,
    NoCounterIncrease,
}

impl AttributionReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleCounterIncrease => "single_counter_increase",
            Self::BatchedCounterIncrease => "batched_counter_increase",
            Self::DisappearedInvite => "disappeared_invite",
            Self::RecentlyDeletedInvite => "recently_deleted_invite",
            Self::AmbiguousCounterIncrease => "ambiguous_counter_increase",
            Self::AmbiguousDeletedInvite => "ambiguous_deleted_invite",
            Self::NoCounterIncrease => "no_counter_increase",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Attribution {
    pub code: Option<String>,
    pub observed_uses: Option<u64>,
    pub confidence: &'static str,
    pub reason: AttributionReason,
    pub next_snapshot: HashMap<String, u64>,
}

pub fn attribute_invite(
    cached: &HashMap<String, u64>,
    current: &HashMap<String, u64>,
    recently_deleted: &HashMap<String, u64>,
) -> Attribution {
    let increased = current
        .iter()
        .filter(|(code, uses)| **uses > cached.get(*code).copied().unwrap_or(0))
        .map(|(code, _)| code)
        .collect::<Vec<_>>();
    let disappeared = cached
        .keys()
        .filter(|code| !current.contains_key(*code))
        .collect::<Vec<_>>();
    let missing_codes = disappeared
        .iter()
        .copied()
        .chain(recently_deleted.keys())
        .collect::<HashSet<_>>();

    if let [code] = increased.as_slice()
        && missing_codes.is_empty()
    {
        let mut next_snapshot = current.clone();
        let consumed = cached.get(*code).copied().unwrap_or(0) + 1;
        let batched = current.get(*code).is_some_and(|uses| *uses > consumed);
        next_snapshot.insert((*code).clone(), consumed);
        return Attribution {
            code: Some((*code).clone()),
            observed_uses: Some(consumed),
            confidence: if batched { "probable" } else { "high" },
            reason: if batched {
                AttributionReason::BatchedCounterIncrease
            } else {
                AttributionReason::SingleCounterIncrease
            },
            next_snapshot,
        };
    }

    if !increased.is_empty() {
        return Attribution {
            code: None,
            observed_uses: None,
            confidence: "none",
            reason: AttributionReason::AmbiguousCounterIncrease,
            next_snapshot: current.clone(),
        };
    }

    if let Some(code) = missing_codes.iter().copied().next()
        && missing_codes.len() == 1
    {
        let (observed_uses, reason) = recently_deleted.get(code).map_or_else(
            || {
                (
                    cached.get(code).copied().map(|uses| uses + 1),
                    AttributionReason::DisappearedInvite,
                )
            },
            |uses| (Some(*uses + 1), AttributionReason::RecentlyDeletedInvite),
        );
        return Attribution {
            code: Some(code.clone()),
            observed_uses,
            confidence: "probable",
            reason,
            next_snapshot: current.clone(),
        };
    }

    Attribution {
        code: None,
        observed_uses: None,
        confidence: "none",
        reason: if missing_codes.len() > 1 {
            AttributionReason::AmbiguousDeletedInvite
        } else {
            AttributionReason::NoCounterIncrease
        },
        next_snapshot: current.clone(),
    }
}

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
    use std::collections::HashMap;

    use super::{AttributionReason, attribute_invite, normalize_invite_code};

    #[test]
    fn attributes_a_single_increase() {
        let cached = HashMap::from([("first".to_owned(), 3), ("second".to_owned(), 5)]);
        let current = HashMap::from([("first".to_owned(), 3), ("second".to_owned(), 6)]);

        let attribution = attribute_invite(&cached, &current, &HashMap::new());

        assert_eq!(attribution.code.as_deref(), Some("second"));
        assert_eq!(attribution.observed_uses, Some(6));
        assert_eq!(attribution.reason, AttributionReason::SingleCounterIncrease);
        assert_eq!(attribution.next_snapshot, current);
    }

    #[test]
    fn consumes_batched_uses_one_join_at_a_time() {
        let current = HashMap::from([("invite".to_owned(), 4)]);
        let first = attribute_invite(
            &HashMap::from([("invite".to_owned(), 2)]),
            &current,
            &HashMap::new(),
        );
        let second = attribute_invite(&first.next_snapshot, &current, &HashMap::new());

        assert_eq!(first.code.as_deref(), Some("invite"));
        assert_eq!(first.reason, AttributionReason::BatchedCounterIncrease);
        assert_eq!(first.confidence, "probable");
        assert_eq!(first.next_snapshot["invite"], 3);
        assert_eq!(second.code.as_deref(), Some("invite"));
        assert_eq!(second.next_snapshot["invite"], 4);
    }

    #[test]
    fn refuses_ambiguous_increases() {
        let cached = HashMap::from([("first".to_owned(), 1), ("second".to_owned(), 1)]);
        let current = HashMap::from([("first".to_owned(), 2), ("second".to_owned(), 2)]);

        let attribution = attribute_invite(&cached, &current, &HashMap::new());

        assert_eq!(attribution.code, None);
        assert_eq!(
            attribution.reason,
            AttributionReason::AmbiguousCounterIncrease
        );
        assert_eq!(attribution.next_snapshot, current);
    }

    #[test]
    fn attributes_a_recently_consumed_invite() {
        let deleted = HashMap::from([("one-use".to_owned(), 0)]);

        let attribution = attribute_invite(&HashMap::new(), &HashMap::new(), &deleted);

        assert_eq!(attribution.code.as_deref(), Some("one-use"));
        assert_eq!(attribution.observed_uses, Some(1));
        assert_eq!(attribution.reason, AttributionReason::RecentlyDeletedInvite);
        assert_eq!(attribution.confidence, "probable");
    }

    #[test]
    fn attributes_a_single_disappeared_invite() {
        let cached = HashMap::from([("one-use".to_owned(), 0), ("persistent".to_owned(), 3)]);
        let current = HashMap::from([("persistent".to_owned(), 3)]);

        let attribution = attribute_invite(&cached, &current, &HashMap::new());

        assert_eq!(attribution.code.as_deref(), Some("one-use"));
        assert_eq!(attribution.observed_uses, Some(1));
        assert_eq!(attribution.reason, AttributionReason::DisappearedInvite);
        assert_eq!(attribution.confidence, "probable");
    }

    #[test]
    fn refuses_mixed_increment_and_deletion_candidates() {
        let cached = HashMap::from([("increased".to_owned(), 2)]);
        let current = HashMap::from([("increased".to_owned(), 3)]);
        let deleted = HashMap::from([("deleted".to_owned(), 0)]);

        let attribution = attribute_invite(&cached, &current, &deleted);

        assert_eq!(attribution.code, None);
        assert_eq!(
            attribution.reason,
            AttributionReason::AmbiguousCounterIncrease
        );
    }

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
