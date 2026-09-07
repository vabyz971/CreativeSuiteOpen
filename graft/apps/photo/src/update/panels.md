# apps/photo/src/update/panels.rs

- handle_toggle_task_menu · function · L25-L28 — fn handle_toggle_task_menu(app: &mut PhotoApp) -> Task<Message>
- handle_toggle_panel · function · L30-L55 — fn handle_toggle_panel(app: &mut PhotoApp, panel_type: PanelType) -> Task<Message>
- handle_open_preferences · function · L57-L78 — fn handle_open_preferences(app: &mut PhotoApp) -> Task<Message>
- handle_window_opened · function · L80-L83 — fn handle_window_opened(app: &mut PhotoApp, id: iced::window::Id) -> Task<Message>
- handle_window_closed · function · L85-L92 — fn handle_window_closed(app: &mut PhotoApp, id: iced::window::Id) -> Task<Message>
- handle_preferences_msg · function · L94-L119 — fn handle_preferences_msg(
- handle_pane_resized · function · L121-L124 — fn handle_pane_resized(app: &mut PhotoApp, split: pane_grid::Split, ratio: f32) -> Task<Message>
- handle_pane_dropped · function · L126-L133 — fn handle_pane_dropped(
- handle_pane_clicked · function · L135-L138 — fn handle_pane_clicked(app: &mut PhotoApp, pane: pane_grid::Pane) -> Task<Message>
- handle_close_pane · function · L140-L143 — fn handle_close_pane(app: &mut PhotoApp, pane: pane_grid::Pane) -> Task<Message>
- handle · function · L145-L164 — pub fn handle(app: &mut PhotoApp, msg: Message) -> Option<Task<Message>>
- handles · function · L167-L181 — pub fn handles(msg: &Message) -> bool
