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

//! Onglets standard Cygnus.
//!
//! En-têtes sélectionnables au style du thème + contenu de l'onglet
//! actif. L'index sélectionné est détenu par l'app (état immédiat
//! egui) ; [`sanitize_selected`] protège des index hors limites.

/// Clamp un index d'onglet à la plage valide.
///
/// Retourne 0 pour une liste vide. Utilisé avant chaque affichage pour
/// survivre à la suppression dynamique d'onglets.
pub fn sanitize_selected(selected: usize, len: usize) -> usize {
    if len == 0 { 0 } else { selected.min(len - 1) }
}

/// Onglets standard Cygnus.
///
/// # Exemple
/// ```rust
/// # use ui_kit::panels::CygnusTabs;
/// # egui::__run_test_ui(|ui| {
/// let mut selected = 0;
/// CygnusTabs::new(&["Calques", "Proprietes"]).show(ui, &mut selected, |ui, index| {
///     ui.label(format!("contenu {index}"));
/// });
/// # });
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CygnusTabs<'a> {
    titles: &'a [&'a str],
}

impl<'a> CygnusTabs<'a> {
    /// Crée un groupe d'onglets avec les titres donnés.
    pub fn new(titles: &'a [&'a str]) -> Self {
        Self { titles }
    }

    /// Nombre d'onglets.
    pub fn len(self) -> usize {
        self.titles.len()
    }

    /// Vrai si aucun onglet.
    pub fn is_empty(self) -> bool {
        self.titles.is_empty()
    }

    /// Affiche les en-têtes puis le contenu de l'onglet actif.
    /// `add_contents` reçoit l'index sanitizé de l'onglet affiché.
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        selected: &mut usize,
        add_contents: impl FnOnce(&mut egui::Ui, usize) -> R,
    ) -> R {
        *selected = sanitize_selected(*selected, self.titles.len());
        ui.horizontal(|ui| {
            for (index, title) in self.titles.iter().enumerate() {
                ui.selectable_value(selected, index, *title);
            }
        });
        ui.separator();
        add_contents(ui, *selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_clamps_out_of_range() {
        assert_eq!(sanitize_selected(0, 0), 0);
        assert_eq!(sanitize_selected(5, 0), 0);
        assert_eq!(sanitize_selected(0, 3), 0);
        assert_eq!(sanitize_selected(2, 3), 2);
        assert_eq!(sanitize_selected(3, 3), 2);
        assert_eq!(sanitize_selected(99, 3), 2);
    }

    #[test]
    fn tabs_show_sanitized_content() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut selected = 99;
            let mut shown = usize::MAX;
            CygnusTabs::new(&["A", "B", "C"]).show(ui, &mut selected, |_ui, index| {
                shown = index;
            });
            assert_eq!(selected, 2);
            assert_eq!(shown, 2);
        })
        .drop_without_applying_deltas();
    }

    #[test]
    fn tabs_empty_renders_without_panic() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut selected = 0;
            let empty: &[&str] = &[];
            CygnusTabs::new(empty).show(ui, &mut selected, |ui, _| {
                ui.label("vide");
            });
        })
        .drop_without_applying_deltas();
    }
}
