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

//! Barre d'outils verticale flottante : UNIQUEMENT la sélection d'outil.
//! Les réglages propres à chaque outil vivent dans la barre contextuelle
//! (`components::options_bar`, en haut sous la barre projet).

use crate::{Message, Tool};
use iced::widget::column;
use iced::{Alignment, Color, Element};
use ui_kit::icon_button;

// https://pictogrammers.com/library/mdi/ - Material Icons Regular
const ICON_PAN_TOOL: &str = "\u{e925}"; // pan_tool - Main
const ICON_ZOOM_IN: &str = "\u{e8ff}"; // zoom_in - Zoom
const ICON_SELECT: &str = "\u{F01BF}"; // select_all - Sélection
const ICON_MOVE: &str = "\u{e89f}"; // open_with - Déplacer
const ICON_BRUSH: &str = "\u{F00E3}"; // brush - Pinceau
/// Vérifié présent dans la cmap de MaterialIcons-Regular.ttf (format_color_reset).
/// La police classique n'a pas de glyphe « eraser » dédié.
const ICON_ERASER: &str = "\u{F01FE}";
const ICON_COLORIZE: &str = "\u{F020A}"; // colorize - Pipette

pub fn render<'a>(
    selected: Tool,
    brush_color: Color,
    picker_open: bool,
    mask_brush_black: bool,
) -> Element<'a, Message> {
    let swatch = iced::widget::button(
        iced::widget::container(
            iced::widget::Space::new()
                .width(iced::Length::Fixed(18.0))
                .height(iced::Length::Fixed(18.0)),
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
    .padding(4)
    .style(|_t, s| ui_kit::style::ghost(s))
    .on_press(Message::ToggleColorPicker);
    let global_color = iced_aw::widget::ColorPicker::new(
        picker_open,
        brush_color,
        swatch,
        Message::ToggleColorPicker,
        Message::SetBrushColor,
    );
    // Toggle noir/blanc pour la peinture de masque (façon Affinity).
    let mask_swatch = iced::widget::button(
        iced::widget::container(
            iced::widget::Space::new()
                .width(iced::Length::Fixed(18.0))
                .height(iced::Length::Fixed(18.0)),
        )
        .style(move |_| iced::widget::container::Style {
            background: Some(
                (if mask_brush_black {
                    iced::Color::BLACK
                } else {
                    iced::Color::WHITE
                })
                .into(),
            ),
            border: iced::Border {
                width: 2.0,
                color: if mask_brush_black {
                    iced::Color::WHITE
                } else {
                    iced::Color::BLACK
                },
                radius: ui_kit::theme::metrics::RADIUS_SM.into(),
            },
            ..Default::default()
        }),
    )
    .padding(4)
    .style(|_t, s| ui_kit::style::ghost(s))
    .on_press(Message::ToggleMaskColor);

    column![
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
            ICON_BRUSH,
            "Pinceau",
            selected == Tool::Brush,
            Message::SelectTool(Tool::Brush)
        ),
        icon_button::render(
            ICON_ERASER,
            "Gomme",
            selected == Tool::Eraser,
            Message::SelectTool(Tool::Eraser)
        ),
        icon_button::render(
            ICON_COLORIZE,
            "Pipette",
            selected == Tool::Eyedropper,
            Message::SelectTool(Tool::Eyedropper)
        ),
        iced::widget::container(
            iced::widget::Space::new()
                .width(iced::Length::Fill)
                .height(iced::Length::Fixed(1.0))
        )
        .style(|_| iced::widget::container::Style {
            background: Some(ui_kit::theme::colors::BORDER_PANEL.into()),
            ..Default::default()
        }),
        global_color,
        mask_swatch,
    ]
    .spacing(4)
    .align_x(Alignment::Center)
    .padding(6)
    .into()
}
