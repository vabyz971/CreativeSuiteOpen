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

//! Slider standard Cygnus : libellé + valeur sur une ligne.
//!
//! Les réglages continus (opacité, zoom…) affichent ce widget et envoient
//! la valeur au moteur via un channel — le widget ne fait AUCUN calcul.

use crate::theme::tokens::CygnusTheme;

/// Slider labellisé Cygnus.
///
/// # Exemple
/// ```rust
/// # use ui_kit::widgets::slider::CygnusSlider;
/// # egui::__run_test_ui(|ui| {
/// let mut opacity = 0.8;
/// CygnusSlider::new("Opacite", 0.0..=1.0).show(ui, &mut opacity);
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct CygnusSlider<'a> {
    label: &'a str,
    range: std::ops::RangeInclusive<f32>,
}

impl<'a> CygnusSlider<'a> {
    /// Crée un slider avec libellé et plage.
    pub fn new(label: &'a str, range: std::ops::RangeInclusive<f32>) -> Self {
        Self { label, range }
    }

    /// Affiche le slider (label à gauche, contrôle à droite).
    pub fn show(self, ui: &mut egui::Ui, value: &mut f32) -> egui::Response {
        let theme = CygnusTheme::dark();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(self.label)
                    .size(theme.typography.body_size)
                    .color(theme.colors.fg_secondary),
            );
            ui.add(egui::Slider::new(value, self.range).show_value(true))
        })
        .inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_renders_without_panic() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut value = 0.5;
                let _ = CygnusSlider::new("Opacite", 0.0..=1.0).show(ui, &mut value);
            });
        })
        .drop_without_applying_deltas();
    }
}
