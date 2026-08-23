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

//! Panneau Propriétés : réglages du calque sélectionné
//! (nom, opacité, mode de fusion, décalage, infos image).

use crate::layers::Layer;
use crate::Message;
use iced::widget::{column, container, row, scrollable, slider, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding};
use ui::theme::{colors, fonts};

pub fn render<'a>(layer: Option<&'a Layer>) -> Element<'a, Message> {
    let Some(layer) = layer else {
        return container(
            column![
                text("Propriétés").size(14).font(fonts::SANS_SEMIBOLD).color(colors::TEXT_PRIMARY),
                Space::new().height(Length::Fixed(12.0)),
                text("Aucun calque sélectionné")
                    .size(13)
                    .color(colors::TEXT_SECONDARY),
                Space::new().height(Length::Fixed(8.0)),
                text("Sélectionnez un calque dans le panneau Calques")
                    .size(11)
                    .color(colors::TEXT_MUTED),
            ]
            .padding(12),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    };

    let id = layer.id;

    let header = {
        let name_field: Element<'_, Message> = text_input("Nom du calque", &layer.name)
            .size(14)
            .padding(4)
            .on_input(move |s| Message::RenameLayer { id, name: s })
            .into();
        container(
            column![
                name_field,
                text(format!("Calque #{}", id)).size(11).color(colors::TEXT_MUTED),
            ]
            .spacing(2),
        )
    }
    .padding(12)
    .style(|_t| container::Style {
        background: Some(colors::BG_NODE_HEADER.into()),
        ..Default::default()
    });

    // Infos image du calque
    use image::GenericImageView as _;
    let (w, h) = layer.image.dimensions();
    let is_rgba8 = layer.image.as_rgba8().is_some();

    let path = "—";
    let info = column![
        row![
            text("Dimensions").size(11).color(colors::TEXT_MUTED),
            Space::new().width(Length::Fill),
            text(format!("{} × {} px", w, h))
                .size(11)
                .color(colors::TEXT_PRIMARY),
        ],
        row![
            text("Mode").size(11).color(colors::TEXT_MUTED),
            Space::new().width(Length::Fill),
            text(if is_rgba8 { "8-bit / canal · RGBA" } else { "16/32-bit" })
                .size(11)
                .color(colors::TEXT_PRIMARY),
        ],
        row![
            text("Source").size(11).color(colors::TEXT_MUTED),
            Space::new().width(Length::Fill),
            text(path.to_string()).size(11).color(colors::TEXT_SECONDARY),
        ],
    ]
    .spacing(4);

    let params = column![
        param_slider("Opacité", layer.opacity, 0.0..=100.0, 1.0, move |v| {
            Message::SetLayerOpacity { id, opacity: v }
        }),
        blend_mode_buttons(layer),
        offset_row("Décalage X", layer.offset_x, move |v| Message::SetLayerOffset {
            id,
            axis: crate::OffsetAxis::X,
            value: v
        }),
        offset_row("Décalage Y", layer.offset_y, move |v| Message::SetLayerOffset {
            id,
            axis: crate::OffsetAxis::Y,
            value: v
        }),
        Space::new().height(Length::Fixed(10.0)),
        info,
    ]
    .spacing(10);

    let content = column![
        header,
        container(
            column![
                text("Paramètres").size(12).color(colors::ON_SURFACE),
                Space::new().height(Length::Fixed(10.0)),
                params,
            ]
            .padding(12)
        ),
    ];

    scrollable(content).width(Length::Fill).height(Length::Fill).into()
}

fn blend_mode_buttons<'a>(layer: &'a Layer) -> Element<'a, Message> {
    let cur = layer.blend_mode.clone();
    let id = layer.id;
    let mode_btn = |label: &'a str| -> Element<'a, Message> {
        let is_sel = label == cur;
        iced::widget::button(text(label).size(11))
            .padding(Padding::new(4.0).left(8.0).right(8.0))
            .style(move |_theme: &iced::Theme, status| {
                let mut st = iced::widget::button::Style::default();
                if is_sel {
                    st.background = Some(colors::ACCENT.into());
                    st.text_color = colors::TEXT_ON_ACCENT;
                } else if status == iced::widget::button::Status::Hovered {
                    st.background = Some(colors::HOVER_OVERLAY.into());
                    st.text_color = colors::TEXT_PRIMARY;
                } else {
                    st.background = Some(colors::SURFACE_CONTAINER_HIGH.into());
                    st.text_color = colors::TEXT_SECONDARY;
                }
                st.border.radius = ui::theme::metrics::RADIUS_BUTTON.into();
                st
            })
            .on_press(Message::SetLayerBlend { id, mode: label.to_string() })
            .into()
    };
    column![
        text("Mode de fusion").size(12).color(colors::ON_SURFACE),
        row![mode_btn("Normal"), mode_btn("Multiply"), mode_btn("Screen")].spacing(6),
        row![mode_btn("Overlay"), mode_btn("Darken"), mode_btn("Lighten")].spacing(6),
    ]
    .spacing(6)
    .into()
}

fn offset_row<'a>(
    label: &'a str,
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label).size(11).color(colors::TEXT_MUTED),
        Space::new().width(Length::Fill),
        text_input("0", &format!("{:.0}", value))
            .size(11)
            .width(Length::Fixed(90.0))
            .on_input(move |s| {
                if let Ok(v) = s.parse::<f32>() {
                    on_change(v)
                } else {
                    Message::MockAction
                }
            }),
    ]
    .align_y(Alignment::Center)
    .spacing(6)
    .into()
}

fn param_slider<'a>(
    label: &'a str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        row![
            text(label).size(12).color(colors::ON_SURFACE),
            Space::new().width(Length::Fill),
            container(text(format!("{:.2}", value)).size(11).color(colors::TEXT_PRIMARY))
                .padding(4)
                .style(|_t| container::Style {
                    background: Some(colors::SURFACE_CONTAINER_HIGH.into()),
                    border: iced::Border {
                        radius: ui::theme::metrics::RADIUS_BUTTON.into(),
                        width: 1.0,
                        color: colors::BORDER_PANEL,
                    },
                    ..Default::default()
                })
        ]
        .align_y(Alignment::Center),
        slider(*range.start()..=*range.end(), value, on_change).step(step),
    ]
    .spacing(6)
    .into()
}
