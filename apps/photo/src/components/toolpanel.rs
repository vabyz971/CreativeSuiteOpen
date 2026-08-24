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

use crate::{Message, Tool};
use iced::widget::column;
use iced::{Alignment, Element, Length};
use ui::icon_button;
use ui::theme::colors;

// Codepoints Material Icons - pas d'emoji en dur, police professionnelle
// Voir https://fonts.google.com/icons - Material Icons Regular
const ICON_PAN_TOOL: &str = "\u{e925}"; // pan_tool - Main
const ICON_ZOOM_IN: &str = "\u{e8ff}"; // zoom_in - Zoom
const ICON_SELECT: &str = "\u{e86e}"; // select_all - Sélection
const ICON_COLORIZE: &str = "\u{e3b7}"; // colorize - Pipette
const ICON_MOVE: &str = "\u{e89f}"; // open_with - Déplacer
const ICON_ROTATE_LEFT: &str = "\u{e419}"; // rotate_left
const ICON_ROTATE_RIGHT: &str = "\u{e41a}"; // rotate_right
const ICON_CROP: &str = "\u{e3be}"; // crop
const ICON_RESET: &str = "\u{e166}"; // restart_alt

// Barre d'outils verticale unifiée - Material Design Icons natifs
pub fn render<'a>(
    selected: Tool,
    selected_layer: Option<u64>,
    has_selection: bool,
) -> Element<'a, Message> {
    let layer_id = selected_layer.unwrap_or(0);
    let has_layer = selected_layer.is_some();

    let action_btn =
        |codepoint: &'a str, msg: Message, enabled: bool| -> Element<'a, Message> {
            let b = iced::widget::button(
                iced::widget::text(codepoint)
                    .font(ui::icon_button::MATERIAL_ICONS)
                    .size(16)
                    .color(if enabled {
                        colors::TEXT_SECONDARY
                    } else {
                        colors::TEXT_MUTED
                    }),
            )
            .padding(4);
            let b = if enabled {
                b.on_press(msg).style(move |_t, s| {
                    let mut st = iced::widget::button::Style::default();
                    st.background = Some(if s == iced::widget::button::Status::Hovered {
                        colors::HOVER_OVERLAY.into()
                    } else {
                        iced::Color::TRANSPARENT.into()
                    });
                    st.border.radius = ui::theme::metrics::RADIUS_BUTTON.into();
                    st
                })
            } else {
                b.style(|_t, _s| iced::widget::button::Style::default())
            };
            b.into()
        };

    let mut col = column![
        icon_button::render(
            ICON_PAN_TOOL,
            "Main",
            selected == Tool::Hand,
            Message::SelectTool(Tool::Hand)
        ),
        icon_button::render(
            ICON_ZOOM_IN,
            "Zoom",
            selected == Tool::Zoom,
            Message::SelectTool(Tool::Zoom)
        ),
        icon_button::render(
            ICON_SELECT,
            "Sélect",
            selected == Tool::Select,
            Message::SelectTool(Tool::Select)
        ),
        icon_button::render(
            ICON_MOVE,
            "Déplacer",
            selected == Tool::Move,
            Message::SelectTool(Tool::Move)
        ),
        icon_button::render(
            ICON_COLORIZE,
            "Pipette",
            selected == Tool::Eyedropper,
            Message::SelectTool(Tool::Eyedropper)
        ),
        separator(),
    ]
    .spacing(6)
    .align_x(Alignment::Center)
    .padding(8);

    // Transformations du calque sélectionné
    col = col.push(action_btn(
        ICON_ROTATE_LEFT,
        Message::RotateLayer90 { id: layer_id, clockwise: false },
        has_layer,
    ));
    col = col.push(action_btn(
        ICON_ROTATE_RIGHT,
        Message::RotateLayer90 { id: layer_id, clockwise: true },
        has_layer,
    ));
    col = col.push(action_btn(
        ICON_RESET,
        Message::ResetLayerTransform(layer_id),
        has_layer,
    ));
    col = col.push(action_btn(
        ICON_CROP,
        Message::CropLayerToSelection,
        has_layer && has_selection,
    ));

    col.into()
}

fn separator<'a>() -> Element<'a, Message> {
    // Largeur FIXE : un Fill étirerait toute la pastille flottante
    iced::widget::container(iced::widget::Space::new().height(Length::Fixed(1.0)).width(Length::Fixed(20.0)))
        .padding(iced::Padding::new(4.0))
        .style(|_| iced::widget::container::Style {
            background: Some(colors::BORDER_PANEL.into()),
            ..Default::default()
        })
        .into()
}
