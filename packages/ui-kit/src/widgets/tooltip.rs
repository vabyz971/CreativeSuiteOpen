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

//! Tooltip standard Cygnus.
//!
//! Tous les boutons d'icônes et contrôles ambigus affichent une infobulle
//! via cet helper, pour un comportement uniforme dans les 3 apps.

/// Attache une infobulle à une réponse egui.
///
/// # Exemple
/// ```rust
/// # use ui_kit::widgets::tooltip::with_tooltip;
/// # egui::__run_test_ui(|ui| {
/// let response = ui.button("?");
/// with_tooltip(response, "Aide contextuelle");
/// # });
/// ```
pub fn with_tooltip(response: egui::Response, text: &str) -> egui::Response {
    response.on_hover_text(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_does_not_panic() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let response = ui.button("?");
                let _ = with_tooltip(response, "Aide");
            });
        })
        .drop_without_applying_deltas();
    }
}
