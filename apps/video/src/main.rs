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

use iced::{Element, Length, Task};

#[derive(Debug, Clone)]
enum Message {
    Mock,
}

struct VideoApp {
    zoom: f32,
}

impl Default for VideoApp {
    fn default() -> Self {
        Self { zoom: 1.0 }
    }
}

fn update(app: &mut VideoApp, msg: Message) -> Task<Message> {
    match msg {
        Message::Mock => {}
    }
    Task::none()
}

fn view(app: &VideoApp) -> Element<'_, Message> {
    let central = iced::widget::container("Working progess")
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    central
}

pub fn main() -> iced::Result {
    iced::application(VideoApp::default, update, view)
        .title("Creative Suite Open — Video")
        .font(include_bytes!(
            "../../../assets/fonts/MaterialIcons-Regular.ttf"
        ))
        .font(include_bytes!(
            "../../../assets/fonts/HankenGrotesk-Regular.ttf"
        ))
        .font(include_bytes!(
            "../../../assets/fonts/HankenGrotesk-SemiBold.ttf"
        ))
        .font(include_bytes!(
            "../../../assets/fonts/HankenGrotesk-Bold.ttf"
        ))
        .default_font(ui_kit::theme::fonts::SANS)
        .run()
}
