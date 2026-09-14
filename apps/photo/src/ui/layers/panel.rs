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

//! Panneau des calques de l'app PHOTO (widget métier).
//!
//! Délègue toute la mécanique de drag & drop à
//! `ui_kit::ReorderableList` (générique, niveau racine) et ne contient
//! que le rendu photo (voir [`super::item`]) plus le fantôme de drag
//! (nom du calque déplacé). Le drop est relayé au worker moteur
//! (`ReorderLayer`, indices d'affichage) via le channel. Les
//! sous-couches (filtres, masques) se réordonnent par boutons
//! monter/descendre dans chaque HUD ; les groupes suivront le même
//! DnD imbriqué plus tard.
//!
//! Le conteneur titré (`CygnusPanel`) est posé par le layout de
//! l'app, pas ici.

use super::item::{LayerItemAction, LayerRenameState, draw_photo_layer_item};
use super::types::PhotoLayerInfo;
use crate::ui::PhotoEngineCommand;
use std::sync::mpsc::Sender;
use ui_kit::theme::tokens::CygnusTheme;
use ui_kit::utils::ReorderDragState;
use ui_kit::widgets::ReorderableList;
use ui_kit::widgets::icon::{CygnusIcon, icon_button};
use uuid::Uuid;

/// Hauteur d'une ligne de calque HUD (rangée principale + bandeau
/// sous-couches, virtualisation à hauteur fixe).
pub const PHOTO_LAYER_ITEM_HEIGHT: f32 = 76.0;

/// Action de la barre de boutons du panneau calques + interactions
/// des HUD (sélection, renommage).
#[derive(Debug, Clone, PartialEq)]
pub enum LayerPanelAction {
    /// Ajouter une image (file picker).
    AddImage,
    /// Nouveau calque vide.
    AddEmpty,
    /// Dupliquer la sélection.
    Duplicate,
    /// Ouvrir le menu d'ajout de filtre.
    OpenFilterMenu,
    /// Ajouter un masque au calque sélectionné.
    AddMask,
    /// Supprimer la sélection.
    Delete,
    /// Sélectionner un calque (clic sur sa rangée).
    Select(Uuid),
    /// Valider le renommage d'un calque.
    RenameCommit { layer: Uuid, name: String },
}

/// Dessine le fantôme semi-transparent qui suit la souris pendant le drag.
fn draw_drag_ghost(ui: &mut egui::Ui, drag_state: &ReorderDragState, layers: &[PhotoLayerInfo]) {
    if !drag_state.is_dragging {
        return;
    }
    let Some(dragging) = drag_state.dragging_index else {
        return;
    };
    let Some(layer) = layers.get(dragging) else {
        return;
    };
    let Some(pointer) = ui.ctx().pointer_latest_pos() else {
        return;
    };
    let theme = CygnusTheme::dark();
    let rect = egui::Rect::from_min_size(
        egui::pos2(pointer.x + 12.0, pointer.y - 16.0),
        egui::vec2(200.0, 32.0),
    );
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("photo_layer_drag_ghost"),
    ));
    painter.rect_filled(rect, theme.radius.sm, theme.colors.item_selected);
    painter.text(
        rect.left_center() + egui::vec2(theme.spacing.sm, 0.0),
        egui::Align2::LEFT_CENTER,
        layer.name.clone(),
        egui::FontId::proportional(theme.typography.body_size),
        theme.colors.fg_primary,
    );
}

/// Dessine la liste des calques (virtualisée, réordonnable par drag &
/// drop au niveau racine) puis la barre de boutons (image, vide,
/// dupliquer, masque, filtre, supprimer).
///
/// Le drop envoie `ReorderLayer` au worker (non bloquant). Retourne
/// les actions, traitées par l'app (sélection locale, renommage et
/// mutations via le worker).
#[allow(clippy::too_many_arguments)]
pub fn draw_photo_layer_panel(
    ui: &mut egui::Ui,
    layers: &[PhotoLayerInfo],
    selected: Option<Uuid>,
    rename: &mut LayerRenameState,
    drag_state: &mut ReorderDragState,
    engine_tx: &Sender<PhotoEngineCommand>,
) -> Vec<LayerPanelAction> {
    let mut actions = Vec::new();
    let reorder = ReorderableList::new(layers, PHOTO_LAYER_ITEM_HEIGHT, drag_state).show(
        ui,
        |ui, layer, _index, _is_dragging| {
            for item_action in
                draw_photo_layer_item(ui, layer, Some(layer.id) == selected, rename, engine_tx)
            {
                match item_action {
                    LayerItemAction::Select(id) => actions.push(LayerPanelAction::Select(id)),
                    LayerItemAction::RenameCommit { layer, name } => {
                        actions.push(LayerPanelAction::RenameCommit { layer, name });
                    }
                }
            }
        },
    );
    if let Some((from, to)) = reorder
        && from != to
    {
        let _ = engine_tx.send(PhotoEngineCommand::ReorderLayer { from, to });
    }
    draw_drag_ghost(ui, drag_state, layers);

    // Barre de boutons bas : ajouter image, calque vide, dupliquer,
    // masque, filtre, supprimer.
    ui.horizontal(|ui| {
        if icon_button(ui, CygnusIcon::ImageIcon, Some("Ajouter une image")).clicked() {
            actions.push(LayerPanelAction::AddImage);
        }
        if icon_button(ui, CygnusIcon::Add, Some("Nouveau calque vide")).clicked() {
            actions.push(LayerPanelAction::AddEmpty);
        }
        ui.add_enabled_ui(selected.is_some(), |ui| {
            if icon_button(ui, CygnusIcon::Duplicate, Some("Dupliquer le calque")).clicked() {
                actions.push(LayerPanelAction::Duplicate);
            }
            if icon_button(ui, CygnusIcon::Mask, Some("Ajouter un masque")).clicked() {
                actions.push(LayerPanelAction::AddMask);
            }
        });
        if icon_button(ui, CygnusIcon::Filter, Some("Liste des filtres")).clicked() {
            actions.push(LayerPanelAction::OpenFilterMenu);
        }
        ui.add_enabled_ui(selected.is_some(), |ui| {
            if icon_button(ui, CygnusIcon::Delete, Some("Supprimer le calque")).clicked() {
                actions.push(LayerPanelAction::Delete);
            }
        });
    });
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use photo_engine::{Document, LayerNode, PixelLayer};
    use std::sync::Arc;
    use std::sync::mpsc::channel;

    fn fixture_layers() -> Vec<PhotoLayerInfo> {
        let mut doc = Document::new(8, 8);
        let image = Arc::new(image::DynamicImage::new_rgba8(4, 4));
        for name in ["fond", "milieu", "dessus"] {
            doc.push_layer(LayerNode::Pixel(PixelLayer::new(name, image.clone())));
        }
        super::super::snapshot_layers(&doc)
    }

    #[test]
    fn panel_renders_without_panic_and_sends_nothing() {
        let ctx = egui::Context::default();
        ui_kit::theme::setup_fonts(&ctx);
        let layers = fixture_layers();
        let (tx, rx) = channel();
        let mut drag_state = ReorderDragState::default();
        let mut rename = LayerRenameState::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let actions =
                    draw_photo_layer_panel(ui, &layers, None, &mut rename, &mut drag_state, &tx);
                assert!(actions.is_empty(), "aucun clic sans interaction");
            });
        })
        .drop_without_applying_deltas();
        assert!(rx.try_recv().is_err(), "aucune commande attendue");
        assert!(!drag_state.is_dragging);
    }

    #[test]
    fn panel_empty_renders_without_panic() {
        let ctx = egui::Context::default();
        ui_kit::theme::setup_fonts(&ctx);
        let (tx, _rx) = channel();
        let mut drag_state = ReorderDragState::default();
        let mut rename = LayerRenameState::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                draw_photo_layer_panel(ui, &[], None, &mut rename, &mut drag_state, &tx);
            });
        })
        .drop_without_applying_deltas();
    }
}
