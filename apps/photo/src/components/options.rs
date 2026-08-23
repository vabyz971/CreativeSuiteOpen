// CreativeSuiteOpen — Suite créative professionnelle open source
// Copyright (C) 2025 vabyz971
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

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length};
use ui::theme::{colors, fonts, metrics};

use crate::Message;

/// Page Options accessible depuis Édition → Préférences (comme GIMP/Photoshop)
pub fn view<'a>(
    gpu_info: Option<String>,
    gpu_available: bool,
    zoom_level: u32,
) -> Element<'a, Message> {
    let header = container(
        row![
            text("Préférences").size(18).font(fonts::SANS_SEMIBOLD).color(colors::TEXT_PRIMARY),
            Space::new().width(Length::Fill),
            button(text("✕").size(14).color(colors::TEXT_PRIMARY))
                .padding(6)
                .style(|_, s| {
                    let mut st = button::Style::default();
                    st.background = Some(if s == button::Status::Hovered {
                        colors::ERROR_CONTAINER.into()
                    } else {
                        iced::Color::TRANSPARENT.into()
                    });
                    st.text_color = colors::TEXT_PRIMARY;
                    st.border.radius = metrics::RADIUS_BUTTON.into();
                    st
                })
                .on_press(Message::CloseOptions)
        ]
        .align_y(Alignment::Center),
    )
    .padding(12)
    .style(|_| container::Style {
        background: Some(colors::BG_NODE_HEADER.into()),
        ..Default::default()
    });

    let hardware_section = section(
        "Matériel",
        "Détection GPU/CPU pour le rendu direct (comme GIMP → Préférences → Système)",
        column![
            row![
                text("Rendu UI :").size(13).color(colors::ON_SURFACE),
                Space::new().width(Length::Fill),
                text(if gpu_available { "WGPU Direct" } else { "CPU Fallback" })
                    .size(12)
                    .color(if gpu_available {
                        colors::SUCCESS
                    } else {
                        colors::ERROR
                    }),
            ]
            .align_y(Alignment::Center),
            Space::new().height(Length::Fixed(8.0)),
            container(
                text(gpu_info.clone().unwrap_or_else(|| "Détection en cours...".into()))
                    .size(11)
                    .color(colors::TEXT_SECONDARY)
            )
            .padding(12)
            .style(|_| container::Style {
                background: Some(colors::SURFACE_CONTAINER_LOWEST.into()),
                border: iced::Border {
                    width: 1.0,
                    color: colors::BORDER_PANEL,
                    radius: metrics::RADIUS_DROPDOWN.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .width(Length::Fill),
            Space::new().height(Length::Fixed(8.0)),
            row![
                button(text("Relancer détection").size(12).color(colors::TEXT_ON_ACCENT))
                    .padding(8)
                    .style(|_, s| {
                        let mut st = button::Style::default();
                        st.background = Some(if s == button::Status::Hovered {
                            colors::ACCENT_HOVER.into()
                        } else {
                            colors::ACCENT.into()
                        });
                        st.text_color = colors::TEXT_ON_ACCENT;
                        st.border.radius = metrics::RADIUS_DROPDOWN.into();
                        st
                    })
                    .on_press(Message::DetectGpu),
                Space::new().width(Length::Fixed(8.0)),
                text("GIMP utilise GEGL + babl + tuiles 512×512, même approche ici")
                    .size(10)
                    .color(colors::TEXT_MUTED),
            ]
            .align_y(Alignment::Center),
            Space::new().height(Length::Fixed(8.0)),
            text("Note : Traitement nodal actuel CPU (rayon + tuiles) → textures Handle uploadées en VRAM via WGPU (pas de copie CPU persistante). Futur compute shader pour brightness/blur direct GPU.")
                .size(10)
                .color(colors::TEXT_SECONDARY),
        ],
    );

    let perf_section = section(
        "Performance",
        "Tuiles et cache comme GIMP/GEGL pour alléger le traitement",
        column![
            row![
                text("Taille tuile :").size(12).color(colors::ON_SURFACE),
                Space::new().width(Length::Fill),
                text("512×512 (5×3 tuiles sur 2560×1440)").size(11).color(colors::TEXT_SECONDARY),
            ],
            row![
                text("Zoom :").size(12).color(colors::ON_SURFACE),
                Space::new().width(Length::Fill),
                text(format!("{}% (pan/zoom GPU, canvas infini)", zoom_level))
                    .size(11)
                    .color(colors::TEXT_SECONDARY),
            ],
            row![
                text("Cache nœuds :").size(12).color(colors::ON_SURFACE),
                Space::new().width(Length::Fill),
                text("ancêtres Output uniquement (nœud déconnecté ignoré)").size(11).color(colors::TEXT_SECONDARY),
            ],
        ]
        .spacing(6),
    );

    let content = column![hardware_section, perf_section,]
        .spacing(16)
        .padding(16);

    let body = scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill);

    container(column![header, body].spacing(0))
        .width(Length::Fixed(640.0))
        .height(Length::Fixed(520.0))
        .style(|_| container::Style {
            background: Some(colors::BG_PANEL.into()),
            border: iced::Border {
                width: 1.0,
                color: colors::BORDER_PANEL,
                radius: 8.0.into(),
                ..Default::default()
            },
            shadow: iced::Shadow {
                color: colors::CABLE_SHADOW,
                offset: iced::Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..Default::default()
        })
        .into()
}

fn section<'a>(title: &'a str, subtitle: &'a str, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(
        column![
            text(title).size(14).color(colors::TEXT_PRIMARY),
            text(subtitle).size(11).color(colors::TEXT_MUTED),
            Space::new().height(Length::Fixed(8.0)),
            content.into(),
        ]
        .spacing(4),
    )
    .padding(12)
    .style(|_| container::Style {
        background: Some(colors::SURFACE_CONTAINER.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            radius: metrics::RADIUS_DROPDOWN.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .width(Length::Fill)
    .into()
}
