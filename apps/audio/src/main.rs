use iced::{Element, Length, Task};

#[derive(Debug, Clone)]
enum Message {
    Mock,
}

struct AudioApp {
    zoom: f32,
}

impl Default for AudioApp {
    fn default() -> Self {
        Self { zoom: 1.0 }
    }
}

fn update(app: &mut AudioApp, msg: Message) -> Task<Message> {
    match msg {
        Message::Mock => {}
    }
    Task::none()
}

fn view(app: &AudioApp) -> Element<'_, Message> {
    let central = iced::widget::container("Working progess")
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    central
}

pub fn main() -> iced::Result {
    iced::application(AudioApp::default, update, view)
        .title("Creative Suite Open — Audio")
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
        .default_font(ui::theme::fonts::SANS)
        .run()
}
