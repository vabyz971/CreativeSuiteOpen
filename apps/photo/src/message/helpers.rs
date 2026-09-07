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

//! Types partagés par les messages : outil, panneau, masque, peinture.

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Hand,
    Zoom,
    Select,
    Eyedropper,
    Move,
    /// Pinceau : peint sur le calque sélectionné.
    Brush,
    /// Gomme : efface (réduit l'alpha) sur le calque sélectionné.
    Eraser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelType {
    Canvas,
    Properties,
    Layers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetAxis {
    X,
    Y,
}

/// Identifie un masque précis parmi les N masques d'un calque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaskTarget {
    pub layer_id: Uuid,
    pub mask_id: Uuid,
}

/// Type d'opération destructrice asynchrone (Flip, Crop).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructiveOp {
    FlipHorizontal,
    FlipVertical,
    Crop,
}

/// Résultat d'une opération destructrice calculée hors thread UI.
/// Buffers RgbaImage PROPRES : l'application côté UI se fait par simple
/// wrap (Arc), zéro copie pixels. Pour Crop, contient aussi le décalage
/// de transform à appliquer.
#[derive(Debug, Clone)]
pub struct DestructiveResult {
    pub source: image::RgbaImage,
    pub masks: Vec<image::RgbaImage>,
    /// Masques des sous-calques de filtres (id du filtre + ses masques dans
    /// l'ordre) — mêmes opérations géométriques que les masques du calque.
    pub filter_masks: Vec<(Uuid, Vec<image::RgbaImage>)>,
    /// Décalage de transform à ajouter (Crop seulement ; `(0, 0)` pour Flip).
    pub offset_delta: (f32, f32),
}

/// Calque pixels décodé (thread async) — Debug manuel car la texture n'est
/// pas formattable.
#[derive(Clone)]
pub struct DecodedLayer(pub crate::layers::PixelLayer);
impl std::fmt::Debug for DecodedLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (w, h) = self.0.dimensions();
        f.debug_struct("DecodedLayer")
            .field("id", &self.0.id)
            .field("dims", &(w, h))
            .finish()
    }
}

/// Trait terminé dont les pixels sont en cours de fusion hors thread UI.
/// La texture d'aperçu (rastérisée par le canvas) reste affichée telle
/// quelle jusqu'à PaintApplied — continuité visuelle parfaite.
#[derive(Clone)]
pub struct PendingPaint {
    pub layer_id: Uuid,
    /// Masque ciblé si le trait peignait un masque (None = pixels du calque).
    pub mask_id: Option<Uuid>,
    pub tex: ui_kit::image_canvas::StrokeTex,
}