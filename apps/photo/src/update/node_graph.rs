//! Handlers graphe nodal — extrait de update/mod.rs
#![allow(dead_code)]
use crate::message::Message;
use crate::state::PhotoApp;
use iced::Task;
pub fn handle(_app: &mut PhotoApp, _msg: Message) -> Option<Task<Message>> {
    None
}
