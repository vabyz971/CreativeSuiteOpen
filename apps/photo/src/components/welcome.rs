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

//! Écran d'accueil (aucun document ouvert) : créer un document aux
//! dimensions choisies ou ouvrir une image existante.

use crate::Message;
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding};
use ui::theme::{colors, fonts, metrics};

/// Presets de documents : (libellé, largeur, hauteur en px)
const PRESETS: &[(&str, u32, u32)] = &[
    ("HD 1920 × 1080", 1920, 1080),
    ("Carré 2048", 2048, 2048),
    ("4K 3840 × 2160", 3840, 2160),
    ("A4 @300dpi", 2480, 3508),
];

pub fn render<'a>(
    w_value: &'a str,
    h_value: &'a str,
    error: Option<&'a str>,
) -> Element<'a, Message> {
    let title = text("Creative Suite Open Photo")
        .size(20)
        .font(fonts::SANS_SEMIBOLD)
        .color(colors::TEXT_PRIMARY);
    let subtitle = text("Créez un document ou ouvrez une image pour commencer.")
        .size(12)
        .color(colors::TEXT_SECONDARY);

    // Dimensions : deux champs px + presets
    let dim_input =
        |placeholder: &'a str, value: &'a str, on_input: fn(String) -> Message| {
            text_input(placeholder, value)
                .on_input(on_input)
                .width(Length::Fixed(90.0))
                .size(13)
                .padding(Padding::new(6.0).left(10.0).right(6.0))
                .style(|_t, s| {
                    let focused = matches!(s, text_input::Status::Focused { .. });
                    iced::widget::text_input::Style {
                        background: colors::SURFACE_CONTAINER_LOWEST.into(),
                        border: iced::Border {
                            width: if focused { 1.5 } else { 1.0 },
                            color: if focused {
                                colors::ACCENT
                            } else {
                                colors::BORDER_PANEL
                            },
                            radius: metrics::RADIUS_BUTTON.into(),
                        },
                        icon: colors::TEXT_MUTED,
                        placeholder: colors::TEXT_MUTED,
                        value: colors::TEXT_PRIMARY,
                        selection: colors::ACCENT,
                    }
                })
        };

    let dims = row![
        field_label("Largeur"),
        dim_input("px", w_value, Message::NewDocWidth),
        field_label("× Hauteur"),
        dim_input("px", h_value, Message::NewDocHeight),
        field_label("px"),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let preset_btns = PRESETS
        .iter()
        .map(|(label, w, h)| {
            button(text(*label).size(11).color(colors::TEXT_SECONDARY))
                .padding(Padding::new(3.0).left(8.0).right(8.0))
                .style(|_, s| {
                    let mut st = button::Style::default();
                    st.background = Some(if s == button::Status::Hovered {
                        colors::HOVER_OVERLAY.into()
                    } else {
                        colors::SURFACE_CONTAINER.into()
                    });
                    st.border.radius = metrics::RADIUS_BUTTON.into();
                    st.border.width = 1.0;
                    st.border.color = colors::BORDER_SUBTLE;
                    st
                })
                .on_press(Message::SetDocPreset {
                    w: *w,
                    h: *h,
                })
        })
        .collect::<Vec<_>>();

    let presets_row = {
        let mut r = row![].spacing(6).align_y(Alignment::Center);
        for b in preset_btns {
            r = r.push(b);
        }
        r
    };

    // Boutons d'action
    let create_btn = button(
        text("Créer le document")
            .size(13)
            .color(colors::TEXT_ON_ACCENT),
    )
    .padding(Padding::new(8.0).left(18.0).right(18.0))
    .style(|_, s| {
        let mut st = button::Style::default();
        st.background = Some(if s == button::Status::Hovered {
            colors::ACCENT_HOVER.into()
        } else {
            colors::ACCENT.into()
        });
        st.border.radius = metrics::RADIUS_BUTTON.into();
        st
    })
    .on_press(Message::CreateDocument);

    let open_btn = button(
        row![
            text("\u{e2c8}")
                .font(ui::icon_button::MATERIAL_ICONS)
                .size(15)
                .color(colors::TEXT_PRIMARY),
            text("Ouvrir une image…").size(13).color(colors::TEXT_PRIMARY),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(8.0).left(14.0).right(14.0))
    .style(|_, s| {
        let mut st = button::Style::default();
        st.background = Some(if s == button::Status::Hovered {
            colors::HOVER_OVERLAY.into()
        } else {
            colors::SURFACE_CONTAINER_HIGH.into()
        });
        st.border.radius = metrics::RADIUS_BUTTON.into();
        st
    })
    .on_press(Message::OpenImage);

    let error_line: Element<'a, Message> = match error {
        Some(e) => text(e.to_string()).size(11).color(colors::ERROR).into(),
        None => iced::widget::Space::new().height(Length::Fixed(0.0)).into(),
    };

    container(
        column![
            title,
            Space::new().height(Length::Fixed(4.0)),
            subtitle,
            Space::new().height(Length::Fixed(22.0)),
            field_label("Nouveau document"),
            Space::new().height(Length::Fixed(8.0)),
            dims,
            Space::new().height(Length::Fixed(8.0)),
            presets_row,
            Space::new().height(Length::Fixed(22.0)),
            row![create_btn, open_btn].spacing(10).align_y(Alignment::Center),
            Space::new().height(Length::Fixed(10.0)),
            error_line,
        ]
        .align_x(Alignment::Start),
    )
    .padding(28)
    .style(|_| container::Style {
        background: Some(colors::BG_PANEL.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            radius: 12.0.into(),
        },
        shadow: iced::Shadow {
            color: colors::CABLE_SHADOW,
            offset: iced::Vector::new(0.0, 10.0),
            blur_radius: 30.0,
        },
        ..Default::default()
    })
    .into()
}

fn field_label(s: &'static str) -> Element<'static, Message> {
    text(s)
        .size(11)
        .font(fonts::SANS_SEMIBOLD)
        .color(colors::TEXT_MUTED)
        .into()
}
