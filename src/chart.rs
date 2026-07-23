use std::io::Cursor;
use std::sync::OnceLock;

use anyhow::{Context as _, anyhow};
use image::{DynamicImage, ImageFormat, RgbImage};
use plotters::prelude::*;
use plotters::style::text_anchor::{HPos, Pos, VPos};

use crate::models::SourceCount;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 450;
const BACKGROUND: RGBColor = RGBColor(15, 23, 42);
const TEXT_PRIMARY: RGBColor = RGBColor(241, 245, 249);
const TEXT_SECONDARY: RGBColor = RGBColor(148, 163, 184);
const GRID: RGBColor = RGBColor(51, 65, 85);
const BAR: RGBColor = RGBColor(45, 212, 191);

static FONTS: OnceLock<Result<(), String>> = OnceLock::new();

pub fn render_bar_chart(title: &str, data: &[SourceCount]) -> anyhow::Result<Vec<u8>> {
    register_fonts()?;
    let mut pixels = vec![0_u8; (WIDTH * HEIGHT * 3) as usize];

    {
        let root = BitMapBackend::with_buffer(&mut pixels, (WIDTH, HEIGHT)).into_drawing_area();
        root.fill(&BACKGROUND).map_err(draw_error)?;

        let title_style = TextStyle::from(("Noto Sans", 20).into_font().style(FontStyle::Bold))
            .color(&TEXT_PRIMARY)
            .pos(Pos::new(HPos::Center, VPos::Center));
        root.draw(&Text::new(title.to_owned(), (400, 34), title_style))
            .map_err(draw_error)?;

        let max_value = data.iter().map(|item| item.joins).max().unwrap_or(1).max(1);
        let chart_left = 225_i32;
        let chart_right = 735_i32;
        let chart_width = chart_right - chart_left;
        let first_row_y = 78_i32;
        let row_pitch = 30_i32;
        let bar_height = 20_i32;

        for step in 0..=5 {
            let x = chart_left + chart_width * step / 5;
            root.draw(&PathElement::new(vec![(x, 68), (x, 382)], GRID.mix(0.55)))
                .map_err(draw_error)?;

            let value = max_value * i64::from(step) / 5;
            let tick_style = TextStyle::from(("Noto Sans", 11).into_font())
                .color(&TEXT_SECONDARY)
                .pos(Pos::new(HPos::Center, VPos::Top));
            root.draw(&Text::new(value.to_string(), (x, 390), tick_style))
                .map_err(draw_error)?;
        }

        for (row, item) in data.iter().take(10).enumerate() {
            let y = first_row_y + i32::try_from(row).unwrap_or_default() * row_pitch;
            let label_style = TextStyle::from(("Noto Sans", 13).into_font())
                .color(&TEXT_SECONDARY)
                .pos(Pos::new(HPos::Right, VPos::Center));
            root.draw(&Text::new(
                truncate(&item.source, 28),
                (chart_left - 14, y + bar_height / 2),
                label_style,
            ))
            .map_err(draw_error)?;

            let width = i32::try_from(item.joins.max(0) * i64::from(chart_width) / max_value)
                .unwrap_or(chart_width);
            if width > 0 {
                root.draw(&Rectangle::new(
                    [(chart_left, y), (chart_left + width, y + bar_height)],
                    BAR.filled(),
                ))
                .map_err(draw_error)?;
            }

            let value_style = TextStyle::from(("Noto Sans", 12).into_font())
                .color(&TEXT_PRIMARY)
                .pos(Pos::new(HPos::Left, VPos::Center));
            root.draw(&Text::new(
                item.joins.to_string(),
                ((chart_left + width + 8).min(770), y + bar_height / 2),
                value_style,
            ))
            .map_err(draw_error)?;
        }

        let axis_style = TextStyle::from(("Noto Sans", 12).into_font())
            .color(&TEXT_SECONDARY)
            .pos(Pos::new(HPos::Center, VPos::Center));
        root.draw(&Text::new("Users Invited", (480, 423), axis_style))
            .map_err(draw_error)?;

        let watermark_style = TextStyle::from(("Noto Sans", 10).into_font())
            .color(&GRID)
            .pos(Pos::new(HPos::Right, VPos::Bottom));
        root.draw(&Text::new("InviteAnalytics", (780, 438), watermark_style))
            .map_err(draw_error)?;
        root.present().map_err(draw_error)?;
    }

    let image = RgbImage::from_raw(WIDTH, HEIGHT, pixels).context("invalid chart pixel buffer")?;
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut output, ImageFormat::Png)
        .context("failed to encode the chart as PNG")?;
    Ok(output.into_inner())
}

fn register_fonts() -> anyhow::Result<()> {
    let result = FONTS.get_or_init(|| {
        plotters::style::register_font("Noto Sans", FontStyle::Normal, ttf_noto_sans::REGULAR)
            .map_err(|_| "failed to register the regular chart font".to_owned())?;
        plotters::style::register_font("Noto Sans", FontStyle::Bold, ttf_noto_sans::BOLD)
            .map_err(|_| "failed to register the bold chart font".to_owned())
    });

    result.clone().map_err(anyhow::Error::msg)
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }

    let mut output = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    output.push('…');
    output
}

fn draw_error(error: impl std::fmt::Debug) -> anyhow::Error {
    anyhow!("failed to draw chart: {error:?}")
}

#[cfg(test)]
mod tests {
    use image::GenericImageView as _;

    use super::{HEIGHT, WIDTH, render_bar_chart};
    use crate::models::SourceCount;

    #[test]
    fn renders_a_png_with_the_expected_dimensions() {
        let data = vec![
            SourceCount {
                source: "Social".to_owned(),
                joins: 12,
            },
            SourceCount {
                source: "Documentation".to_owned(),
                joins: 7,
            },
        ];

        let png = render_bar_chart("Invite Sources", &data).unwrap();
        let image = image::load_from_memory(&png).unwrap();

        assert_eq!(image.dimensions(), (WIDTH, HEIGHT));
    }
}
