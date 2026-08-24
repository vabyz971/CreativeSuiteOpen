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

//! Panneau Préférences → Raccourcis clavier (fenêtre modale façon
//! Affinity/Photoshop) : liste des actions groupées par catégorie,
//! modification par capture de touche, remise à défaut par action ou globale.

use ui::shortcuts::{Action, Shortcuts};
use ui::theme::{colors, metrics};

use crate::Message;
use iced::widget::{button, column, container, row, scrollable, Space, text};
use iced::{Alignment, Element, Length, Padding};

pub fn view<'a>(
    shortcuts: &Shortcuts,
    capturing: Option<Action>,
) -> Element<'a, Message> {
    // Ligne d'une action : libellé | combinaison | Modifier | Reset
    let action_row =
        |action: Action| -> Element<'a, Message> {
            let is_capturing = capturing == Some(action);
            let binding_label = if is_capturing {
                "Appuyez sur une touche… (Échap annule)".to_string()
            } else {
                let l = shortcuts.label(action);
                if l.is_empty() {
                    "—".to_string()
                } else {
                    l
                }
            };

            let capturing_flag = is_capturing;
            let chip = container(
                text(binding_label)
                    .size(11)
                    .color(if is_capturing {
                        colors::ACCENT
                    } else if shortcuts.label(action).is_empty() {
                        colors::TEXT_MUTED
                    } else {
                        colors::TEXT_PRIMARY
                    }),
            )
            .padding(Padding::new(4.0).left(8.0).right(8.0))
            .style(move |_| container::Style {
                background: Some(if capturing_flag {
                    colors::BG_NODE_SELECTED.into()
                } else {
                    colors::SURFACE_CONTAINER_HIGH.into()
                }),
                border: iced::Border {
                    width: if capturing_flag { 1.0 } else { 0.0 },
                    color: colors::ACCENT,
                    radius: metrics::RADIUS_BUTTON.into(),
                },
                ..Default::default()
            });

            let small_btn = |label: &'a str, msg: Message, enabled: bool| {
                let b = button(text(label).size(11))
                    .padding(Padding::new(3.0).left(8.0).right(8.0));
                if enabled {
                    b.on_press(msg).style(|_, s| {
                        let mut st = button::Style::default();
                        st.background = Some(if s == button::Status::Hovered {
                            colors::ACCENT.into()
                        } else {
                            colors::SURFACE_CONTAINER_HIGH.into()
                        });
                        st.text_color = if s == button::Status::Hovered {
                            colors::TEXT_ON_ACCENT
                        } else {
                            colors::TEXT_SECONDARY
                        };
                        st.border.radius = metrics::RADIUS_BUTTON.into();
                        st
                    })
                } else {
                    b.style(|_, _| button::Style::default())
                }
            };

            row![
                text(action.label()).size(12).color(colors::TEXT_PRIMARY),
                Space::new().width(Length::Fill),
                chip,
                small_btn(
                    "Modifier",
                    Message::ShortcutCapture(action),
                    capturing.is_none(),
                ),
                small_btn(
                    "Défaut",
                    Message::ShortcutReset(action),
                    capturing.is_none(),
                ),
            ]
            .align_y(Alignment::Center)
            .spacing(8)
            .into()
        };

    // Liste groupée par catégorie
    let mut list = column![].spacing(6).padding(12);
    let mut last_cat = "";
    for action in Action::all() {
        let cat = action.category();
        if cat != last_cat {
            last_cat = cat;
            list = list.push(
                text(cat)
                    .size(12)
                    .font(ui::theme::fonts::SANS_SEMIBOLD)
                    .color(colors::ACCENT),
            );
        }
        list = list.push(action_row(action));
    }

    let header = container(
        row![
            text("Préférences — Raccourcis clavier")
                .size(15)
                .font(ui::theme::fonts::SANS_SEMIBOLD)
                .color(colors::TEXT_PRIMARY),
            Space::new().width(Length::Fill),
            button(text("Fermer").size(12))
                .padding(Padding::new(4.0).left(10.0).right(10.0))
                .on_press(Message::CloseShortcuts)
                .style(|_, s| {
                    let mut st = button::Style::default();
                    st.background = Some(if s == button::Status::Hovered {
                        colors::ACCENT.into()
                    } else {
                        colors::SURFACE_CONTAINER_HIGH.into()
                    });
                    st.text_color = if s == button::Status::Hovered {
                        colors::TEXT_ON_ACCENT
                    } else {
                        colors::TEXT_SECONDARY
                    };
                    st.border.radius = metrics::RADIUS_BUTTON.into();
                    st
                }),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(12)
    .style(|_| container::Style {
        background: Some(colors::BG_NODE_HEADER.into()),
        ..Default::default()
    });

    let footer = container(
        row![
            text("Cliquez sur « Modifier » puis appuyez sur la nouvelle combinaison.")
                .size(11)
                .color(colors::TEXT_MUTED),
            Space::new().width(Length::Fill),
            button(text("Tout réinitialiser").size(12))
                .padding(Padding::new(4.0).left(10.0).right(10.0))
                .on_press(Message::ShortcutResetAll)
                .style(|_, s| {
                    let mut st = button::Style::default();
                    st.background = Some(if s == button::Status::Hovered {
                        colors::ERROR_CONTAINER.into()
                    } else {
                        colors::SURFACE_CONTAINER_HIGH.into()
                    });
                    st.text_color = if s == button::Status::Hovered {
                        colors::TEXT_ON_ACCENT
                    } else {
                        colors::TEXT_SECONDARY
                    };
                    st.border.radius = metrics::RADIUS_BUTTON.into();
                    st
                }),
        ]
        .align_y(Alignment::Center)
        .spacing(10),
    )
    .width(Length::Fill)
    .padding(12)
    .style(|_| container::Style {
        background: Some(colors::BG_MENU_BAR.into()),
        ..Default::default()
    });

    let panel = column![header, scrollable(list).height(Length::Fill), footer]
        .width(Length::Fixed(620.0))
        .height(Length::Fill);

    // Fenêtre modale : panel centré, taille bornée
    iced::widget::container(
        container(panel)
            .width(Length::Fill)
            .height(Length::Fixed(640.0))
            .max_height(700)
            .style(|_| container::Style {
                background: Some(colors::BG_PANEL.into()),
                border: iced::Border {
                    width: 1.0,
                    color: colors::BORDER_PANEL,
                    radius: 8.0.into(),
                },
                shadow: ui::theme::shadows::panel(),
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .padding(40)
    .into()
}
