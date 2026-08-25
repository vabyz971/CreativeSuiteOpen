// CreativeSuiteOpen — Suite créative professionnelle open source
// Copyright (C) 2026 vabyz971
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Système de menus générique partagé par toutes les apps.
//! Utilise le widget natif `iced_aw::menu::MenuBar` : sous-menus au survol,
//! comme Photoshop/Affinity. Inspiré de `iced_aw/examples/menu.rs`.

use crate::theme::{colors, metrics};
use iced::widget::{Space, button, container, text};
use iced::{Alignment, Element, Length};

/// Largeur d'un slot de bouton menu
pub const SLOT_WIDTH: f32 = 64.0;
/// Espace entre deux slots
pub const SLOT_GAP: f32 = 2.0;
/// Hauteur de la rangée de menus
pub const BAR_HEIGHT: f32 = 28.0;

/// Un item de menu déroulant
pub enum Item<Message> {
    Action {
        label: String,
        shortcut: String,
        /// État cochable optionnel (préfixe ✓ dans le libellé)
        checked: bool,
        message: Message,
    },
    Separator,
    SubMenu {
        label: String,
        items: Vec<Item<Message>>,
    },
}

impl<Message> Item<Message> {
    /// Action simple sans coche ni raccourci
    pub fn action(label: impl Into<String>, message: Message) -> Self {
        Item::Action {
            label: label.into(),
            shortcut: String::new(),
            checked: false,
            message,
        }
    }
}

/// Un menu complet (bouton + contenu déroulant)
pub struct Menu<Message> {
    pub label: String,
    pub items: Vec<Item<Message>>,
}

impl<Message> Menu<Message> {
    pub fn new(label: impl Into<String>, items: Vec<Item<Message>>) -> Self {
        Self {
            label: label.into(),
            items,
        }
    }
}

/// Rangée de boutons menus avec le `MenuBar` natif iced_aw.
/// Les sous-menus s'ouvrent au survol, comme dans Photoshop/Affinity.
/// L'état d'ouverture est géré en interne par le widget.
pub fn bar<'a, Message: Clone + 'a>(menus: &[Menu<Message>]) -> Element<'a, Message> {
    use iced_aw::menu::{Item as AwItem, Menu as AwMenu};

    let aw_items: Vec<AwItem<'a, Message, iced::Theme, iced::Renderer>> = menus
        .iter()
        .map(|m| {
            let aw_sub_items: Vec<AwItem<'a, Message, iced::Theme, iced::Renderer>> = m
                .items
                .iter()
                .map(|item| match item {
                    Item::Action {
                        label,
                        shortcut,
                        checked,
                        message,
                    } => {
                        let content = if shortcut.is_empty() {
                            if *checked {
                                format!("✓ {}", label)
                            } else {
                                format!("   {}", label)
                            }
                        } else if *checked {
                            format!("✓ {}  {}", label, shortcut)
                        } else {
                            format!("   {}  {}", label, shortcut)
                        };
                        AwItem::new(
                            button(text(content).size(13))
                                .width(Length::Fill)
                                .padding(iced::Padding::new(6.0).left(10.0).right(8.0))
                                .style(|_, s| {
                                    let mut st = button::Style::default();
                                    let hovered = s == iced::widget::button::Status::Hovered;
                                    st.background = Some(if hovered {
                                        colors::ACCENT.into()
                                    } else {
                                        iced::Color::TRANSPARENT.into()
                                    });
                                    st.text_color = if hovered {
                                        colors::TEXT_ON_ACCENT
                                    } else {
                                        colors::ON_SURFACE
                                    };
                                    st.border.radius = metrics::RADIUS_BUTTON.into();
                                    st
                                })
                                .on_press(message.clone()),
                        )
                    }
                    Item::Separator => AwItem::new(
                        container(Space::new().height(Length::Fixed(1.0)))
                            .padding(iced::Padding::new(1.0).left(6.0).right(6.0))
                            .style(|_| container::Style {
                                background: Some(colors::BORDER_PANEL.into()),
                                ..Default::default()
                            }),
                    ),
                    Item::SubMenu { label, items } => {
                        let sub_items: Vec<AwItem<'a, Message, iced::Theme, iced::Renderer>> =
                            items
                                .iter()
                                .map(|sub| match sub {
                                    Item::Action {
                                        label,
                                        shortcut,
                                        checked,
                                        message,
                                    } => {
                                        let content = if shortcut.is_empty() {
                                            if *checked {
                                                format!("✓ {}", label)
                                            } else {
                                                format!("   {}", label)
                                            }
                                        } else if *checked {
                                            format!("✓ {}  {}", label, shortcut)
                                        } else {
                                            format!("   {}  {}", label, shortcut)
                                        };
                                        AwItem::new(
                                            button(text(content).size(13))
                                                .width(Length::Fill)
                                                .padding(
                                                    iced::Padding::new(6.0).left(10.0).right(8.0),
                                                )
                                                .style(|_, s| {
                                                    let mut st = button::Style::default();
                                                    let hovered =
                                                        s == iced::widget::button::Status::Hovered;
                                                    st.background = Some(if hovered {
                                                        colors::ACCENT.into()
                                                    } else {
                                                        iced::Color::TRANSPARENT.into()
                                                    });
                                                    st.text_color = if hovered {
                                                        colors::TEXT_ON_ACCENT
                                                    } else {
                                                        colors::ON_SURFACE
                                                    };
                                                    st.border.radius =
                                                        metrics::RADIUS_BUTTON.into();
                                                    st
                                                })
                                                .on_press(message.clone()),
                                        )
                                    }
                                    Item::Separator => AwItem::new(
                                        container(Space::new().height(Length::Fixed(1.0)))
                                            .padding(iced::Padding::new(1.0).left(6.0).right(6.0))
                                            .style(|_| container::Style {
                                                background: Some(colors::BORDER_PANEL.into()),
                                                ..Default::default()
                                            }),
                                    ),
                                    Item::SubMenu { .. } => {
                                        unreachable!("sous-menu imbriqué non supporté")
                                    }
                                })
                                .collect();
                        let sub_menu = AwMenu::new(sub_items).width(200.0).offset(4.0).spacing(2.0);
                        AwItem::with_menu(
                            button(
                                iced::widget::row![
                                    text(label.clone()).size(13),
                                    Space::new().width(Length::Fill),
                                    text("▶").size(10).color(colors::TEXT_MUTED),
                                ]
                                .align_y(Alignment::Center),
                            )
                            .width(Length::Fill)
                            .padding(iced::Padding::new(6.0).left(10.0).right(8.0))
                            .style(|_, s| {
                                let mut st = button::Style::default();
                                let hovered = s == iced::widget::button::Status::Hovered;
                                st.background = Some(if hovered {
                                    colors::ACCENT.into()
                                } else {
                                    iced::Color::TRANSPARENT.into()
                                });
                                st.text_color = if hovered {
                                    colors::TEXT_ON_ACCENT
                                } else {
                                    colors::ON_SURFACE
                                };
                                st.border.radius = metrics::RADIUS_BUTTON.into();
                                st
                            }),
                            sub_menu,
                        )
                    }
                })
                .collect();

            let aw_menu = AwMenu::new(aw_sub_items)
                .width(240.0)
                .offset(6.0)
                .spacing(2.0);
            AwItem::with_menu(
                button(text(m.label.clone()).size(12).center())
                    .width(Length::Fixed(SLOT_WIDTH))
                    .height(Length::Fixed(BAR_HEIGHT))
                    .style(|_, s| {
                        let mut st = button::Style::default();
                        let hovered = s == iced::widget::button::Status::Hovered;
                        st.background = Some(if hovered {
                            colors::HOVER_OVERLAY.into()
                        } else {
                            iced::Color::TRANSPARENT.into()
                        });
                        st.text_color = colors::TEXT_PRIMARY;
                        st.border.radius = metrics::RADIUS_BUTTON.into();
                        st
                    }),
                aw_menu,
            )
        })
        .collect();

    iced_aw::menu::MenuBar::new(aw_items)
        .spacing(SLOT_GAP)
        .into()
}
