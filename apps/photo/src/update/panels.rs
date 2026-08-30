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

//! Panel / layout / preferences window handlers — extracted from update/mod.rs

use iced::Task;
use iced::widget::pane_grid;

use crate::message::{Message, PanelType};
use crate::state::PhotoApp;

fn handle_toggle_task_menu(app: &mut PhotoApp) -> Task<Message> {
    app.task_menu_open = !app.task_menu_open;
    Task::none()
}

fn handle_toggle_panel(app: &mut PhotoApp, panel_type: PanelType) -> Task<Message> {
    let existing_pane = app
        .panes
        .iter()
        .find(|(_, p)| **p == panel_type)
        .map(|(pane, _)| *pane);

    if let Some(pane) = existing_pane {
        app.panes.close(pane);
    } else {
        let target_canvas_pane = app
            .panes
            .iter()
            .find(|(_, p)| **p == PanelType::Canvas)
            .map(|(p, _)| *p);

        if let Some(canvas_pane) = target_canvas_pane {
            app.panes
                .split(pane_grid::Axis::Vertical, canvas_pane, panel_type);
        }
    }
    Task::none()
}

fn handle_open_preferences(app: &mut PhotoApp) -> Task<Message> {
    // Already open: give it back the focus (pro behavior)
    if let Some(id) = app.preferences_window_id {
        return iced::window::gain_focus(id);
    }
    app.preferences_window = Some(crate::preferences_window::PreferencesWindow::new(
        app.preferences.clone(),
    ));
    let (_, open) = iced::window::open(iced::window::Settings {
        size: iced::Size::new(780.0, 580.0),
        min_size: Some(iced::Size::new(620.0, 460.0)),
        resizable: true,
        exit_on_close_request: false,
        ..iced::window::Settings::default()
    });
    // Hardware detection OFF the UI thread for the Hardware section
    let detect = Task::perform(
        async { preferences::HardwareReport::detect().await },
        Message::HardwareDetected,
    );
    Task::batch([open.map(Message::WindowOpened), detect])
}

fn handle_window_opened(app: &mut PhotoApp, id: iced::window::Id) -> Task<Message> {
    app.preferences_window_id = Some(id);
    Task::none()
}

fn handle_window_closed(app: &mut PhotoApp, id: iced::window::Id) -> Task<Message> {
    // OS close button: purge the associated state
    if app.is_preferences_window(id) {
        app.preferences_window = None;
        app.preferences_window_id = None;
    }
    Task::none()
}

fn handle_preferences_msg(
    app: &mut PhotoApp,
    msg: crate::preferences_window::Message,
) -> Task<Message> {
    use crate::preferences_window::Message as PrefsMsg;
    match msg {
        PrefsMsg::Close | PrefsMsg::SaveAndClose => {
            if let Some(window) = &mut app.preferences_window {
                window.update(msg.clone());
                if matches!(msg, PrefsMsg::SaveAndClose) {
                    app.preferences = window.draft.clone();
                    app.resolver = preferences::KeybindingResolver::from_bindings(
                        &window.draft.keybindings.bindings,
                    );
                }
            }
            app.close_preferences_window()
        }
        inner => {
            if let Some(window) = &mut app.preferences_window {
                window.update(inner);
            }
            Task::none()
        }
    }
}

fn handle_pane_resized(app: &mut PhotoApp, split: pane_grid::Split, ratio: f32) -> Task<Message> {
    app.panes.resize(split, ratio);
    Task::none()
}

fn handle_pane_dropped(
    app: &mut PhotoApp,
    pane: pane_grid::Pane,
    target: pane_grid::Target,
) -> Task<Message> {
    app.panes.drop(pane, target);
    Task::none()
}

fn handle_pane_clicked(app: &mut PhotoApp, pane: pane_grid::Pane) -> Task<Message> {
    app.focus = Some(pane);
    Task::none()
}

fn handle_close_pane(app: &mut PhotoApp, pane: pane_grid::Pane) -> Task<Message> {
    app.panes.close(pane);
    Task::none()
}

pub fn handle(app: &mut PhotoApp, msg: Message) -> Option<Task<Message>> {
    match msg {
        Message::ToggleTaskMenu => Some(handle_toggle_task_menu(app)),
        Message::TogglePanel(panel_type) => Some(handle_toggle_panel(app, panel_type)),
        Message::OpenPreferences => Some(handle_open_preferences(app)),
        Message::PreferencesMsg(msg) => Some(handle_preferences_msg(app, msg)),
        Message::WindowOpened(id) => Some(handle_window_opened(app, id)),
        Message::WindowClosed(id) => Some(handle_window_closed(app, id)),
        Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
            Some(handle_pane_resized(app, split, ratio))
        }
        Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
            Some(handle_pane_dropped(app, pane, target))
        }
        Message::PaneDragged(_) => Some(Task::none()),
        Message::PaneClicked(pane) => Some(handle_pane_clicked(app, pane)),
        Message::ClosePane(pane) => Some(handle_close_pane(app, pane)),
        _ => None,
    }
}
