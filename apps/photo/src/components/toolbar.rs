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

use crate::Message;
use iced::widget::{button, row, text};
use iced::{Alignment, Element, Length, Padding};

pub fn context_bar() -> Element<'static, Message> {
    let material = ui::icon_button::MATERIAL_ICONS;

    // Bouton primaire Export (accent #007AFF)
    let export_btn = button(
        row![
            text("\u{e2c6}").font(material).size(16), // file_upload
            text("Exporter").size(13),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(6.0).left(14.0).right(14.0))
    .style(|_, s| ui::style::primary(s))
    .on_press(Message::MockAction);

    row![iced::widget::Space::new().width(Length::Fill), export_btn,]
        .align_y(Alignment::Center)
        .padding(Padding::new(5.0).left(8.0).right(8.0))
        .into()
}
