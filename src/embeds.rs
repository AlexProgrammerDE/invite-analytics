use poise::serenity_prelude as serenity;

pub const BRAND_COLOR: u32 = 0x2D_D4_BF;
pub const SUCCESS_COLOR: u32 = 0x22_C5_5E;
pub const ERROR_COLOR: u32 = 0xEF_44_44;

pub fn brand() -> serenity::CreateEmbed {
    base(BRAND_COLOR)
}

pub fn success() -> serenity::CreateEmbed {
    base(SUCCESS_COLOR)
}

pub fn error(message: impl Into<String>) -> serenity::CreateEmbed {
    base(ERROR_COLOR).description(message)
}

pub fn log(title: impl Into<String>, description: impl Into<String>) -> serenity::CreateEmbed {
    brand().title(title).description(description)
}

fn base(color: u32) -> serenity::CreateEmbed {
    serenity::CreateEmbed::new()
        .color(serenity::Colour::new(color))
        .timestamp(serenity::Timestamp::now())
}
