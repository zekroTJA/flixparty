//! Small layout helpers shared by the views.

use freya::prelude::*;

pub const MUTED: (u8, u8, u8) = (150, 150, 150);
pub const ACCENT: (u8, u8, u8) = (64, 156, 255);
pub const SUCCESS: (u8, u8, u8) = (80, 200, 120);
pub const WARNING: (u8, u8, u8) = (240, 180, 60);
pub const DANGER: (u8, u8, u8) = (235, 90, 90);

pub fn section(title: &str, children: Vec<Element>) -> Element {
    Card::new()
        .width(Size::fill())
        .child(
            rect()
                .width(Size::fill())
                .spacing(10.)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(16.)
                        .font_weight(FontWeight::BOLD),
                )
                .children(children),
        )
        .into()
}

/// A [`section`] that takes a share of the remaining height of its parent,
/// which needs `Content::Flex`, and scrolls its content when it overflows.
pub fn scroll_section(title: &str, children: Vec<Element>) -> Element {
    Card::new()
        .width(Size::fill())
        .height(Size::flex(1.))
        .min_height(Size::px(120.))
        .child(
            rect()
                .expanded()
                .content(Content::Flex)
                .spacing(10.)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(16.)
                        .font_weight(FontWeight::BOLD),
                )
                .child(
                    ScrollView::new()
                        .width(Size::fill())
                        .height(Size::flex(1.))
                        .child(rect().width(Size::fill()).spacing(6.).children(children)),
                ),
        )
        .into()
}

pub fn field(name: &str, hint: Option<&str>, input: impl Into<Element>) -> Element {
    let mut caption = name.to_string();
    if let Some(hint) = hint {
        caption.push_str(&format!(" ({hint})"));
    }
    labeled(caption, MUTED, input)
}

/// A field that must be filled in. `missing` marks the label red; the input
/// should be marked with [`invalid_input_colors`].
pub fn required_field(name: &str, missing: bool, input: impl Into<Element>) -> Element {
    labeled(
        name.to_string(),
        if missing { DANGER } else { MUTED },
        input,
    )
}

pub fn invalid_input_colors() -> InputColorsThemePartial {
    InputColorsThemePartial::new()
        .border_fill(DANGER)
        .focus_border_fill(DANGER)
}

fn labeled(caption: String, color: (u8, u8, u8), input: impl Into<Element>) -> Element {
    rect()
        .width(Size::fill())
        .spacing(4.)
        .child(label().text(caption).font_size(13.).color(color))
        .child(input)
        .into()
}

pub fn dot(color: (u8, u8, u8)) -> Element {
    rect()
        .width(Size::px(10.))
        .height(Size::px(10.))
        .corner_radius(5.)
        .background(color)
        .into()
}

pub fn error_banner(text: String) -> Element {
    rect()
        .width(Size::fill())
        .padding(10.)
        .corner_radius(6.)
        .background((90, 30, 30))
        .color((255, 220, 220))
        .child(label().text(text))
        .into()
}
