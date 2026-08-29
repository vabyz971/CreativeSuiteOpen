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
use iced::{Alignment, Element, Length, Subscription};

use crate::components;
use crate::menus::app_menus;
use crate::message::Message;
use crate::state::PhotoApp;

pub fn view(app: &PhotoApp, window: iced::window::Id) -> Element<'_, Message> {
    // Fenêtre OS des préférences : contenu dédié plein cadre
    if app.is_preferences_window(window) {
        if let Some(prefs) = &app.preferences_window {
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
    let menus = app_menus(app.tools_visible, app.selected_layer);
    let menu_buttons = ui_kit::menu::bar(&menus);

    // Bouton spinner façon Final Cut Pro : toujours visible, tourne pendant
    // un traitement en arrière-plan, clic → menu des tâches en cours
    let spinning = !app.background_tasks.is_empty();
    let spinner_btn = iced::widget::button(
        // Canvas 20 px centré dans un bouton 30 px sans padding → pas de crop
        iced::widget::center(ui_kit::spinner::circle(
            if spinning { app.spinner_angle } else { 0.0 },
            20.0,
        )),
    )
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(30.0))
    .padding(0)
    .style(|_, s| ui_kit::style::ghost(s))
    .on_press(Message::ToggleTaskMenu);

    let task_menu = {
        let items: Vec<iced::Element<'_, Message>> = if app.background_tasks.is_empty() {
            vec![
                iced::widget::container(
                    iced::widget::text("Aucun traitement en cours")
                        .size(12)
                        .color(ui_kit::theme::colors::TEXT_MUTED),
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
                            .color(ui_kit::theme::colors::TEXT_PRIMARY),
                        iced::widget::Space::new().width(Length::Fill),
                        ui_kit::spinner::circle(app.spinner_angle, 12.0),
                    ]
                    .align_y(Alignment::Center)
                    .into()
                })
                .collect()
        };
        iced::widget::container(iced::widget::column(items).spacing(2).padding(4))
            .width(Length::Fixed(240.0))
            .style(|_| {
                ui_kit::style::floating_card(
                    ui_kit::theme::colors::BG_DROPDOWN,
                    ui_kit::theme::metrics::RADIUS_DROPDOWN,
                    ui_kit::theme::shadows::dropdown(),
                )
            })
    };

    let spinner = Some(
        iced_aw::DropDown::new(spinner_btn, task_menu, app.task_menu_open)
            .width(Length::Fixed(240.0))
            .alignment(iced_aw::drop_down::Alignment::BottomEnd)
            .on_dismiss(Message::ToggleTaskMenu)
            .into(),
    );

    // Barre haute : Export + menu du tool à sa droite, sans fond
    let selected_scale_percent = app
        .selected_layer
        .and_then(|id| app.doc.pixel_layer(id).map(|l| l.transform.scale * 100.0));
    let context_bar = components::toolbar::context_bar(
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
        context_bar,
        components::workspace::render(
            &app.panes,
            app.focus,
            &app.doc,
            &app.preview_cache,
            app.selected_layer,
            app.dragged_layer,
            doc_size,
            app.fallback_handle.clone(),
            app.fallback_size,
            app.move_anchor.map(|(id, _)| id),
            app.drag_background.clone(),
            app.drag_background_size,
            app.image_path.clone(),
            app.image_error.clone(),
            app.selected_tool,
            app.brush_color,
            app.color_picker_open,
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
            ui_kit::image_canvas::BrushStyle {
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
    // Taille du document — titre du panel principal
    let doc_size_label: iced::Element<'_, Message> = if let Some((w, h)) = app.doc_dims() {
        iced::widget::container(
            iced::widget::text(format!("Document {}×{} px", w, h))
                .size(11)
                .color(ui_kit::theme::colors::TEXT_MUTED),
        )
        .padding(iced::Padding::new(3.0).left(8.0))
        .style(|_| iced::widget::container::Style {
            background: Some(ui_kit::theme::colors::SURFACE_CONTAINER_LOW.into()),
            ..Default::default()
        })
        .into()
    } else {
        iced::widget::Space::new()
            .height(iced::Length::Fixed(0.0))
            .into()
    };
    let central_with_title = iced::widget::column![doc_size_label, central];
    // Shell : menus intégrés à la top bar — outils Photo en flottant sur le canvas
    let base_layout = ui_kit::shell::minimalist_layout_menus_only(
        "Creative Suite Open Photo",
        menu_buttons,
        central_with_title,
        spinner,
    );

    // Dialogue redimensionnement document (Édition → Taille du document...)
    if app.resize_dialog_open {
        let dialog = iced::widget::container(
            iced::widget::column![
                iced::widget::text("Taille du document")
                    .size(16)
                    .color(ui_kit::theme::colors::TEXT_PRIMARY),
                iced::widget::row![
                    iced::widget::text("Largeur")
                        .size(12)
                        .width(iced::Length::Fixed(60.0)),
                    iced::widget::text_input("1920", &app.resize_w)
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
                    iced::widget::text_input("1080", &app.resize_h)
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
                            width: app.resize_w.parse::<u32>().unwrap_or(800),
                            height: app.resize_h.parse::<u32>().unwrap_or(600),
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
            background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.45).into()),
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
    let tick = if !app.background_tasks.is_empty() {
        iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::TickFrame)
    } else {
        Subscription::none()
    };
    let keyboard = iced::event::listen_with(keyboard_filter);
    let closes = iced::window::close_events().map(Message::WindowClosed);
    Subscription::batch([tick, keyboard, closes])
}

/// Filtre d'abonnement : uniquement les PRESSIONS de touches non consommées.
fn keyboard_filter(
    event: iced::Event,
    status: iced::event::Status,
    window: iced::window::Id,
) -> Option<Message> {
    match (&event, status) {
        (
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { .. }),
            iced::event::Status::Ignored,
        ) => Some(Message::Event { event, window }),
        _ => None,
    }
}
