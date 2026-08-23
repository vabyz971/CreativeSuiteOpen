use crate::theme::{colors, metrics, shadows};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Padding, Theme};

pub fn menu_item<'a, Message>(
    label: &'a str,
    shortcut: &'a str,
    message: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a, // Un bouton émet un message, il faut le Clone
{
    let content = row![
        text(label).size(13),
        Space::new().width(Length::Fill),
        text(shortcut)
            .size(12)
            .style(|_theme: &Theme| text::Style {
                color: Some(colors::TEXT_MUTED),
            })
    ]
    .align_y(Alignment::Center);

    button(content)
        .width(Length::Fill)
        .padding(Padding::new(6.0).left(12.0).right(8.0))
        .style(dropdown_item_style)
        .on_press(message)
        .into()
}

pub fn menu_separator<'a, Message>() -> Element<'a, Message>
where
    Message: 'a, // CORRECTION : Garantie de durée de vie
{
    column![
        Space::new().height(Length::Fixed(4.0)),
        container(Space::new().width(Length::Fill).height(Length::Fixed(1.0))).style(|_theme| {
            container::Style {
                background: Some(colors::BORDER_PANEL.into()),
                ..Default::default()
            }
        }),
        Space::new().height(Length::Fixed(4.0)),
    ]
    .into()
}

pub fn dropdown_box<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    width: f32,
) -> Element<'a, Message>
where
    Message: 'a, // CORRECTION : Garantie de durée de vie
{
    container(content)
        .width(Length::Fixed(width))
        .padding(Padding::new(4.0))
        .style(dropdown_container_style)
        .into()
}

// --- STYLES INTERNES PARTAGÉS ---

fn dropdown_item_style(_theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::Style::default();
    style.text_color = colors::ON_SURFACE;
    style.border.radius = metrics::RADIUS_BUTTON.into();

    if status == button::Status::Hovered {
        style.background = Some(colors::ACCENT.into());
        style.text_color = colors::TEXT_ON_ACCENT;
    } else {
        style.background = Some(iced::Color::TRANSPARENT.into());
    }
    style
}

fn dropdown_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors::BG_DROPDOWN.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_SUBTLE,
            radius: metrics::RADIUS_DROPDOWN.into(),
            ..Default::default()
        },
        shadow: shadows::dropdown(),
        ..Default::default()
    }
}
