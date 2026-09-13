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
    /// Couvertures de groupes à la taille du document (espace document),
    /// clé = id du groupe, invalidées par signature (voir `sync`).
    couvertures_groupes: HashMap<Uuid, CoverEntree>,
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
    /// Tampon d'apparence bakée (masques inclus) — le chemin GPU unique
    /// l'envoie tel quel comme texture du calque (zéro copie, clone d'Arc).
    apparence: photo_engine::RgbaBuf,
}

/// Couverture de groupe en cache (taille document + signature).
struct CoverEntree {
    signature: u64,
    couverture: photo_engine::RgbaBuf,
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
            // D'abord un lookup STRICT (aucun pool, aucun render) : à chaud,
            // chaque message re-valide tous les calques sans aucun coût de
            // thread. En cas de vrai miss (source/filtres changés) on bascule
            // sur le chemin lourd `appearance` — le seul qui exécute.
            let appearance = match doc.appearance_hit(id) {
                Some(a) => a,
                None => {
                    let Some(a) = doc.appearance(id) else {
                        continue;
                    };
                    a
                }
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
                            apparence: appearance.preview.clone(),
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

        // Couvertures de groupes : recalcul par signature + éviction des
        // groupes disparus (même point unique de synchronisation).
        self.maj_couvertures(doc);
    }

    /// Recalcule les couvertures de groupes périmées et évince les groupes
    /// disparus. Appelé par `sync` après chaque message.
    fn maj_couvertures(&mut self, doc: &Document) {
        fn visiter(cache: &mut PreviewCache, noeuds: &[photo_engine::LayerNode], w: u32, h: u32) {
            for noeud in noeuds {
                if let photo_engine::LayerNode::Group(g) = noeud {
                    let signature = signature_couverture(w, h, &g.masks);
                    let frais = cache.couvertures_groupes.get(&g.id).map(|e| e.signature)
                        != Some(signature);
                    if frais {
                        match composer_couverture(w, h, &g.masks) {
                            Some(couverture) => {
                                cache.couvertures_groupes.insert(
                                    g.id,
                                    CoverEntree {
                                        signature,
                                        couverture,
                                    },
                                );
                            }
                            None => {
                                cache.couvertures_groupes.remove(&g.id);
                            }
                        }
                    }
                    visiter(cache, &g.children, w, h);
                }
            }
        }
        visiter(self, &doc.root, doc.width, doc.height);
        // Éviction des groupes disparus.
        let mut presents = Vec::new();
        fn collecter(noeuds: &[photo_engine::LayerNode], ids: &mut Vec<Uuid>) {
            for noeud in noeuds {
                if let photo_engine::LayerNode::Group(g) = noeud {
                    ids.push(g.id);
                    collecter(&g.children, ids);
                }
            }
        }
        collecter(&doc.root, &mut presents);
        self.couvertures_groupes
            .retain(|id, _| presents.contains(id));
    }

    /// Handle d'aperçu interactif du calque (texture canvas).
    pub fn preview(&self, id: Uuid) -> Option<&iced::widget::image::Handle> {
        self.entries.get(&id).map(|e| &e.preview)
    }

    /// Tampon d'apparence bakée (zéro copie) pour le chemin GPU unique.
    pub fn apparence(&self, id: Uuid) -> Option<photo_engine::RgbaBuf> {
        self.entries.get(&id).map(|e| e.apparence.clone())
    }

    /// Handle miniature du calque (panneau Calques).
    pub fn thumb(&self, id: Uuid) -> Option<&iced::widget::image::Handle> {
        self.entries.get(&id).map(|e| &e.thumb)
    }

    /// Handle miniature d'un masque (panneau Calques).
    pub fn mask_thumb(&self, id: Uuid) -> Option<&iced::widget::image::Handle> {
        self.mask_thumbs.get(&id).map(|e| &e.thumb)
    }

    /// Couverture en cache d'un groupe (`None` = aucun masque actif).
    /// Calculée par `sync` — ici simple lecture pour la vue.
    pub fn couverture_groupe(&self, groupe_id: Uuid) -> Option<photo_engine::RgbaBuf> {
        self.couvertures_groupes
            .get(&groupe_id)
            .map(|e| e.couverture.clone())
    }
}

/// Signature FNV du contenu des masques (id, version, actif, inversé) +
/// dimensions document.
fn signature_couverture(doc_l: u32, doc_h: u32, masques: &[photo_engine::LayerMask]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut nourrir = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
    };
    nourrir(u64::from(doc_l));
    nourrir(u64::from(doc_h));
    for m in masques {
        for b in m.id.as_bytes() {
            nourrir(u64::from(*b));
        }
        nourrir(m.version);
        nourrir(u64::from(m.enabled));
        nourrir(u64::from(m.inverted));
    }
    h
}

/// Compose la couverture : base blanche taille document, collage de chaque
/// masque actif à l'origine (avec inversion), multiplication — miroir exact
/// de `prepare_group_mask` + `combine_group_masks` côté moteur.
fn composer_couverture(
    doc_l: u32,
    doc_h: u32,
    masques: &[photo_engine::LayerMask],
) -> Option<photo_engine::RgbaBuf> {
    let (w, h) = (doc_l.max(1) as usize, doc_h.max(1) as usize);
    let mut acc: Option<Vec<u8>> = None;
    for m in masques.iter().filter(|m| m.enabled) {
        let (ml, mh) = (m.image.width() as usize, m.image.height() as usize);
        // Base blanche : 255 hors du masque, comme le CPU.
        let mut couvre = vec![255u8; w * h];
        let (cw, ch) = (ml.min(w), mh.min(h));
        for y in 0..ch {
            for x in 0..cw {
                let v = m.image.get_pixel(x as u32, y as u32)[0];
                couvre[y * w + x] = if m.inverted { 255 - v } else { v };
            }
        }
        acc = Some(match acc {
            // Multiplication (0 hors de l'un OU l'autre, comme le CPU).
            Some(a) => a
                .into_iter()
                .zip(couvre)
                .map(|(av, bv)| (u32::from(av) * u32::from(bv) / 255) as u8)
                .collect(),
            None => couvre,
        });
    }
    acc.map(|r| {
        let mut rgba = vec![0u8; w * h * 4];
        for (i, v) in r.into_iter().enumerate() {
            rgba[i * 4] = v;
            rgba[i * 4 + 3] = 255;
        }
        photo_engine::RgbaBuf::from_vec(w as u32, h as u32, rgba)
    })
}
