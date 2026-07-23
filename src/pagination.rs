use poise::serenity_prelude as serenity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageAction {
    First,
    Previous,
    Next,
    Last,
}

impl PageAction {
    pub fn from_custom_id(value: &str) -> Option<Self> {
        match value {
            "page:first" => Some(Self::First),
            "page:prev" => Some(Self::Previous),
            "page:next" => Some(Self::Next),
            "page:last" => Some(Self::Last),
            _ => None,
        }
    }
}

pub fn next_page(current: u32, total: u32, action: PageAction) -> u32 {
    let total = total.max(1);
    match action {
        PageAction::First => 1,
        PageAction::Previous => current.saturating_sub(1).max(1),
        PageAction::Next => current.saturating_add(1).min(total),
        PageAction::Last => total,
    }
}

pub fn controls(current_page: u32, total_pages: u32) -> serenity::CreateActionRow {
    use serenity::{ButtonStyle, CreateButton};

    serenity::CreateActionRow::Buttons(vec![
        CreateButton::new("page:first")
            .label("<<")
            .style(ButtonStyle::Secondary)
            .disabled(current_page <= 1),
        CreateButton::new("page:prev")
            .label("<")
            .style(ButtonStyle::Secondary)
            .disabled(current_page <= 1),
        CreateButton::new("page:indicator")
            .label(format!("{current_page} / {total_pages}"))
            .style(ButtonStyle::Secondary)
            .disabled(true),
        CreateButton::new("page:next")
            .label(">")
            .style(ButtonStyle::Primary)
            .disabled(current_page >= total_pages),
        CreateButton::new("page:last")
            .label(">>")
            .style(ButtonStyle::Secondary)
            .disabled(current_page >= total_pages),
    ])
}

#[cfg(test)]
mod tests {
    use super::{PageAction, next_page};

    #[test]
    fn pagination_stays_within_bounds() {
        assert_eq!(next_page(1, 4, PageAction::Previous), 1);
        assert_eq!(next_page(4, 4, PageAction::Next), 4);
        assert_eq!(next_page(3, 4, PageAction::First), 1);
        assert_eq!(next_page(2, 4, PageAction::Last), 4);
    }
}
