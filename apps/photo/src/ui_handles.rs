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
// MERCHANTABILITY for A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Frontière moteur → UI : convertit les buffers RGBA purs de `photo-engine`
//! en handles iced, avec cache.
//!
//! Le moteur ne connaît AUCUN framework d'interface (`RgbaBuf` = données
//! pures). Cette couche crée les textures iced UNE FOIS par version de
//! buffer (identifiée par l'adresse de son Arc) — jamais par frame :
//! l'identité stable du Handle préserve le cache de textures GPU de iced.

use std::collections::HashMap;
use std::sync::Arc;

use crate::layers::Layer;

/// Handle iced depuis un buffer pur — ZÉRO copie de pixels :
/// `Bytes::from_owner` partage l'Arc sous-jacent avec le moteur.
pub fn rgba_handle(buf: &photo_engine::document::RgbaBuf) -> iced::widget::image::Handle {
    iced::widget::image::Handle::from_rgba(
        buf.width,
        buf.height,
        iced_core::Bytes::from_owner(Arc::clone(&buf.data)),
    )
}

#[derive(Default)]
pub struct PreviewCache {
    entries: HashMap<u64, Entry>,
}

struct Entry {
    /// Adresses des Arc détenus — invalide l'entrée dès que le moteur
    /// régénère un buffer (apply_edit, crop, peinture…).
    key_preview: usize,
    key_thumb: usize,
    /// Les Arc sont conservés vivants ici : empêche la réutilisation de
    /// leur adresse par un nouvel alloc tant qu'elles servent de clé (ABA).
    _keep_alive: (Arc<[u8]>, Arc<[u8]>),
    preview: iced::widget::image::Handle,
    thumb: iced::widget::image::Handle,
}

impl PreviewCache {
    /// Aligne le cache sur l'état courant des calques. Appelé après CHAQUE
    /// message (point unique de synchronisation, coût : N lookups).
    pub fn sync(&mut self, layers: &[Layer]) {
        let ids: Vec<u64> = layers.iter().map(|l| l.id).collect();
        self.entries.retain(|id, _| ids.contains(id));
        for layer in layers {
            // Fat pointer → pointeur brut : cast via `.cast::<u8>()`
            let kp = Arc::as_ptr(&layer.preview.data).cast::<u8>() as usize;
            let kt = Arc::as_ptr(&layer.thumb.data).cast::<u8>() as usize;
            match self.entries.get(&layer.id) {
                Some(e) if e.key_preview == kp && e.key_thumb == kt => {}
                _ => {
                    let preview = rgba_handle(&layer.preview);
                    let thumb = rgba_handle(&layer.thumb);
                    self.entries.insert(
                        layer.id,
                        Entry {
                            key_preview: kp,
                            key_thumb: kt,
                            _keep_alive: (
                                Arc::clone(&layer.preview.data),
                                Arc::clone(&layer.thumb.data),
                            ),
                            preview,
                            thumb,
                        },
                    );
                }
            }
        }
    }

    /// Handle d'aperçu interactif du calque (texture canvas).
    pub fn preview(&self, id: u64) -> Option<&iced::widget::image::Handle> {
        self.entries.get(&id).map(|e| &e.preview)
    }

    /// Handle miniature du calque (panneau Calques).
    pub fn thumb(&self, id: u64) -> Option<&iced::widget::image::Handle> {
        self.entries.get(&id).map(|e| &e.thumb)
    }
}
