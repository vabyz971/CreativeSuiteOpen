// Cygnus — Suite créative professionnelle open source
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

//! Sélecteur de studio (Pixel / Vecteur / Mise en page) : rangée de 3
//! boutons posée dans la top bar, même pattern que `toolpanel` (boutons
//! `ui_kit::icon_button::render` + état sélectionné).

use crate::{Message, StudioMode};
use iced::Element;
use ui_kit::icon_button;

// Codepoints Material vérifiés présents dans assets/fonts/
// (photo, gesture, dashboard) via fc-query.
const ICON_PIXEL: &str = "\u{e410}"; // photo - Retouche pixel
const ICON_VECTOR: &str = "\u{e155}"; // gesture - Illustration vectorielle
const ICON_LAYOUT: &str = "\u{e871}"; // dashboard - Mise en page

pub fn render<'a>(actif: StudioMode) -> Element<'a, Message> {
    iced::widget::row![
        icon_button::render(
            ICON_PIXEL,
            "Pixel",
            actif == StudioMode::Pixel,
            Message::SetStudioMode(StudioMode::Pixel)
        ),
        icon_button::render(
            ICON_VECTOR,
            "Vecteur",
            actif == StudioMode::Vector,
            Message::SetStudioMode(StudioMode::Vector)
        ),
        icon_button::render(
            ICON_LAYOUT,
            "Mise en page",
            actif == StudioMode::Layout,
            Message::SetStudioMode(StudioMode::Layout)
        ),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center)
    .into()
}
