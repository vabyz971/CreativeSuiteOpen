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

//! État de drag & drop GÉNÉRIQUE basé sur des index.
//!
//! Réutilisable : calques (photo), clips (video), pistes (audio).
//! Strictement domain-agnostic : aucun type métier ici, uniquement
//! des index.

/// État de drag & drop générique basé sur des index.
#[derive(Debug, Clone, Default)]
pub struct ReorderDragState {
    /// Index d'origine du drag, pendant un drag.
    pub dragging_index: Option<usize>,
    /// Dernière position souris connue pendant le drag.
    pub current_mouse_pos: egui::Pos2,
    /// Index d'insertion courant, pendant un drag.
    pub target_index: Option<usize>,
    /// Vrai si un drag est en cours.
    pub is_dragging: bool,
}

impl ReorderDragState {
    /// Démarre le drag de l'item à `index`.
    pub fn start(&mut self, index: usize) {
        self.dragging_index = Some(index);
        self.is_dragging = true;
    }

    /// Réinitialise l'état (fin ou annulation de drag).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Termine le drag et retourne `(from, to)` si un
    /// réordonnancement doit être appliqué, `None` sinon.
    /// Réinitialise l'état dans tous les cas.
    pub fn end_drag(&mut self) -> Option<(usize, usize)> {
        let result = match (self.dragging_index, self.target_index) {
            (Some(from), Some(to)) if from != to => Some((from, to)),
            _ => None,
        };
        self.reset();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_reset() {
        let mut state = ReorderDragState::default();
        assert!(!state.is_dragging);
        state.start(2);
        assert!(state.is_dragging);
        assert_eq!(state.dragging_index, Some(2));
        state.target_index = Some(5);
        assert_eq!(state.end_drag(), Some((2, 5)));
        // Réinitialisé après end_drag.
        assert!(!state.is_dragging);
        assert_eq!(state.dragging_index, None);
        assert_eq!(state.target_index, None);
    }

    #[test]
    fn end_drag_same_index_returns_none() {
        let mut state = ReorderDragState::default();
        state.start(3);
        state.target_index = Some(3);
        assert_eq!(state.end_drag(), None);
        assert!(!state.is_dragging);
    }

    #[test]
    fn end_drag_without_target_returns_none() {
        let mut state = ReorderDragState::default();
        state.start(0);
        assert_eq!(state.end_drag(), None);
    }
}
