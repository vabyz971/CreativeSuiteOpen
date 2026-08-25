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

use iced::widget::{button, text};
use iced::{Element, Font, Length};

/// Police Material Design - chargée au démarrage via `application.font(bytes)`
/// Codepoints Unicode Material Icons : https://fonts.google.com/icons
pub const MATERIAL_ICONS: Font = Font::with_name("Material Icons");
const SIZE_BUTTON: f32 = 32.0;

/// Bouton icône de palette flottante — style macOS : sans fond, icône
/// discrète au repos, éclaircie au survol, sélection teintée accent.
pub fn render<'a, Message>(
    icon_unicode: &'a str,
    _label: &'a str,
    selected: bool,
    on_press: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    // Pas de .color() explicite : `style::tool_button` pilote la teinte
    // selon l'état (repos discret / survol / sélection).
    let icon = text(icon_unicode)
        .font(MATERIAL_ICONS)
        .size(20)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);

    button(icon)
        .width(Length::Fixed(SIZE_BUTTON))
        .height(Length::Fixed(SIZE_BUTTON))
        .padding(0)
        .style(move |_theme: &iced::Theme, status| {
            // Palette macOS : teinte accent si sélectionné
            crate::style::tool_button(selected, status)
        })
        .on_press(on_press)
        .into()
}
