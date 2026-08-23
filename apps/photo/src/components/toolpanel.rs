use crate::{Message, Tool};
use iced::widget::{column, container};
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

// Barre d'outils verticale unifiée - Material Design Icons natifs
pub fn render<'a>(selected: Tool) -> Element<'a, Message> {
    column![
        icon_button::render(ICON_PAN_TOOL, "Main", selected == Tool::Hand, Message::SelectTool(Tool::Hand)),
        icon_button::render(ICON_ZOOM_IN, "Zoom", selected == Tool::Zoom, Message::SelectTool(Tool::Zoom)),
        icon_button::render(ICON_SELECT, "Sélect", selected == Tool::Select, Message::SelectTool(Tool::Select)),
        icon_button::render(ICON_MOVE, "Déplacer", selected == Tool::Move, Message::SelectTool(Tool::Move)),
        separator(),
        icon_button::render(ICON_COLORIZE, "Pipette", selected == Tool::Eyedropper, Message::SelectTool(Tool::Eyedropper)),
    ]
    .spacing(6)
    .align_x(Alignment::Center)
    .padding(8)
    .into()
}

fn separator<'a>() -> Element<'a, Message> {
    // Largeur FIXE : un Fill étirerait toute la pastille flottante
    container(iced::widget::Space::new().height(Length::Fixed(1.0)).width(Length::Fixed(20.0)))
        .padding(iced::Padding::new(4.0))
        .style(|_| container::Style {
            background: Some(colors::BORDER_PANEL.into()),
            ..Default::default()
        })
        .into()
}
