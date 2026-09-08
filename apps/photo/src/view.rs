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

use iced::widget::container;
use iced::{Element, Length, Subscription};

use crate::components;
use crate::menus::app_menus;
use crate::message::Message;
use crate::state::PhotoApp;

pub fn view(app: &PhotoApp, window: iced::window::Id) -> Element<'_, Message> {
    // Fenêtre OS des préférences : contenu dédié plein cadre
    if app.is_preferences_window(window) {
        if let Some(prefs) = &app.windows.preferences_window {
            return container(prefs.view().map(Message::PreferencesMsg))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| iced::widget::container::Style {
                    background: Some(ui_kit::theme::colors::BG_APP.into()),
                    ..Default::default()
                })
                .into();
        }
        return iced::widget::container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    }

    let doc_size = app
        .doc_dims()
        .map(|(w, h)| iced::Size::new(w as f32, h as f32));
    // Contenu central : barre contextuelle (projet/zoom/export) + workspace
    let menus = app_menus(app.canvas.tools_visible, app.document.selected_layer);
    let menu_buttons = ui_kit::menu::bar(&menus);

    // Bouton spinner façon Final Cut Pro : toujours visible, tourne pendant
    // un traitement en arrière-plan, clic → menu des tâches en cours.
    // La primitive vit dans ui_kit::shell (réutilisable par vidéo/audio).
    let spinning = !app.rendering.background_tasks.is_empty();
    let spinner = Some(ui_kit::shell::task_indicator(
        spinning,
        app.rendering.spinner_angle,
        app.rendering.background_tasks.labels(),
        app.rendering.task_menu_open,
        Message::ToggleTaskMenu,
        Message::ToggleTaskMenu,
    ));

    // Barre haute : Export + menu du tool à sa droite, sans fond
    let selected_scale_percent = app.document.selected_layer.and_then(|id| {
        app.document
            .doc
            .pixel_layer(id)
            .map(|l| l.transform.scale_x * 100.0)
    });
    let context_bar = components::toolbar::context_bar(
        app.tools.selected_tool,
        app.document.selected_layer,
        selected_scale_percent,
        app.canvas.canvas_selection.is_some(),
        app.tools.brush_color,
        app.tools.brush_size,
        app.tools.brush_opacity,
        app.tools.color_picker_open,
    );

    let central = iced::widget::column![
        context_bar,
        components::workspace::render(
            &app.workspace.panes,
            app.workspace.focus,
            &app.document.doc,
            &app.rendering.preview_cache,
            app.document.selected_layer,
            app.tools.dragged_layer,
            app.tools.active_mask,
            &app.tools.expanded_fx_stack,
            app.tools.filter_menu_open,
            app.tools.context_menu_open,
            app.windows.preferences.general.layer_item_radius,
            app.tools.mask_brush_black,
            doc_size,
            app.rendering.fallback_handle.clone(),
            app.rendering.fallback_size,
            app.tools.move_anchor.map(|(id, _)| id),
            app.tools.move_anchor.map(|(_, t)| (t.offset_x, t.offset_y)),
            app.rendering.drag_background.clone(),
            app.rendering.drag_background_size,
            app.rendering.drag_layer_composite.clone(),
            app.rendering.drag_layer_composite_size,
            app.canvas.image_path.clone(),
            app.canvas.image_error.clone(),
            app.tools.selected_tool,
            app.tools.brush_color,
            app.tools.color_picker_open,
            app.canvas.tools_visible,
            app.canvas.canvas_pan,
            app.canvas.zoom_level,
            app.canvas.canvas_selection,
            app.canvas.color_profile.clone(),
            app.canvas.canvas_viewport,
            ui_kit::image_canvas::BrushStyle {
                color: [
                    (app.tools.brush_color.r * 255.0).clamp(0.0, 255.0) as u8,
                    (app.tools.brush_color.g * 255.0).clamp(0.0, 255.0) as u8,
                    (app.tools.brush_color.b * 255.0).clamp(0.0, 255.0) as u8,
                ],
                radius: app.tools.brush_size / 2.0,
                opacity: app.tools.brush_opacity,
                erase: app.tools.selected_tool == crate::message::Tool::Eraser,
            },
            app.tools.pending_paint.as_ref().map(|p| p.tex.clone()),
            app.tools.pick_loupe.clone(),
            &app.tools.new_doc_w,
            &app.tools.new_doc_h,
            app.tools.welcome_error.as_deref(),
        )
    ];
    let central_with_title = iced::widget::column![central];
    // Shell : menus intégrés à la top bar — outils Photo en flottant sur le canvas
    let base_layout = ui_kit::shell::minimalist_layout_menus_only(
        "Creative Suite Open Photo",
        menu_buttons,
        central_with_title,
        spinner,
    );

    // Dialogue redimensionnement document (Édition → Taille du document...)
    if app.tools.resize_dialog_open {
        let dialog = iced::widget::container(
            iced::widget::column![
                iced::widget::text("Taille du document")
                    .size(16)
                    .color(ui_kit::theme::colors::TEXT_PRIMARY),
                iced::widget::row![
                    iced::widget::text("Largeur")
                        .size(12)
                        .width(iced::Length::Fixed(60.0)),
                    iced::widget::text_input("1920", &app.tools.resize_w)
                        .on_input(Message::SetResizeWidth)
                        .width(iced::Length::Fixed(80.0)),
                    iced::widget::text("px").size(11),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                iced::widget::row![
                    iced::widget::text("Hauteur")
                        .size(12)
                        .width(iced::Length::Fixed(60.0)),
                    iced::widget::text_input("1080", &app.tools.resize_h)
                        .on_input(Message::SetResizeHeight)
                        .width(iced::Length::Fixed(80.0)),
                    iced::widget::text("px").size(11),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                iced::widget::row![
                    iced::widget::button(iced::widget::text("Annuler").size(12))
                        .on_press(Message::ShowResizeDialog)
                        .style(|_, s| ui_kit::style::ghost(s)),
                    iced::widget::button(iced::widget::text("Appliquer").size(12))
                        .on_press(Message::ResizeDocument {
                            width: app.tools.resize_w.parse::<u32>().unwrap_or(800),
                            height: app.tools.resize_h.parse::<u32>().unwrap_or(600),
                        })
                        .style(|_, s| ui_kit::style::primary(s)),
                ]
                .spacing(8),
            ]
            .spacing(12)
            .padding(16),
        )
        .width(iced::Length::Fixed(300.0))
        .style(|_| {
            ui_kit::style::floating_card(
                ui_kit::theme::colors::BG_DROPDOWN,
                ui_kit::theme::metrics::RADIUS_DROPDOWN,
                ui_kit::theme::shadows::dropdown(),
            )
        });
        let overlay = iced::widget::center(dialog).style(|_| iced::widget::container::Style {
            background: Some(ui_kit::theme::colors::SCRIM.into()),
            ..Default::default()
        });
        return iced::widget::stack![base_layout, overlay].into();
    }

    // Modal Préférences unifiée (Général / Raccourcis clavier / À propos)
    // Les dropdowns des menus sont gérés nativement par iced_aw::DropDown
    base_layout
}

/// Tick d'animation (spinner) + écoute clavier GLOBALE.
///
/// Le filtre `Status::Ignored` est la clé du comportement : une pression
/// de touche CONSUMÉE par un widget (champ texte en cours d'édition, par
/// exemple) n'atteint jamais le résolveur — plus besoin d'un flag
/// `text_input_focused` maintenu à la main.
pub fn subscription(app: &PhotoApp) -> Subscription<Message> {
    let tick = if !app.rendering.background_tasks.is_empty() {
        iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::TickFrame)
    } else {
        Subscription::none()
    };
    let keyboard = iced::event::listen_with(keyboard_filter);
    let closes = iced::window::close_events().map(Message::WindowClosed);
    Subscription::batch([tick, keyboard, closes])
}

/// Filtre d'abonnement : PRESSIONS et RELEASES non consommées.
fn keyboard_filter(
    event: iced::Event,
    status: iced::event::Status,
    window: iced::window::Id,
) -> Option<Message> {
    match (&event, status) {
        (
            iced::Event::Keyboard(
                iced::keyboard::Event::KeyPressed { .. }
                | iced::keyboard::Event::KeyReleased { .. },
            ),
            iced::event::Status::Ignored,
        ) => Some(Message::Event { event, window }),
        _ => None,
    }
}
