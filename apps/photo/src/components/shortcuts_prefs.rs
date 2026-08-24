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

//! Fenêtre Préférences (vraie fenêtre OS, cf. iced examples/multi_window).
//! Sidebar gauche listant les sections d'options, contenu à droite.
//! Section active : Raccourcis clavier (capture, reset, défauts).
//! Design : tokens DESIGN.md via ui::theme.

use ui::shortcuts::{Action, Shortcuts};
use ui::theme::{colors, fonts, metrics};

use crate::Message;
use iced::widget::{button, column, container, row, scrollable, Space, text};
use iced::{Alignment, Element, Length, Padding};

/// Sections de la sidebar
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrefsSection {
    Shortcuts,
    Interface,
    About,
}

pub fn view<'a>(
    shortcuts: &'a Shortcuts,
    capturing: Option<Action>,
    section: PrefsSection,
) -> Element<'a, Message> {
    // --- Sidebar gauche (liste des options de l'application) ---
    let sidebar_entry = |label: &'a str, section_kind: PrefsSection| {
        let is_active = section == section_kind;
        button(
            text(label)
                .size(13)
                .color(if is_active {
                    colors::TEXT_PRIMARY
                } else {
                    colors::TEXT_SECONDARY
                })
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding(Padding::new(8.0).left(12.0).right(8.0))
        .style(move |_t, s| {
            let mut st = button::Style::default();
            st.background = Some(if is_active {
                colors::BG_NODE_SELECTED.into()
            } else if s == button::Status::Hovered {
                colors::HOVER_OVERLAY.into()
            } else {
                iced::Color::TRANSPARENT.into()
            });
            st.border.radius = metrics::RADIUS_BUTTON.into();
            st
        })
        .on_press(Message::PrefsSection(section_kind))
    };

    let sidebar = container(
        column![
            text("Préférences")
                .size(15)
                .font(fonts::SANS_SEMIBOLD)
                .color(colors::TEXT_PRIMARY),
            Space::new().height(Length::Fixed(14.0)),
            sidebar_entry("Raccourcis clavier", PrefsSection::Shortcuts),
            sidebar_entry("Interface", PrefsSection::Interface),
            sidebar_entry("À propos", PrefsSection::About),
            Space::new().height(Length::Fill),
            button(
                text("Fermer la fenêtre").size(12).color(colors::TEXT_SECONDARY)
            )
            .width(Length::Fill)
            .padding(Padding::new(8.0).left(12.0).right(8.0))
            .style(|_t, s| {
                let mut st = button::Style::default();
                st.background = Some(if s == button::Status::Hovered {
                    colors::HOVER_OVERLAY.into()
                } else {
                    iced::Color::TRANSPARENT.into()
                });
                st.border.radius = metrics::RADIUS_BUTTON.into();
                st
            })
            .on_press(Message::CloseShortcuts),
        ]
        .spacing(4)
        .padding(10),
    )
    .width(Length::Fixed(200.0))
    .height(Length::Fill)
    .style(|_| container::Style {
        background: Some(colors::SURFACE_CONTAINER_LOW.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            ..Default::default()
        },
        ..Default::default()
    });

    // --- Contenu droit selon la section ---
    let content: Element<'a, Message> = match section {
        PrefsSection::Shortcuts => shortcuts_view(shortcuts, capturing),
        PrefsSection::Interface => centered_note(
            "Options d'interface",
            "Affichage des panneaux, thème et disposition — à venir.",
        ),
        PrefsSection::About => centered_note(
            "Creative Suite Open — Photo",
            "Suite créative professionnelle open source pour Linux, Windows et macOS.\nLicence GPL-3.0-or-later — voir LICENSE.",
        ),
    };

    container(row![sidebar, content])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(colors::BG_APP.into()),
            ..Default::default()
        })
        .into()
}

fn centered_note<'a>(title: &'a str, body: &'a str) -> Element<'a, Message> {
    container(
        column![
            text(title)
                .size(18)
                .font(fonts::SANS_SEMIBOLD)
                .color(colors::TEXT_PRIMARY),
            Space::new().height(Length::Fixed(10.0)),
            text(body).size(13).color(colors::TEXT_SECONDARY),
        ]
        .spacing(4),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

// ---------------------------------------------------------------------------
// Section Raccourcis clavier
// ---------------------------------------------------------------------------

fn shortcuts_view<'a>(
    shortcuts: &'a Shortcuts,
    capturing: Option<Action>,
) -> Element<'a, Message> {
    // Ligne d'action : libellé | chip combinaison | Modifier | Défaut
    let action_row =
        |action: Action| -> Element<'a, Message> {
            let is_capturing = capturing == Some(action);
            let current = shortcuts.label(action);
            let binding_label = if is_capturing {
                "Appuyez sur une touche… (Échap annule)".to_string()
            } else if current.is_empty() {
                "—".to_string()
            } else {
                current.clone()
            };

            let capturing_flag = is_capturing;
            let chip = container(
                text(binding_label)
                    .size(11)
                    .color(if is_capturing {
                        colors::ACCENT
                    } else if current.is_empty() {
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

    let mut list = column![].spacing(6).padding(16);
    let mut last_cat = "";
    for action in Action::all() {
        let cat = action.category();
        if cat != last_cat {
            last_cat = cat;
            list = list.push(
                text(cat)
                    .size(12)
                    .font(fonts::SANS_SEMIBOLD)
                    .color(colors::ACCENT),
            );
        }
        list = list.push(action_row(action));
    }

    let header = container(
        row![
            text("Raccourcis clavier")
                .size(18)
                .font(fonts::SANS_SEMIBOLD)
                .color(colors::TEXT_PRIMARY),
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
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(Padding::new(12.0).left(16.0).right(16.0));

    let footer = container(
        text("Cliquez sur « Modifier » puis appuyez sur la nouvelle combinaison. Les modifications sont enregistrées automatiquement.")
            .size(11)
            .color(colors::TEXT_MUTED),
    )
    .width(Length::Fill)
    .padding(Padding::new(12.0).left(16.0).right(16.0));

    column![
        header,
        container(scrollable(list).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(colors::SURFACE.into()),
                ..Default::default()
            }),
        footer,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
