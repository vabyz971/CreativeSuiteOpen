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
#![allow(dead_code)] // TODO(Phase 5) : levé au câblage dans app.rs.
#![allow(unused_imports)] // TODO(Phase 5) : idem.

//! Panneau des propriétés du calque sélectionné (widget métier photo).
//!
//! Nom, type, visibilité et opacité (slider live → `SetOpacity`,
//! coalescé côté worker). Sans sélection : état vide explicite.
//! Style exclusivement ui-kit.

use super::layers::PhotoLayerInfo;
use crate::ui::engine_bridge::PhotoEngineCommand;
use std::sync::mpsc::Sender;
use ui_kit::theme::tokens::CygnusTheme;
use ui_kit::theme::typography::{body_text, heading_text};
use ui_kit::widgets::{CygnusSlider, CygnusToggle};

/// Dessine les propriétés du calque sélectionné (`None` = vide).
pub fn draw_photo_properties(
    ui: &mut egui::Ui,
    selected: Option<&PhotoLayerInfo>,
    engine_tx: &Sender<PhotoEngineCommand>,
) {
    let theme = CygnusTheme::dark();
    let Some(layer) = selected else {
        ui.label(body_text(&theme, "Aucun calque selectionne"));
        return;
    };
    ui.label(heading_text(&theme, &layer.name));
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(body_text(&theme, "Type :"));
        ui.label(body_text(&theme, layer.kind.icon_label()));
    });
    ui.horizontal(|ui| {
        ui.label(body_text(
            &theme,
            &format!(
                "Filtres : {} · Masques : {}",
                layer.filters.len(),
                layer.masks.len()
            ),
        ));
    });
    let mut visible = layer.visible;
    CygnusToggle::new("Visible").show(ui, &mut visible);
    if visible != layer.visible {
        let _ = engine_tx.send(PhotoEngineCommand::ToggleLayerVisibility(layer.id));
    }
    let mut opacity = layer.opacity;
    CygnusSlider::new("Opacite", 0.0..=100.0).show(ui, &mut opacity);
    if opacity != layer.opacity {
        let _ = engine_tx.send(PhotoEngineCommand::SetOpacity {
            layer: layer.id,
            opacity,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photo_engine::{Document, LayerNode, PixelLayer};
    use std::sync::Arc;
    use std::sync::mpsc::channel;

    fn fixture_layer() -> PhotoLayerInfo {
        let mut doc = Document::new(8, 8);
        let image = Arc::new(image::DynamicImage::new_rgba8(4, 4));
        doc.push_layer(LayerNode::Pixel(PixelLayer::new("fond", image)));
        super::super::layers::snapshot_layers(&doc)
            .pop()
            .expect("un calque")
    }

    #[test]
    fn properties_render_without_panic() {
        let ctx = egui::Context::default();
        ui_kit::theme::setup_fonts(&ctx);
        let layer = fixture_layer();
        let (tx, _rx) = channel();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                draw_photo_properties(ui, Some(&layer), &tx);
                draw_photo_properties(ui, None, &tx);
            });
        })
        .drop_without_applying_deltas();
    }

    #[test]
    fn properties_without_interaction_send_nothing() {
        let ctx = egui::Context::default();
        ui_kit::theme::setup_fonts(&ctx);
        let layer = fixture_layer();
        let (tx, rx) = channel();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                draw_photo_properties(ui, Some(&layer), &tx);
            });
        })
        .drop_without_applying_deltas();
        assert!(rx.try_recv().is_err(), "aucune commande attendue");
    }
}
