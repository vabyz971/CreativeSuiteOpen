//! Handlers projet / image / document — extrait de update/mod.rs
//! TODO: migrer les arms NewProject, OpenProject, SaveProject, ExportImage, Image* etc.
use crate::message::Message;
use crate::state::PhotoApp;
use iced::Task;
pub fn handle(_app: &mut PhotoApp, _msg: Message) -> Option<Task<Message>> {
    None
}
