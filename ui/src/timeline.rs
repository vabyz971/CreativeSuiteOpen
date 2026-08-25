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

//! Timeline Final Cut-like — widget minimaliste pour l'app Vidéo
//! Réutilise le même shell et le même moteur de graphe que Photo.

use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Color, Element, Length};

/// Barre de timeline horizontale (pistes + clips)
pub fn view<'a, Message>() -> Element<'a, Message>
where
    Message: 'a,
{
    let track = |label: &'static str, color: Color| {
        container(
            row![
                container(
                    text(label)
                        .size(11)
                        .color(Color::from_rgb(0.75, 0.75, 0.75))
                )
                .width(Length::Fixed(80.0))
                .padding(6),
                container(
                    Space::new()
                        .width(Length::Fixed(120.0))
                        .height(Length::Fixed(28.0))
                )
                .style(move |_| container::Style {
                    background: Some(color.into()),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .padding(4),
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center),
        )
        .padding(4)
        .style(|_| container::Style {
            background: Some(Color::from_rgb(0.13, 0.13, 0.13).into()),
            border: iced::Border {
                width: 1.0,
                color: Color::from_rgb(0.20, 0.20, 0.20),
                radius: 4.0.into(),
            },
            ..Default::default()
        })
    };

    container(
        column![
            container(
                row![
                    text("Timeline").size(13).color(Color::WHITE),
                    Space::new().width(Length::Fill),
                    text("00:12:34:08")
                        .size(11)
                        .color(Color::from_rgb(0.6, 0.6, 0.6)),
                ]
                .align_y(Alignment::Center)
            )
            .padding(8)
            .style(|_| container::Style {
                background: Some(Color::from_rgb(0.11, 0.11, 0.11).into()),
                ..Default::default()
            }),
            track("V1", Color::from_rgb(0.25, 0.45, 0.75)),
            track("V2", Color::from_rgb(0.45, 0.35, 0.65)),
            track("A1", Color::from_rgb(0.20, 0.55, 0.35)),
            container(Space::new().height(Length::Fixed(24.0)))
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Color::from_rgb(0.10, 0.10, 0.10).into()),
                    ..Default::default()
                }),
        ]
        .spacing(6),
    )
    .padding(8)
    .style(|_| container::Style {
        background: Some(Color::from_rgb(0.09, 0.09, 0.09).into()),
        ..Default::default()
    })
    .into()
}
