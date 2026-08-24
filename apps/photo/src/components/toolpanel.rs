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
use iced::{Alignment, Element};
use ui::icon_button;
use ui::theme::colors;

// Codepoints Material Icons - pas d'emoji en dur, police professionnelle
// Voir https://fonts.google.com/icons - Material Icons Regular
const ICON_PAN_TOOL: &str = "\u{e925}"; // pan_tool - Main
const ICON_ZOOM_IN: &str = "\u{e8ff}"; // zoom_in - Zoom
const ICON_SELECT: &str = "\u{e86e}"; // select_all - Sélection
const ICON_COLORIZE: &str = "\u{e3b7}"; // colorize - Pipette
const ICON_MOVE: &str = "\u{e89f}"; // open_with - Déplacer

// Barre d'outils verticale unifiée - Material Design Icons natifs
pub fn render<'a>(
    selected: Tool,
    selected_layer: Option<u64>,
    has_selection: bool,
) -> Element<'a, Message> {
    let action_btn =
        |codepoint: &'a str, _tip: &'a str, msg: Message, enabled: bool| -> Element<'a, Message> {
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

    let col = column![
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
    ]
    .spacing(6)
    .align_x(Alignment::Center)
    .padding(8);

    col.into()
}
