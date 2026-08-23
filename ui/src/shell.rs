//! Shell minimaliste et modulaire — coque partagée Photo / Vidéo (Final Cut) / Audio (FL Studio)
//! Une seule barre top + rail gauche icônes, le reste est injecté par l'app.
//! Logique partagée avec core/shell (suite-shell).

pub use suite_shell::{AppKind, ShellState};

/// Largeur réservée au logo + titre dans la top bar (esthétique)
const TITLE_RESERVED: f32 = 190.0;

use crate::theme::{colors, fonts, metrics};
use iced::widget::{container, row, text, Space};
use iced::{Alignment, Color, Element, Font, Length, Padding};

/// Barre supérieure façon Lumina Creative : logo + titre (largeur réservée),
/// action notifications à droite.
pub fn top_bar<'a, Message>(title: &'a str) -> Element<'a, Message>
where
    Message: 'a,
{
    let material = Font::with_name("Material Icons");

    let icon_btn = |codepoint: &'a str| {
        container(text(codepoint).font(material).size(20).color(colors::TEXT_SECONDARY))
            .width(Length::Fixed(30.0))
            .height(Length::Fixed(30.0))
            .center_x(Length::Fixed(30.0))
            .center_y(Length::Fixed(30.0))
            .style(|_| container::Style {
                background: Some(colors::HOVER_OVERLAY.into()),
                border: iced::Border {
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    radius: (metrics::RADIUS_DROPDOWN).into(),
                },
                ..Default::default()
            })
    };

    container(
        row![
            // Logo + titre : largeur réservée (esthétique)
            container(
                row![
                    container(Space::new().width(Length::Fixed(10.0)).height(Length::Fixed(10.0)))
                        .style(|_| container::Style {
                            background: Some(colors::ACCENT.into()),
                            border: iced::Border {
                                width: 0.0,
                                color: Color::TRANSPARENT,
                                radius: 9999.0.into(),
                            },
                            ..Default::default()
                        }),
                    text(title)
                        .size(13)
                        .font(fonts::SANS_BOLD)
                        .color(colors::TEXT_PRIMARY),
                ]
                .align_y(Alignment::Center)
                .spacing(8),
            )
            .width(Length::Fixed(TITLE_RESERVED)),
            Space::new().width(Length::Fill),
            icon_btn("\u{e7f4}"), // notifications
        ]
        .align_y(Alignment::Center)
        .spacing(12),
    )
    .width(Length::Fill)
    .height(Length::Fixed(36.0))
    .padding(Padding::new(4.0).left(12.0).right(12.0))
    .style(|_| container::Style {
        background: Some(colors::BG_MENU_BAR.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Variante avec menus applicatifs insérés entre le titre et les actions.
/// `menu_buttons` est produit par `ui::menu::buttons` — l'app garde la main
/// sur ses Messages ; les dropdowns sont rendus en overlay racine via
/// `ui::menu::dropdown_offset_x`.
pub fn top_bar_with_menus<'a, Message>(
    title: &'a str,
    menu_buttons: impl Into<Element<'a, Message>>,
    extra_actions: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let title_zone = container(
        row![
            container(Space::new().width(Length::Fixed(10.0)).height(Length::Fixed(10.0)))
                .style(|_| container::Style {
                    background: Some(colors::ACCENT.into()),
                    border: iced::Border {
                        width: 0.0,
                        color: Color::TRANSPARENT,
                        radius: 9999.0.into(),
                    },
                    ..Default::default()
                }),
            text(title)
                .size(13)
                .font(fonts::SANS_BOLD)
                .color(colors::TEXT_PRIMARY),
        ]
        .align_y(Alignment::Center)
        .spacing(8),
    )
    .width(Length::Fixed(TITLE_RESERVED));

    container(
        row![
            title_zone,
            menu_buttons.into(),
            Space::new().width(Length::Fill),
            extra_actions.into(),
        ]
        .align_y(Alignment::Center)
        .spacing(12),
    )
    .width(Length::Fill)
    .height(Length::Fixed(36.0))
    .padding(Padding::new(4.0).left(12.0).right(12.0))
    .style(|_| container::Style {
        background: Some(colors::BG_MENU_BAR.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Actions globales de droite (notifications uniquement)
pub fn global_actions<'a, Message>() -> Element<'a, Message>
where
    Message: 'a,
{
    let material = Font::with_name("Material Icons");

    let icon_btn = |codepoint: &'a str| {
        container(text(codepoint).font(material).size(20).color(colors::TEXT_SECONDARY))
            .width(Length::Fixed(28.0))
            .height(Length::Fixed(28.0))
            .center_x(Length::Fixed(28.0))
            .center_y(Length::Fixed(28.0))
            .style(|_| container::Style {
                background: Some(colors::HOVER_OVERLAY.into()),
                border: iced::Border {
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    radius: (metrics::RADIUS_DROPDOWN).into(),
                },
                ..Default::default()
            })
    };

    row![icon_btn("\u{e7f4}")] // notifications
        .align_y(Alignment::Center)
        .spacing(10)
        .into()
}

/// Rail gauche icon-only 48px (outils), collapsible
pub fn left_rail<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    container(content.into())
        .width(Length::Fixed(48.0))
        .height(Length::Fill)
        .padding(4)
        .style(|_| container::Style {
            background: Some(colors::SURFACE_CONTAINER_LOWEST.into()),
            border: iced::Border {
                width: 1.0,
                color: colors::BORDER_SUBTLE,
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Layout minimaliste : top_bar + menus applicatifs + (left_rail + central)
pub fn minimalist_layout_with_menus<'a, Message>(
    title: &'a str,
    menu_buttons: impl Into<Element<'a, Message>>,
    left_rail_content: impl Into<Element<'a, Message>>,
    central: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let top = top_bar_with_menus(
        title,
        menu_buttons,
        global_actions(),
    );
    let left = left_rail(left_rail_content);
    let center = container(central.into())
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(4)
        .style(|_| container::Style {
            background: Some(colors::SURFACE_CONTAINER_LOWEST.into()),
            ..Default::default()
        });

    iced::widget::column![
        top,
        iced::widget::row![left, center].spacing(4).height(Length::Fill)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Variante SANS rail gauche : l'app gère ses outils en flottant
/// (ex. Photo — barre verticale au-dessus du canvas)
pub fn minimalist_layout_menus_only<'a, Message>(
    title: &'a str,
    menu_buttons: impl Into<Element<'a, Message>>,
    central: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let top = top_bar_with_menus(title, menu_buttons, global_actions());
    let center = container(central.into())
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(4)
        .style(|_| container::Style {
            background: Some(colors::SURFACE_CONTAINER_LOWEST.into()),
            ..Default::default()
        });

    iced::widget::column![top, center.height(Length::Fill)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Layout minimaliste : top_bar + (left_rail + central) — apps distinctes, pas de switcher
pub fn minimalist_layout<'a, Message>(
    title: &'a str,
    left_rail_content: impl Into<Element<'a, Message>>,
    central: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let top = top_bar(title);
    let left = left_rail(left_rail_content);
    let center = container(central.into())
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(4)
        .style(|_| container::Style {
            background: Some(colors::SURFACE_CONTAINER_LOWEST.into()),
            ..Default::default()
        });

    iced::widget::column![
        top,
        iced::widget::row![left, center].spacing(4).height(Length::Fill)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
