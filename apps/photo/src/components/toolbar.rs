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

//! Barre contextuelle haute : actions globales de droite (Export).
//! Le nom du fichier/document est porté par le titre du panneau Canvas —
//! pas de sélecteur redondant ici.

use crate::{Message, Tool};
use iced::widget::{button, row, text};
use iced::{Alignment, Element, Length, Padding};
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub fn context_bar<'a>(
    selected_tool: Tool,
    selected_layer: Option<Uuid>,
    selected_scale_percent: Option<f32>,
    has_selection: bool,
    brush_color: iced::Color,
    brush_size: f32,
    brush_opacity: f32,
    color_picker_open: bool,
) -> Element<'a, Message> {
    let material = ui_kit::icon_button::MATERIAL_ICONS;

    // Bouton primaire Export — couleur via ui_kit::theme::colors::ACCENT (#007AFF)
    let export_btn = button(
        row![
            text("\u{e2c6}").font(material).size(16), // file_upload
            text("Exporter").size(13),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(6.0).left(14.0).right(14.0))
    .style(|_, s| ui_kit::style::primary(s))
    .on_press(Message::MockAction);

    // Sélecteur couleur global — visible pour tous les tools
    let swatch = iced::widget::button(
        iced::widget::container(
            iced::widget::Space::new()
                .width(Length::Fixed(18.0))
                .height(Length::Fixed(18.0)),
        )
        .style(move |_| iced::widget::container::Style {
            background: Some(brush_color.into()),
            border: iced::Border {
                width: 1.0,
                color: ui_kit::theme::colors::BORDER_SUBTLE,
                radius: ui_kit::theme::metrics::RADIUS_SM.into(),
            },
            ..Default::default()
        }),
    )
    .padding(3)
    .style(|_t, s| ui_kit::style::ghost(s))
    .on_press(Message::ToggleColorPicker);
    let global_color = iced_aw::widget::ColorPicker::new(
        color_picker_open,
        brush_color,
        swatch,
        Message::ToggleColorPicker,
        Message::SetBrushColor,
    );

    // Menu du tool à gauche d'Exporter, sans fond — juste les btn/sliders
    if let Some(controls) = crate::components::options_bar::tool_controls(
        selected_tool,
        selected_layer,
        selected_scale_percent,
        has_selection,
        brush_color,
        brush_size,
        brush_opacity,
        color_picker_open,
    ) {
        row![
            controls,
            iced::widget::Space::new().width(Length::Fill),
            global_color,
            export_btn,
        ]
        .align_y(Alignment::Center)
        .padding(Padding::new(5.0).left(8.0).right(8.0))
        .into()
    } else {
        row![
            iced::widget::Space::new().width(Length::Fill),
            global_color,
            export_btn,
        ]
        .align_y(Alignment::Center)
        .padding(Padding::new(5.0).left(8.0).right(8.0))
        .into()
    }
}
