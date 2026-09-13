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

//! Types partagés par les messages : outil, panneau, masque, peinture.

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Hand,
    Zoom,
    Select,
    Eyedropper,
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

/// Studio actif du contenu central : Pixel (photo, moteur actuel),
/// Vecteur (`vector-engine`, futur) et Mise en page (`layout-engine`, futur).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StudioMode {
    /// Retouche photo — seul mode fonctionnel pour l'instant.
    #[default]
    Pixel,
    /// Illustration vectorielle — panneau d'attente (phase 4+).
    Vector,
    /// Mise en page — panneau d'attente (phase 4+).
    Layout,
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

/// Réglage de paramètre demandé au slider mais pas encore appliqué :
/// le vivant garde l'ancienne valeur (donc `sync()` HIT, zéro freeze)
/// pendant que le worker pré-chauffe la nouvelle. Le pouce du slider
/// affiche cette valeur en attendant ; `param_epoch` invalide les vols
/// périmés par un undo/redo.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingParam {
    pub layer_id: Uuid,
    pub filter_id: Uuid,
    pub key: String,
    pub value: datatypes::ParamValue,
}

/// Toggle d'activation appliqué sur un clone en `spawn_blocking` puis
/// rejoué sur le document vivant à la réception de `AppearanceWarmed`.
/// `layer_id` = porteur du toggle (peut être un sous-calque de filtre
/// pour les masques) ; le pré-chauffage vise le calque pixels porteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceToggle {
    /// (Sous-)calque de filtre on/off — `layer_id` = parent pixels ou ajustement.
    FilterEnabled { filter_id: Uuid, enabled: bool },
    /// Masque on/off — `layer_id` = porteur du masque.
    MaskEnabled { mask_id: Uuid, enabled: bool },
    /// Inversion du masque — `layer_id` = porteur du masque.
    MaskInverted { mask_id: Uuid, inverted: bool },
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

/// Image peinte à appliquer sur le vivant : le MÊME `Arc` sert des deux
/// côtés (clone worker → document vivant), ce qui préserve l'identité de
/// pointeur exigée par le cache d'apparences — l'entrée pré-chauffée
/// transportée avec reste valide.
/// `Mask` porte la version touchée UNE seule fois côté worker : la
/// réception l'adopte telle quelle (jamais de second `touch()`, qui
/// ferait diverger la signature — voir `apply_toggle_flag`).
#[derive(Clone)]
pub enum PaintedImage {
    Layer(std::sync::Arc<image::DynamicImage>),
    Mask {
        image: std::sync::Arc<image::RgbaImage>,
        version: u64,
    },
}

impl std::fmt::Debug for PaintedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use image::GenericImageView as _;
        match self {
            Self::Layer(img) => f
                .debug_struct("PaintedImage::Layer")
                .field("dims", &img.dimensions())
                .finish(),
            Self::Mask { image, version } => f
                .debug_struct("PaintedImage::Mask")
                .field("dims", &image.dimensions())
                .field("version", version)
                .finish(),
        }
    }
}
