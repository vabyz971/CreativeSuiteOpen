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

//! Rendu de l'interface + abonnements (spinner, raccourcis clavier).

use iced::{Alignment, Element, Length, Subscription};

use crate::components;
use crate::menus::app_menus;
use crate::message::Message;
use crate::state::PhotoApp;

pub fn view(app: &PhotoApp, _window: iced::window::Id) -> Element<'_, Message> {
    let doc_size = app
        .doc_size
        .map(|(w, h)| iced::Size::new(w as f32, h as f32));
    // Contenu central : barre contextuelle (projet/zoom/export) + workspace
    let menus = app_menus(app.tools_visible, app.selected_layer);
    let menu_buttons = ui::menu::bar(&menus);

    // Bouton spinner façon Final Cut Pro : toujours visible, tourne pendant
    // un traitement en arrière-plan, clic → menu des tâches en cours
    let spinning = !app.background_tasks.is_empty();
    let spinner_btn = iced::widget::button(
        // Canvas 20 px centré dans un bouton 30 px sans padding → pas de crop
        iced::widget::container(ui::spinner::circle(
            if spinning { app.spinner_angle } else { 0.0 },
            20.0,
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(30.0))
    .padding(0)
    .style(|_, s| {
        let background: Option<iced::Color> = if s == iced::widget::button::Status::Hovered {
            Some(ui::theme::colors::HOVER_OVERLAY)
        } else {
            None
        };
        iced::widget::button::Style {
            background: background.map(iced::Background::Color),
            border: iced::Border {
                radius: ui::theme::metrics::RADIUS_DROPDOWN.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(Message::ToggleTaskMenu);

    let task_menu = {
        let items: Vec<iced::Element<'_, Message>> = if app.background_tasks.is_empty() {
            vec![
                iced::widget::container(
                    iced::widget::text("Aucun traitement en cours")
                        .size(12)
                        .color(ui::theme::colors::TEXT_MUTED),
                )
                .padding(iced::Padding::new(8.0).left(10.0).right(10.0))
                .into(),
            ]
        } else {
            app.background_tasks
                .iter()
                .map(|label| {
                    iced::widget::row![
                        iced::widget::text(label)
                            .size(12)
                            .color(ui::theme::colors::TEXT_PRIMARY),
                        iced::widget::Space::new().width(Length::Fill),
                        ui::spinner::circle(app.spinner_angle, 12.0),
                    ]
                    .align_y(Alignment::Center)
                    .into()
                })
                .collect()
        };
        iced::widget::container(iced::widget::column(items).spacing(2).padding(4))
            .width(Length::Fixed(240.0))
            .style(|_| iced::widget::container::Style {
                background: Some(ui::theme::colors::BG_DROPDOWN.into()),
                border: iced::Border {
                    width: 1.0,
                    color: ui::theme::colors::BORDER_SUBTLE,
                    radius: ui::theme::metrics::RADIUS_DROPDOWN.into(),
                },
                shadow: ui::theme::shadows::dropdown(),
                ..Default::default()
            })
    };

    let spinner = Some(
        iced_aw::DropDown::new(spinner_btn, task_menu, app.task_menu_open)
            .width(Length::Fixed(240.0))
            .alignment(iced_aw::drop_down::Alignment::BottomEnd)
            .on_dismiss(Message::ToggleTaskMenu)
            .into(),
    );

    // Barre d'options contextuelle : contenu selon l'outil sélectionné
    let selected_scale_percent = app.selected_layer.and_then(|id| {
        app.layers
            .iter()
            .find(|l| l.id == id)
            .map(|l| l.scale * 100.0)
    });
    let options_bar = components::options_bar::render(
        app.selected_tool,
        app.selected_layer,
        selected_scale_percent,
        app.canvas_selection.is_some(),
        app.brush_color,
        app.brush_size,
        app.brush_opacity,
        app.color_picker_open,
    );

    let central = iced::widget::column![
        components::toolbar::context_bar(app.image_path.as_deref()),
        options_bar,
        components::workspace::render(
            &app.panes,
            app.focus,
            &app.layers,
            &app.preview_cache,
            app.selected_layer,
            doc_size,
            app.fallback_handle.clone(),
            app.fallback_size,
            app.move_anchor.map(|(id, _, _)| id),
            app.drag_background.clone(),
            app.drag_background_size,
            app.image_path.clone(),
            app.image_error.clone(),
            app.selected_tool,
            app.tools_visible,
            app.canvas_pan,
            app.zoom_level,
            app.canvas_selection,
            app.color_profile.clone(),
            app.canvas_viewport,
            &app.gen_graph,
            app.gen_selected_node,
            &app.gen_previews,
            app.node_context_menu,
            app.node_context_world,
            ui::image_canvas::BrushStyle {
                color: [
                    (app.brush_color.r * 255.0).clamp(0.0, 255.0) as u8,
                    (app.brush_color.g * 255.0).clamp(0.0, 255.0) as u8,
                    (app.brush_color.b * 255.0).clamp(0.0, 255.0) as u8,
                ],
                radius: app.brush_size / 2.0,
                opacity: app.brush_opacity,
                erase: app.selected_tool == crate::message::Tool::Eraser,
            },
            app.pending_paint.as_ref().map(|p| p.tex.clone()),
            &app.new_doc_w,
            &app.new_doc_h,
            app.welcome_error.as_deref(),
        )
    ];
    // Shell : menus intégrés à la top bar — outils Photo en flottant sur le canvas
    let base_layout = ui::shell::minimalist_layout_menus_only(
        "Creative Suite Open Photo",
        menu_buttons,
        central,
        spinner,
    );

    // Modal Préférences unifiée (Général / Raccourcis clavier / À propos)
    if app.show_prefs {
        let prefs_overlay = iced::widget::stack![
            // Scrim : clic hors modal ferme
            iced::widget::mouse_area(
                iced::widget::container(
                    iced::widget::Space::new()
                        .width(Length::Fill)
                        .height(Length::Fill)
                )
                .style(|_| iced::widget::container::Style {
                    background: Some(ui::theme::colors::CABLE_SHADOW.into()),
                    ..Default::default()
                })
                .width(Length::Fill)
                .height(Length::Fill)
            )
            .on_press(Message::ClosePreferences),
            components::preferences::view(
                &app.shortcuts,
                app.capturing,
                app.prefs_section,
                app.gpu_info.clone(),
                app.gpu_available,
            ),
        ];
        return iced::widget::stack![base_layout, prefs_overlay].into();
    }

    // Les dropdowns des menus sont gérés nativement par iced_aw::DropDown
    base_layout
}

/// Tick d'animation uniquement pendant un chargement (spinner + barre)
pub fn subscription(app: &PhotoApp) -> Subscription<Message> {
    let tick = if !app.background_tasks.is_empty() {
        iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::TickFrame)
    } else {
        Subscription::none()
    };
    Subscription::batch([
        tick,
        ui::shortcuts::subscription(
            &app.shortcuts,
            app.capturing.is_some(),
            PhotoApp::message_for,
            Message::ShortcutCaptured,
        ),
    ])
}
