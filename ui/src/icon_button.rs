// CreativeSuiteOpen — Suite créative professionnelle open source
// Copyright (C) 2025 vabyz971
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

use crate::theme::{colors, metrics};
use iced::widget::{button, text};
use iced::{Color, Element, Font, Length, Theme};

/// Police Material Design - chargée au démarrage via `application.font(bytes)`
/// Codepoints Unicode Material Icons : https://fonts.google.com/icons
pub const MATERIAL_ICONS: Font = Font::with_name("Material Icons");
const SIZE_BUTTON: f32 = 36.0;

/// Bouton icône Affinity Canvas - sans fond, Material Icons pur
/// Vision Affinity : fond transparent, icône blanche, hover gris très léger, sélection accent
pub fn render<'a, Message>(
    icon_unicode: &'a str,
    _label: &'a str,
    selected: bool,
    on_press: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let icon = text(icon_unicode)
        .font(MATERIAL_ICONS)
        .size(22)
        .color(if selected {
            colors::TEXT_PRIMARY
        } else {
            colors::ON_SURFACE
        })
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);

    button(icon)
        .width(Length::Fixed(SIZE_BUTTON))
        .height(Length::Fixed(SIZE_BUTTON))
        .padding(0)
        .style(move |_theme: &Theme, status| {
            let mut s = button::Style::default();
            // Affinity : pas de fond par défaut, juste icône
            s.background = Some(if selected {
                colors::BG_PANEL_HEADER_FOCUSED.into()
            } else if status == button::Status::Hovered {
                colors::HOVER_OVERLAY.into()
            } else {
                Color::TRANSPARENT.into()
            });
            s.text_color = colors::TEXT_PRIMARY;
            s.border.radius = metrics::RADIUS_BUTTON.into();
            s.border.width = 0.0;
            s.border.color = Color::TRANSPARENT;
            s
        })
        .on_press(on_press)
        .into()
}
