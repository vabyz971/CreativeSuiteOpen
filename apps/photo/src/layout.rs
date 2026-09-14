// Cygnus — Suite créative professionnelle open source
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

//! DISPOSITION photo : menu contextuel en haut, barre des modes et
//! paramètres d'outils (toute la largeur) juste en dessous, puis
//! 3 colonnes — rail d'outils à gauche, canvas central à onglets
//! (multi-documents), studio droit (propriétés + calques) — et barre
//! de statut basse.
//!
//! Le style vient exclusivement de ui-kit ; seule la disposition est
//! ici. Dessinable en headless (tests) comme sous eframe.

use crate::app::{PhotoApp, sample_preview_color};
use crate::ui::canvas::{PhotoCanvas, PhotoCanvasTool};
use crate::ui::dialogs::{draw_export_dialog, draw_help_dialog, draw_new_document_dialog};
use crate::ui::engine_bridge::PhotoEngineCommand;
use crate::ui::layers::{LayerPanelAction, draw_photo_layer_panel};
use crate::ui::menubar::{MenuAvailability, PhotoMenuAction, draw_menu_bar};
use crate::ui::modebar::draw_photo_modebar;
use crate::ui::properties::draw_photo_properties;
use crate::ui::toolbar::draw_tool_rail;
use ui_kit::dialogs::{CygnusModal, ModalAction, pick_image_to_open};
use ui_kit::panels::CygnusCollapsible;
use ui_kit::theme::tokens::CygnusTheme;
use ui_kit::theme::typography::body_text;
use ui_kit::widgets::dropdown::CygnusDropdown;
use ui_kit::widgets::icon::{CygnusIcon, icon_button};

/// Dessine un frame complet de l'app Photo.
pub fn draw_photo_layout(ui: &mut egui::Ui, app: &mut PhotoApp) {
    // 1. Menu contextuel haut (Fichier, Édition, Calque, Affichage, Aide).
    egui::Panel::top("photo_menubar")
        .resizable(false)
        .show(ui, |ui| {
            let availability = MenuAvailability {
                can_undo: app.active_doc().ui.can_undo,
                can_redo: app.active_doc().ui.can_redo,
                has_selection: app.active_doc().ui.selected.is_some(),
            };
            let actions = draw_menu_bar(ui, availability);
            let ctx = ui.ctx().clone();
            for action in actions {
                apply_menu_action(app, &ctx, action);
            }
        });

    // 2. Barre des modes + paramètres de l'outil (toute la largeur).
    egui::Panel::top("photo_modebar")
        .resizable(false)
        .show(ui, |ui| {
            let doc = app.active_doc_mut();
            let chosen = draw_photo_modebar(
                ui,
                &mut doc.ui.edit_mode,
                doc.ui.tool,
                &mut doc.ui.brush,
                &mut doc.ui.show_grid,
            );
            if let Some(mode) = chosen
                && !mode.supports(doc.ui.tool)
            {
                doc.ui.tool = PhotoCanvasTool::Move;
            }
        });

    // 3. Rail d'outils à gauche (icônes + tooltips, selon le mode).
    egui::Panel::left("photo_tools")
        .resizable(false)
        .exact_size(60.0)
        .show(ui, |ui| {
            let doc = app.active_doc_mut();
            draw_tool_rail(
                ui,
                &mut doc.ui.tool,
                &mut doc.ui.brush.color,
                doc.ui.edit_mode,
            );
        });
    // 4. Studio droit : propriétés (rangée 1) + calques (rangée 2).
    egui::Panel::right("photo_studio")
        .resizable(true)
        .default_size(300.0)
        .min_size(220.0)
        .show(ui, |ui| {
            draw_right_studio(ui, app);
        });

    // 5. Barre de statut basse (hints façon Affinity).
    egui::Panel::bottom("photo_status")
        .resizable(false)
        .show(ui, |ui| {
            draw_status_bar(ui, app);
        });

    // 6. Canvas central à onglets EN DERNIER.
    egui::CentralPanel::default().show(ui, |ui| {
        draw_doc_tabs(ui, app);
        draw_active_canvas(ui, app);
    });

    // 7. Modales (overlays) : filtre, nouveau document, export, aide.
    draw_filter_modal(ui, app);
    draw_new_document_overlay(ui, app);
    draw_export_overlay(ui, app);
    draw_help_dialog(ui.ctx(), &mut app.help_open);
}

/// Applique une action du menu (locale ou commande worker).
fn apply_menu_action(app: &mut PhotoApp, ctx: &egui::Context, action: PhotoMenuAction) {
    match action {
        PhotoMenuAction::NewDocument => app.new_doc_dialog.open(),
        PhotoMenuAction::OpenImage => {
            app.open_picker = Some(pick_image_to_open());
        }
        PhotoMenuAction::Export => app.export_dialog.open(),
        PhotoMenuAction::Quit => {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        PhotoMenuAction::Undo => {
            let _ = app.active_doc().tx.send(PhotoEngineCommand::Undo);
        }
        PhotoMenuAction::Redo => {
            let _ = app.active_doc().tx.send(PhotoEngineCommand::Redo);
        }
        PhotoMenuAction::AddEmptyLayer => {
            let _ = app.active_doc().tx.send(PhotoEngineCommand::AddEmptyLayer);
        }
        PhotoMenuAction::DuplicateLayer => {
            if let Some(id) = app.active_doc().ui.selected {
                let _ = app
                    .active_doc()
                    .tx
                    .send(PhotoEngineCommand::DuplicateLayer(id));
            }
        }
        PhotoMenuAction::AddMask => {
            if let Some(id) = app.active_doc().ui.selected {
                let _ = app
                    .active_doc()
                    .tx
                    .send(PhotoEngineCommand::AddMask { layer: id });
            }
        }
        PhotoMenuAction::DeleteLayer => {
            if let Some(id) = app.active_doc().ui.selected {
                let _ = app
                    .active_doc()
                    .tx
                    .send(PhotoEngineCommand::DeleteLayer(id));
            }
        }
        PhotoMenuAction::ToggleGrid => {
            let ui = &mut app.active_doc_mut().ui;
            ui.show_grid = !ui.show_grid;
        }
        PhotoMenuAction::ZoomIn => {
            app.active_doc_mut().ui.viewport.zoom_by(1.25, None);
        }
        PhotoMenuAction::ZoomOut => {
            app.active_doc_mut().ui.viewport.zoom_by(0.8, None);
        }
        PhotoMenuAction::ZoomReset => {
            app.active_doc_mut().ui.viewport.reset();
        }
        PhotoMenuAction::ShowHelp => {
            app.help_open = true;
        }
    }
}

/// Studio droit : propriétés du calque (rangée 1) + panneau calques
/// (rangée 2, avec barre de boutons en bas).
fn draw_right_studio(ui: &mut egui::Ui, app: &mut PhotoApp) {
    // Rangée 1 : propriétés du calque sélectionné.
    let mut props_open = app.props_open;
    let selected_id = app.active_doc().ui.selected;
    CygnusCollapsible::new("Proprietes").show(ui, &mut props_open, |ui| {
        let doc = app.active_doc_mut();
        let selected = selected_id.and_then(|id| doc.ui.layers.iter().find(|layer| layer.id == id));
        draw_photo_properties(ui, selected, &doc.tx);
    });
    app.props_open = props_open;
    ui.separator();

    // Rangée 2 : calques du document actif (remplit le reste).
    draw_layers_panel(ui, app);
}

/// Panneau calques : liste HUD + barre de boutons bas (image, vide,
/// dupliquer, masque, filtres, supprimer).
fn draw_layers_panel(ui: &mut egui::Ui, app: &mut PhotoApp) {
    // Liste bornée : réserve la barre de boutons en bas.
    let bar_height = 40.0;
    let list_height = (ui.available_height() - bar_height).max(80.0);
    let pending = ui
        .allocate_ui_with_layout(
            egui::vec2(ui.available_width(), list_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                let doc = app.active_doc_mut();
                // Le renommage en cours sur un calque disparu : annuler.
                if doc
                    .ui
                    .rename
                    .editing
                    .is_some_and(|id| !doc.ui.layers.iter().any(|layer| layer.id == id))
                {
                    doc.ui.rename.editing = None;
                }
                draw_photo_layer_panel(
                    ui,
                    &doc.ui.layers,
                    doc.ui.selected,
                    &mut doc.ui.rename,
                    &mut doc.ui.drag_state,
                    &doc.tx,
                )
            },
        )
        .inner;
    // Actions traitées hors emprunt du document.
    for action in pending {
        apply_layer_action(app, action);
    }
}

/// Applique une action du panneau calques (sélection locale,
/// renommage et mutations via le worker).
fn apply_layer_action(app: &mut PhotoApp, action: LayerPanelAction) {
    match action {
        LayerPanelAction::AddImage => {
            app.open_picker = Some(pick_image_to_open());
        }
        LayerPanelAction::AddEmpty => {
            let _ = app.active_doc().tx.send(PhotoEngineCommand::AddEmptyLayer);
        }
        LayerPanelAction::Duplicate => {
            if let Some(id) = app.active_doc().ui.selected {
                let _ = app
                    .active_doc()
                    .tx
                    .send(PhotoEngineCommand::DuplicateLayer(id));
            }
        }
        LayerPanelAction::Delete => {
            if let Some(id) = app.active_doc().ui.selected {
                let _ = app
                    .active_doc()
                    .tx
                    .send(PhotoEngineCommand::DeleteLayer(id));
            }
        }
        LayerPanelAction::AddMask => {
            if let Some(id) = app.active_doc().ui.selected {
                let _ = app
                    .active_doc()
                    .tx
                    .send(PhotoEngineCommand::AddMask { layer: id });
            }
        }
        LayerPanelAction::OpenFilterMenu => {
            app.filter_modal.open = app.active_doc().ui.selected.is_some();
            app.filter_modal.choice = 0;
        }
        LayerPanelAction::Select(id) => {
            app.active_doc_mut().ui.selected = Some(id);
        }
        LayerPanelAction::RenameCommit { layer, name } => {
            let _ = app
                .active_doc()
                .tx
                .send(PhotoEngineCommand::RenameLayer { layer, name });
        }
    }
}

/// Onglets de documents + boutons nouveau/fermer.
fn draw_doc_tabs(ui: &mut egui::Ui, app: &mut PhotoApp) {
    ui.horizontal(|ui| {
        for index in 0..app.doc_count() {
            let title = app.docs[index].title.clone();
            let selected = index == app.active;
            if ui.selectable_label(selected, title).clicked() {
                app.switch_tab(index);
            }
        }
        if icon_button(ui, CygnusIcon::Add, Some("Nouveau document")).clicked() {
            app.new_doc_dialog.open();
        }
        if icon_button(ui, CygnusIcon::Close, Some("Fermer le document")).clicked() {
            app.close_active_tab();
        }
    });
    ui.separator();
}

/// Canvas du document actif + échantillonnage pipette.
fn draw_active_canvas(ui: &mut egui::Ui, app: &mut PhotoApp) {
    let doc = app.active_doc_mut();
    let texture = doc.ui.texture_cache.texture();
    let outcome = PhotoCanvas::new(&mut doc.ui.stroke, &doc.tx)
        .texture(texture)
        .tool(doc.ui.tool)
        .show_grid(doc.ui.show_grid)
        .active_layer(doc.ui.selected)
        .brush(doc.ui.brush)
        .show(ui, &mut doc.ui.viewport);
    // Pipette : échantillonne le composite sous le curseur.
    if doc.ui.tool == PhotoCanvasTool::Eyedropper
        && let (Some(world), Some(preview)) =
            (outcome.pointer_world.last(), doc.ui.last_preview.as_ref())
        && let Some(color) = sample_preview_color(preview, world.x, world.y)
    {
        doc.ui.brush.color = color;
    }
    let _ = outcome;
}

/// Modale d'ajout de filtre (registre moteur statique).
fn draw_filter_modal(ui: &mut egui::Ui, app: &mut PhotoApp) {
    if !app.filter_modal.open {
        return;
    }
    let names: Vec<&str> = app
        .filter_types
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let mut open = true;
    let mut choice = app.filter_modal.choice;
    let ctx = ui.ctx().clone();
    let action =
        CygnusModal::new("Ajouter un filtre", "Ajouter", "Annuler").show(&ctx, &mut open, |ui| {
            CygnusDropdown::new("Filtre", &names).show(ui, &mut choice);
        });
    app.filter_modal.open = open;
    app.filter_modal.choice = choice;
    if action == Some(ModalAction::Confirm)
        && let (Some(id), Some((_, type_id))) = (
            app.active_doc().ui.selected,
            app.filter_types.get(app.filter_modal.choice),
        )
    {
        let _ = app.active_doc().tx.send(PhotoEngineCommand::AddFilter {
            layer: id,
            filter_type: type_id.clone(),
        });
    }
}

/// Fenêtre « Nouveau document » : validation = nouvel onglet.
fn draw_new_document_overlay(ui: &mut egui::Ui, app: &mut PhotoApp) {
    let ctx = ui.ctx().clone();
    if let Some((width, height)) = draw_new_document_dialog(&ctx, &mut app.new_doc_dialog) {
        app.open_sized_tab(width, height);
    }
}

/// Fenêtre « Exportation » : validation = export du document actif.
fn draw_export_overlay(ui: &mut egui::Ui, app: &mut PhotoApp) {
    let ctx = ui.ctx().clone();
    if let Some(request) = draw_export_dialog(&ctx, &mut app.export_dialog) {
        if let Some(parent) = request.path.parent()
            && !parent.as_os_str().is_empty()
        {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = app.active_doc().tx.send(PhotoEngineCommand::Export {
            path: request.path,
            quality: request.quality,
        });
    }
}

/// Barre de statut : hint de l'outil à gauche, doc/zoom à droite.
fn draw_status_bar(ui: &mut egui::Ui, app: &PhotoApp) {
    let theme = CygnusTheme::dark();
    let doc = app.active_doc();
    ui.horizontal(|ui| {
        ui.label(body_text(&theme, doc.ui.tool.hint()));
        if !doc.ui.status.is_empty() {
            ui.separator();
            ui.label(body_text(&theme, &doc.ui.status));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(body_text(
                &theme,
                &format!(
                    "{} · {} · zoom {}% · {} calques",
                    doc.title,
                    doc.ui.edit_mode.label(),
                    (doc.ui.viewport.zoom() * 100.0).round(),
                    doc.ui.layers.len()
                ),
            ));
        });
    });
}
