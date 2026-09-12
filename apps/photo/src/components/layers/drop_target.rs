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

//! Cibles de drop du panneau Calques.
//!
//! Ce module ne mute jamais le document : il traduit une zone survolée en
//! opération candidate, puis interroge le moteur pour la valider.

use photo_engine::Document;
use uuid::Uuid;

/// Opération de drop proposée par l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerDropTarget {
    Before(Uuid),
    After(Uuid),
    Inside(Uuid),
}

/// Zone survolée : trait d'insertion avant/après, ou corps d'un groupe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPosition {
    Before,
    After,
    Inside,
}

impl LayerDropTarget {
    /// Calque visé par la cible (référence avant/après, ou groupe parent).
    #[must_use]
    pub fn target_id(self) -> Uuid {
        match self {
            Self::Before(id) | Self::After(id) | Self::Inside(id) => id,
        }
    }

    /// Demande au moteur si l'opération candidate est valide.
    #[must_use]
    pub fn is_valid(self, doc: &Document, dragged: Uuid) -> bool {
        match self {
            Self::Before(target) | Self::After(target) => doc.can_reorder_before(dragged, target),
            Self::Inside(group) => doc.can_move_into(dragged, group),
        }
    }
}

/// Traduit une zone survolée en cible valide, ou `None` si interdite.
///
/// Les règles métier (soi-même, descendant, groupe inexistant) restent côté
/// moteur via [`Document::can_reorder_before`] et [`Document::can_move_into`].
#[must_use]
pub fn resolve_drop_target(
    doc: &Document,
    dragged: Uuid,
    hovered: Uuid,
    position: DropPosition,
) -> Option<LayerDropTarget> {
    let candidate = match position {
        DropPosition::Before => LayerDropTarget::Before(hovered),
        DropPosition::After => LayerDropTarget::After(hovered),
        DropPosition::Inside => LayerDropTarget::Inside(hovered),
    };
    candidate.is_valid(doc, dragged).then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgba};
    use photo_engine::{GroupLayer, LayerNode, PixelLayer};
    use std::sync::Arc;

    fn pixel(name: &str) -> LayerNode {
        let image = ImageBuffer::from_pixel(1, 1, Rgba([9, 9, 9, 255]));
        LayerNode::Pixel(PixelLayer::new(
            name,
            Arc::new(DynamicImage::ImageRgba8(image)),
        ))
    }

    fn document_imbrique() -> (Document, Uuid, Uuid, Uuid, Uuid, Uuid) {
        let mut doc = Document::new(4, 4);
        let image1 = pixel("image 1");
        let image2 = pixel("image 2");
        let image3 = pixel("image 3");
        let image4 = pixel("image 4");
        let (id1, id2, id3, id4) = (image1.id(), image2.id(), image3.id(), image4.id());
        let groupe_b = GroupLayer::new("groupe B", vec![image3, image4]);
        let _gid_b = groupe_b.id;
        let groupe_a =
            GroupLayer::new("groupe A", vec![image1, image2, LayerNode::Group(groupe_b)]);
        let gid_a = groupe_a.id;
        doc.push_layer(LayerNode::Group(groupe_a));
        (doc, id1, id2, id3, id4, gid_a)
    }

    #[test]
    fn avant_apres_valides() {
        let (doc, id1, id2, _, _, _) = document_imbrique();

        assert_eq!(
            resolve_drop_target(&doc, id1, id2, DropPosition::Before),
            Some(LayerDropTarget::Before(id2))
        );
        assert_eq!(
            resolve_drop_target(&doc, id1, id2, DropPosition::After),
            Some(LayerDropTarget::After(id2))
        );
    }

    #[test]
    fn dedans_groupe_valide() {
        let (doc, id1, _, _, _, gid_a) = document_imbrique();

        assert_eq!(
            resolve_drop_target(&doc, id1, gid_a, DropPosition::Inside),
            Some(LayerDropTarget::Inside(gid_a))
        );
    }

    #[test]
    fn auto_drop_et_descendant_refuses() {
        let (doc, _, _, id3, _, gid_a) = document_imbrique();
        let groupe_b = match doc.find(gid_a) {
            Some(LayerNode::Group(groupe)) => groupe
                .children
                .iter()
                .find_map(|enfant| match enfant {
                    LayerNode::Group(groupe) => Some(groupe.id),
                    _ => None,
                })
                .expect("groupe B"),
            _ => unreachable!("groupe A"),
        };

        assert_eq!(
            resolve_drop_target(&doc, gid_a, gid_a, DropPosition::Inside),
            None
        );
        assert_eq!(
            resolve_drop_target(&doc, gid_a, id3, DropPosition::Before),
            None
        );
        assert_eq!(
            resolve_drop_target(&doc, groupe_b, groupe_b, DropPosition::After),
            None
        );
    }

    #[test]
    fn dedans_non_groupe_refuse() {
        let (doc, id1, id2, _, _, _) = document_imbrique();

        assert_eq!(
            resolve_drop_target(&doc, id1, id2, DropPosition::Inside),
            None
        );
    }
}
