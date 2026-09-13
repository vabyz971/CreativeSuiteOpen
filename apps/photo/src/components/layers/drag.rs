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

//! Machine d'état explicite du drag & drop des calques.
//!
//! Le document n'est jamais touché ici : cet état décrit seulement le geste
//! en cours. La validation et la mutation ont lieu au relâchement, dans les
//! handlers `update`.

use iced::Point;
use uuid::Uuid;

use super::drop_target::LayerDropTarget;

/// Distance minimale avant qu'un pressé devienne un drag (anti-clic accidentel).
pub const DRAG_DEADBAND: f32 = 5.0;

/// État du geste de déplacement d'un calque dans le panneau.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LayerDragState {
    #[default]
    Idle,
    Pressed {
        layer_id: Uuid,
        origin: Option<Point>,
        cursor: Option<Point>,
    },
    Dragging {
        layer_id: Uuid,
        cursor: Point,
        target: Option<LayerDropTarget>,
    },
}

/// Résultat pur d'un relâchement : sélection, commit ou abandon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerDragRelease {
    None,
    Select(Uuid),
    Commit {
        layer_id: Uuid,
        target: LayerDropTarget,
    },
}

impl LayerDragState {
    /// Démarre un pressé ; l'origine sera fixée au premier mouvement reçu.
    #[must_use]
    pub fn press(layer_id: Uuid) -> Self {
        Self::Pressed {
            layer_id,
            origin: None,
            cursor: None,
        }
    }

    /// Vrai dès qu'un pressé ou un drag est en cours.
    #[must_use]
    pub fn is_active(self) -> bool {
        !matches!(self, Self::Idle)
    }

    /// Vrai seulement après la deadband : les indicateurs n'apparaissent
    /// qu'en drag réel, pas au simple pressé.
    #[must_use]
    pub fn is_dragging(self) -> bool {
        matches!(self, Self::Dragging { .. })
    }

    /// Calque source du geste en cours, s'il existe.
    #[must_use]
    pub fn dragged_id(self) -> Option<Uuid> {
        match self {
            Self::Idle => None,
            Self::Pressed { layer_id, .. } | Self::Dragging { layer_id, .. } => Some(layer_id),
        }
    }

    /// Cible candidate affichée pendant le drag, s'il y en a une.
    #[must_use]
    pub fn target(self) -> Option<LayerDropTarget> {
        match self {
            Self::Dragging { target, .. } => target,
            _ => None,
        }
    }

    /// Avance le geste avec la position courante du pointeur.
    pub fn moved(&mut self, position: Point) {
        match self {
            Self::Idle => {}
            Self::Pressed {
                layer_id,
                origin,
                cursor,
            } => {
                let origin = origin.get_or_insert(position);
                *cursor = Some(position);
                let dx = position.x - origin.x;
                let dy = position.y - origin.y;
                if dx.hypot(dy) >= DRAG_DEADBAND {
                    *self = Self::Dragging {
                        layer_id: *layer_id,
                        cursor: position,
                        target: None,
                    };
                }
            }
            Self::Dragging { cursor, .. } => {
                *cursor = position;
            }
        }
    }

    /// Mémorise la cible survolée ; sans effet hors drag.
    pub fn hover(&mut self, target: Option<LayerDropTarget>) {
        if let Self::Dragging {
            target: current, ..
        } = self
        {
            *current = target;
        }
    }

    /// Termine le geste sans toucher au document.
    #[must_use]
    pub fn release(self) -> (LayerDragRelease, Self) {
        match self {
            Self::Idle => (LayerDragRelease::None, Self::Idle),
            Self::Pressed { layer_id, .. } => (LayerDragRelease::Select(layer_id), Self::Idle),
            Self::Dragging {
                layer_id, target, ..
            } => match target {
                Some(target) => (LayerDragRelease::Commit { layer_id, target }, Self::Idle),
                None => (LayerDragRelease::None, Self::Idle),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn clic_sans_mouvement_selectionne() {
        let id = Uuid::nil();
        let state = LayerDragState::press(id);
        let (release, next) = state.release();

        assert_eq!(release, LayerDragRelease::Select(id));
        assert_eq!(next, LayerDragState::Idle);
    }

    #[test]
    fn petit_mouvement_reste_presse() {
        let mut state = LayerDragState::press(Uuid::nil());
        state.moved(point(10.0, 10.0));
        state.moved(point(10.0 + DRAG_DEADBAND - 0.5, 10.0));

        assert!(matches!(state, LayerDragState::Pressed { .. }));
        assert!(state.is_active());
    }

    #[test]
    fn mouvement_suffisant_basculer_en_drag() {
        let mut state = LayerDragState::press(Uuid::nil());
        state.moved(point(10.0, 10.0));
        state.moved(point(10.0 + DRAG_DEADBAND, 10.0));

        assert!(matches!(state, LayerDragState::Dragging { .. }));
    }

    #[test]
    fn survol_hors_drag_sans_effet() {
        let mut state = LayerDragState::press(Uuid::nil());
        let target = LayerDropTarget::Before(Uuid::nil());
        state.hover(Some(target));

        assert_eq!(state.target(), None);
    }

    #[test]
    fn relachement_sans_cible_abandonne() {
        let mut state = LayerDragState::press(Uuid::nil());
        state.moved(point(0.0, 0.0));
        state.moved(point(DRAG_DEADBAND, 0.0));
        let (release, next) = state.release();

        assert_eq!(release, LayerDragRelease::None);
        assert_eq!(next, LayerDragState::Idle);
    }

    #[test]
    fn relachement_avec_cible_propose_un_commit() {
        let id = Uuid::nil();
        let target = LayerDropTarget::After(Uuid::nil());
        let mut state = LayerDragState::press(id);
        state.moved(point(0.0, 0.0));
        state.moved(point(DRAG_DEADBAND, 0.0));
        state.hover(Some(target));
        let (release, next) = state.release();

        assert_eq!(
            release,
            LayerDragRelease::Commit {
                layer_id: id,
                target
            }
        );
        assert_eq!(next, LayerDragState::Idle);
    }

    #[test]
    fn mouvements_hors_geste_ignores() {
        let mut state = LayerDragState::Idle;
        state.moved(point(100.0, 100.0));

        assert_eq!(state, LayerDragState::Idle);
    }
}
