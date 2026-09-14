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

//! Panneau couleur PHOTO (onglet du studio droit, façon Affinity).
//!
//! Sélecteur de couleur du pinceau + nuancier de base. État purement
//! local (aucun channel) : la couleur choisie alimente les traits.

use ui_kit::theme::tokens::CygnusTheme;
use ui_kit::theme::typography::body_text;

/// Nuancier de base (lignes de 6).
const SWATCHES: &[[u8; 3]] = &[
    [0, 0, 0],
    [255, 255, 255],
    [200, 40, 40],
    [240, 140, 40],
    [240, 220, 60],
    [80, 200, 120],
    [88, 124, 255],
    [150, 90, 220],
    [200, 120, 180],
    [140, 100, 70],
    [120, 120, 120],
    [60, 60, 70],
];

/// Dessine le sélecteur de couleur et met à jour `color` (RGB).
pub fn draw_color_panel(ui: &mut egui::Ui, color: &mut [u8; 3]) {
    let theme = CygnusTheme::dark();
    ui.label(body_text(&theme, "Couleur du pinceau"));
    egui::color_picker::color_edit_button_srgb(ui, color);
    ui.add_space(theme.spacing.sm);
    ui.label(body_text(&theme, "Nuancier"));
    egui::Grid::new("photo_swatches")
        .num_columns(6)
        .show(ui, |ui| {
            for (index, swatch) in SWATCHES.iter().enumerate() {
                let selected = *color == *swatch;
                let button = egui::Button::new("")
                    .fill(egui::Color32::from_rgb(swatch[0], swatch[1], swatch[2]))
                    .selected(selected)
                    .min_size(egui::vec2(22.0, 22.0));
                if ui.add(button).on_hover_text("Choisir").clicked() {
                    *color = *swatch;
                }
                if (index + 1) % 6 == 0 {
                    ui.end_row();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_panel_renders_without_panic_and_idle() {
        let ctx = egui::Context::default();
        ui_kit::theme::setup_fonts(&ctx);
        let mut color = [255u8, 255, 255];
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                draw_color_panel(ui, &mut color);
            });
        })
        .drop_without_applying_deltas();
        assert_eq!(color, [255, 255, 255]);
    }
}
