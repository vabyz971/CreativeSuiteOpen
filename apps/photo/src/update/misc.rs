//! Handlers divers (préférences, canvas, hardware, fallback) — extrait de update/mod.rs
use crate::message::Message;
use crate::state::PhotoApp;
use iced::Task;
pub fn handle(_app: &mut PhotoApp, _msg: Message) -> Option<Task<Message>> {
    None
}
