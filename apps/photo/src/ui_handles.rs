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

//! Frontière moteur → UI : convertit les buffers RGBA purs de `photo-engine`
//! en handles iced, avec cache.
//!
//! Le moteur ne connaît AUCUN framework d'interface (`RgbaBuf` = données
//! pures). Cette couche crée les textures iced UNE FOIS par version de
//! buffer (identifiée par l'adresse de son Arc) — jamais par frame :
//! l'identité stable du Handle préserve le cache de textures GPU de iced.

use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use photo_engine::Document;

/// Handle iced depuis un buffer pur — ZÉRO copie de pixels :
/// `Bytes::from_owner` partage l'Arc sous-jacent avec le moteur.
pub fn rgba_handle(buf: &photo_engine::RgbaBuf) -> iced::widget::image::Handle {
    iced::widget::image::Handle::from_rgba(
        buf.width,
        buf.height,
        iced_core::Bytes::from_owner(Arc::clone(&buf.data)),
    )
}

#[derive(Default)]
pub struct PreviewCache {
    entries: HashMap<Uuid, Entry>,
    mask_thumbs: HashMap<Uuid, MaskThumb>,
}

struct Entry {
    /// Adresses des Arc détenus — invalide l'entrée dès que le moteur
    /// régénère un buffer (édition source, changement de filtre…).
    key_preview: usize,
    key_thumb: usize,
    /// Les Arc sont conservés vivants ici : empêche la réutilisation de
    /// leur adresse par un nouvel alloc tant qu'elles servent de clé (ABA).
    _keep_alive: (Arc<[u8]>, Arc<[u8]>),
    preview: iced::widget::image::Handle,
    thumb: iced::widget::image::Handle,
}

/// Miniature d'un masque, clé = adresse de l'Arc du buffer d'image.
struct MaskThumb {
    key: usize,
    _keep_alive: Arc<image::RgbaImage>,
    thumb: iced::widget::image::Handle,
}

fn arc_addr(data: &Arc<[u8]>) -> usize {
    Arc::as_ptr(data).cast::<u8>() as usize
}

fn img_arc_addr(data: &Arc<image::RgbaImage>) -> usize {
    Arc::as_ptr(data).cast::<u8>() as usize
}

/// Miniature 36×24 (aspect conservé) d'un buffer de masque.
fn mask_thumb_handle(img: &image::RgbaImage) -> iced::widget::image::Handle {
    let out = image::imageops::thumbnail(img, 36, 24);
    let (w, h) = out.dimensions();
    iced::widget::image::Handle::from_rgba(
        w,
        h,
        iced_core::Bytes::from_owner(Arc::from(out.into_raw())),
    )
}

impl PreviewCache {
    /// Aligne le cache sur l'apparence courante des calques pixels de
    /// l'arbre. Appelé après CHAQUE message (point unique de synchronisation).
    pub fn sync(&mut self, doc: &Document) {
        let ids: Vec<Uuid> = doc.iter_pixels().iter().map(|l| l.id).collect();
        self.entries.retain(|id, _| ids.contains(id));
        for id in ids {
            // Fat pointer → pointeur brut : cast via `.cast::<u8>()`
            let Some(appearance) = doc.appearance(id) else {
                continue;
            };
            let kp = arc_addr(&appearance.preview.data);
            let kt = arc_addr(&appearance.thumb.data);
            match self.entries.get(&id) {
                Some(e) if e.key_preview == kp && e.key_thumb == kt => {}
                _ => {
                    let preview = rgba_handle(&appearance.preview);
                    let thumb = rgba_handle(&appearance.thumb);
                    self.entries.insert(
                        id,
                        Entry {
                            key_preview: kp,
                            key_thumb: kt,
                            _keep_alive: (
                                Arc::clone(&appearance.preview.data),
                                Arc::clone(&appearance.thumb.data),
                            ),
                            preview,
                            thumb,
                        },
                    );
                }
            }
        }

        // Masques : mêmes règles d'identité par Arc.
        let mask_ids: Vec<Uuid> = doc.iter_masks().iter().map(|(_, m)| m.id).collect();
        self.mask_thumbs.retain(|id, _| mask_ids.contains(id));
        for (_, mask) in doc.iter_masks() {
            let key = img_arc_addr(&mask.image);
            match self.mask_thumbs.get(&mask.id) {
                Some(e) if e.key == key => {}
                _ => {
                    let thumb = mask_thumb_handle(&mask.image);
                    self.mask_thumbs.insert(
                        mask.id,
                        MaskThumb {
                            key,
                            _keep_alive: Arc::clone(&mask.image),
                            thumb,
                        },
                    );
                }
            }
        }
    }

    /// Handle d'aperçu interactif du calque (texture canvas).
    pub fn preview(&self, id: Uuid) -> Option<&iced::widget::image::Handle> {
        self.entries.get(&id).map(|e| &e.preview)
    }

    /// Handle miniature du calque (panneau Calques).
    pub fn thumb(&self, id: Uuid) -> Option<&iced::widget::image::Handle> {
        self.entries.get(&id).map(|e| &e.thumb)
    }

    /// Handle miniature d'un masque (panneau Calques).
    pub fn mask_thumb(&self, id: Uuid) -> Option<&iced::widget::image::Handle> {
        self.mask_thumbs.get(&id).map(|e| &e.thumb)
    }
}
