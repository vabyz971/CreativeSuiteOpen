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

use crate::Message;
use iced::widget::{button, row, text};
use iced::{Alignment, Element, Length, Padding};
use ui::theme::{colors, metrics};

// --- BARRE CONTEXTUELLE : sélecteur de projet | zoom | Export ---
// (les menus Fichier/Édition/Affichage vivent désormais dans le shell, voir ui::menu)
pub fn context_bar<'a>(project_name: Option<&'a str>) -> Element<'a, Message> {
    let material = ui::icon_button::MATERIAL_ICONS;

    // Sélecteur de projet : dossier + nom du fichier + chevron
    let name = project_name.unwrap_or("Sans titre");
    let project_selector = button(
        row![
            text("\u{e2c8}")
                .font(material)
                .size(16)
                .color(colors::TEXT_SECONDARY), // folder
            text(name)
                .size(12)
                .font(ui::theme::fonts::SANS_SEMIBOLD)
                .color(colors::TEXT_PRIMARY),
            text("\u{e313}")
                .font(material)
                .size(16)
                .color(colors::TEXT_MUTED), // expand_more
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(5.0).left(10.0).right(6.0))
    .style(|_theme, status| {
        let mut st = button::Style::default();
        st.background = Some(if status == button::Status::Hovered {
            colors::SURFACE_CONTAINER_HIGH.into()
        } else {
            colors::SURFACE_CONTAINER.into()
        });
        st.border.radius = metrics::RADIUS_DROPDOWN.into();
        st.border.width = 1.0;
        st.border.color = colors::BORDER_SUBTLE;
        st.text_color = colors::TEXT_PRIMARY;
        st
    })
    .on_press(Message::OpenImage);

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
    .style(|_theme, status| {
        let mut st = button::Style::default();
        st.background = Some(if status == button::Status::Hovered {
            colors::ACCENT_HOVER.into()
        } else {
            colors::ACCENT.into()
        });
        st.border.radius = metrics::RADIUS_DROPDOWN.into();
        st.text_color = colors::TEXT_ON_ACCENT;
        st
    })
    .on_press(Message::MockAction);

    row![
        project_selector,
        iced::widget::Space::new().width(Length::Fill),
        export_btn,
    ]
    .align_y(Alignment::Center)
    .padding(Padding::new(5.0).left(8.0).right(8.0))
    .into()
}
