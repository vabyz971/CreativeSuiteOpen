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

//! Piano Roll FL Studio-like — widget minimaliste pour l'app Audio

use iced::widget::{column, container, row, text, Space};
use iced::{Alignment, Color, Element, Length};

pub fn view<'a, Message>() -> Element<'a, Message>
where
    Message: 'a,
{
    container(
        column![
            container(
                row![
                    text("Piano Roll").size(13).color(Color::WHITE),
                    Space::new().width(Length::Fill),
                    text("C4 • 120 BPM").size(11).color(Color::from_rgb(0.6, 0.6, 0.6)),
                ]
                .align_y(Alignment::Center)
            )
            .padding(8)
            .style(|_| container::Style {
                background: Some(Color::from_rgb(0.11, 0.11, 0.11).into()),
                ..Default::default()
            }),
            container(
                column![
                    text("FL Studio • Channel Rack / Mixer").size(12).color(Color::from_rgb(0.7,0.7,0.7)),
                    Space::new().height(Length::Fixed(12.0)),
                    text("Notes, automation, mixer — même shell que Photo/Vidéo").size(11).color(Color::from_rgb(0.5,0.5,0.5)),
                ]
                .padding(16)
                .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| container::Style {
                background: Some(Color::from_rgb(0.09, 0.09, 0.09).into()),
                ..Default::default()
            }),
        ]
        .spacing(4),
    )
    .padding(8)
    .style(|_| container::Style {
        background: Some(Color::from_rgb(0.09, 0.09, 0.09).into()),
        ..Default::default()
    })
    .into()
}
