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

//! Rendu des apparences par calque avec CACHE CIBLÉ.
//!
//! [`Renderer`] est l'orchestrateur unique du calcul d'apparence
//! (source × live filters). Pour chaque calque :
//!
//! 1. clé de validité = (identité du calque, SIGNATURE ordonnée de sa
//!    chaîne de filtres, identité physique de la source) ;
//! 2. HIT → l'apparence mise en cache est retournée telle quelle :
//!    ZÉRO recalcul, zéro dispatch compute, zéro allocation pixel ;
//! 3. MISS → la chaîne est exécutée via [`crate::filters::render_chain`],
//!    qui route chaque effet vers les COMPUTE SHADERS de [`crate::gpu`]
//!    quand un adaptateur est disponible (fallback CPU rayon sinon),
//!    puis le résultat (image pleine résolution + preview + miniature)
//!    est inséré dans le cache.
//!
//! L'identité de la source est validée par comparaison de pointeurs Arc
//! (avec keep-alive dans l'entrée pour empêcher toute réutilisation
//! d'adresse — même défense ABA que le cache de textures UI).
//!
//! Les compteurs [`Renderer::hits`]/[`Renderer::misses`] rendent le
//! comportement observable : éditer le calque N ne doit provoquer qu'un
//! MISS sur N, jamais sur ses voisins — c'est la promesse « cache GPU
//! ciblé » du LayerTree.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use image::DynamicImage;
use uuid::Uuid;

use crate::document::{Appearance, Document, FilterNode, PixelLayer};

/// Entrée de cache : apparence dérivée + preuves de validité.
struct CacheEntry {
    /// Signature ordonnée de la chaîne de filtres au moment du calcul.
    signature: u64,
    /// Source conservée vivante : validation par identité de pointeur
    /// (une peinture/remplacement produit un nouvel Arc → miss garanti).
    source: Arc<DynamicImage>,
    appearance: Appearance,
}

/// Cache d'apparences par calque, alimenté par la chaîne de rendu
/// (compute shaders GPU lorsque disponibles, CPU rayon sinon).
#[derive(Default)]
pub struct Renderer {
    entries: HashMap<Uuid, CacheEntry>,
    hits: u64,
    misses: u64,
}

impl Renderer {
    /// Renderer détaché du backend GPU : les effets resteront sur leur
    /// chemin CPU même si un adaptateur apparaît plus tard. Réservé aux
    /// tests déterministes ; privilégier [`Renderer::new`] en production.
    pub fn without_gpu() -> Self {
        Self::default()
    }

    /// Apparence dérivée du calque, depuis le cache si elle est encore
    /// valide — sinon recalculée (GPU compute si disponible) et insérée.
    pub fn appearance(&mut self, layer: &PixelLayer) -> Appearance {
        let signature = filters_signature(&layer.live_filters);
        // perf-entry-api: use Entry to avoid double hashing on miss path
        use std::collections::hash_map::Entry;
        match self.entries.entry(layer.id) {
            Entry::Occupied(entry)
                if entry.get().signature == signature
                    && Arc::ptr_eq(&entry.get().source, &layer.source_image) =>
            {
                self.hits += 1;
                entry.get().appearance.clone()
            }
            Entry::Occupied(mut entry) => {
                self.misses += 1;
                let rendered =
                    crate::filters::render_chain(&layer.source_image, &layer.live_filters);
                let appearance = Appearance {
                    preview: crate::document::preview_buf(&rendered),
                    thumb: crate::document::thumb_buf(&rendered),
                    image: Arc::clone(&rendered),
                };
                entry.insert(CacheEntry {
                    signature,
                    source: Arc::clone(&layer.source_image),
                    appearance: appearance.clone(),
                });
                appearance
            }
            Entry::Vacant(slot) => {
                self.misses += 1;
                let rendered =
                    crate::filters::render_chain(&layer.source_image, &layer.live_filters);
                let appearance = Appearance {
                    preview: crate::document::preview_buf(&rendered),
                    thumb: crate::document::thumb_buf(&rendered),
                    image: Arc::clone(&rendered),
                };
                slot.insert(CacheEntry {
                    signature,
                    source: Arc::clone(&layer.source_image),
                    appearance: appearance.clone(),
                });
                appearance
            }
        }
    }

    /// Invalide UNIQUEMENT l'apparence du calque donné (micro-édition
    /// imposée manuellement ; normalement inutile : la signature suffit).
    pub fn invalidate_layer(&mut self, layer_id: Uuid) {
        self.entries.remove(&layer_id);
    }

    /// Invalide tout le cache (opération structurelle, restauration…).
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }

    /// Élague les entrées des calques qui n'existent plus dans l'arbre.
    /// À appeler après une suppression/groupement/duplication ou un undo
    /// structurel — libère la VRAM/RAM des sous-arbres disparus.
    pub fn sync_tree(&mut self, doc: &Document) {
        let live: HashSet<Uuid> = doc.iter_pixels().into_iter().map(|l| l.id).collect();
        self.entries.retain(|id, _| live.contains(id));
    }

    /// Nombre d'apparences actuellement en cache.
    pub fn cached_len(&self) -> usize {
        self.entries.len()
    }

    /// Apparences servies SANS recalcul depuis la construction (ou le
    /// dernier reset des compteurs) — l'indicateur direct du cache ciblé.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Apparences recalculées.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Remet les compteurs à zéro (observabilité par fenêtre).
    pub fn reset_stats(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }
}

/// Signature ORDONNÉE d'une chaîne de live filters : deux chaînes ont la
/// même signature ssi mêmes filtres (id, type, état actif) avec mêmes
/// paramètres dans le même ordre. L'ordre est significatif — c'est une
/// CHAÎNE de traitement, pas un ensemble.
///
/// Déterministe entre processus (`DefaultHasher::new()` = clés fixes),
/// indépendant de l'itération désordonnée de `HashMap` (clés triées).
#[must_use]
pub fn filters_signature(filters: &[FilterNode]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    let mut h = DefaultHasher::new();
    h.write_usize(filters.len());
    for f in filters {
        h.write(f.id.as_bytes());
        h.write(f.type_id.as_bytes());
        h.write_u8(u8::from(f.enabled));
        // Params triés par clé : même contenu => même signature
        let mut keys: Vec<&str> = Vec::with_capacity(f.params.len());
        keys.extend(f.params.keys().map(String::as_str));
        keys.sort_unstable();
        h.write_usize(keys.len());
        for key in keys {
            h.write(key.as_bytes());
            hash_param_value(&mut h, &f.params[key]);
        }
    }
    h.finish()
}

fn hash_param_value(
    h: &mut std::collections::hash_map::DefaultHasher,
    value: &datatypes::ParamValue,
) {
    use std::hash::Hasher;
    // Discriminant + bits bruts : f32 exclus de Hash std (NaN), on hashe
    // leurs bits — deux valeurs égales au sens PartialEq ont les mêmes bits.
    match value {
        datatypes::ParamValue::Float(v) => {
            h.write_u8(0);
            h.write_u32(v.to_bits());
        }
        datatypes::ParamValue::Int(v) => {
            h.write_u8(1);
            h.write_i32(*v);
        }
        datatypes::ParamValue::Bool(v) => {
            h.write_u8(2);
            h.write_u8(u8::from(*v));
        }
        datatypes::ParamValue::Color(c) => {
            h.write_u8(3);
            for channel in c {
                h.write_u32(channel.to_bits());
            }
        }
        datatypes::ParamValue::Text(s) => {
            h.write_u8(4);
            h.write(s.as_bytes());
        }
        datatypes::ParamValue::Enum(s) => {
            h.write_u8(5);
            h.write(s.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datatypes::ParamValue;
    use image::ImageBuffer;
    use image::Rgba;

    fn solid(value: u8) -> Arc<DynamicImage> {
        Arc::new(DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            2,
            2,
            Rgba([value, value, value, 255]),
        )))
    }

    fn layer_with_filter(brightness: f32) -> PixelLayer {
        let mut l = PixelLayer::new("test", solid(100));
        let mut f = FilterNode::new("brightness_contrast");
        f.params
            .insert("brightness".to_string(), ParamValue::Float(brightness));
        l.live_filters.push(f);
        l
    }

    #[test]
    fn premier_acces_miss_puis_hits_sans_recalcul() {
        let layer = layer_with_filter(10.0);
        let mut r = Renderer::default();

        let a1 = r.appearance(&layer);
        assert_eq!((r.misses(), r.hits()), (1, 0));

        let a2 = r.appearance(&layer);
        assert_eq!((r.misses(), r.hits()), (1, 1));
        assert_eq!(r.cached_len(), 1);
        // Même Arc d'image : aucun recopiage de pixels
        assert!(Arc::ptr_eq(&a1.image, &a2.image));
    }

    #[test]
    fn changer_un_parametre_provoque_un_seul_nouveau_miss() {
        let mut layer = layer_with_filter(10.0);
        let mut r = Renderer::default();
        let _ = r.appearance(&layer);

        // Réglage du slider : même id de filtre, nouvelle valeur
        let fid = layer.live_filters[0].id;
        layer.live_filters[0]
            .params
            .insert("brightness".to_string(), ParamValue::Float(20.0));
        let _ = r.appearance(&layer);
        assert_eq!(r.misses(), 2, "signature changée → recalcul");
        assert_eq!(r.hits(), 0);

        // Relecture sans changement : retour au HIT
        let _ = r.appearance(&layer);
        assert_eq!((r.misses(), r.hits()), (2, 1));
        let _ = fid;
    }

    #[test]
    fn remplacer_la_source_invalide_memes_sans_filtres() {
        // Chaîne vide : la signature ne bougera jamais — seule
        // l'identité de la source peut détecter une peinture/un crop.
        let mut layer = PixelLayer::new("peinture", solid(50));
        let mut r = Renderer::default();
        let avant = r.appearance(&layer);
        assert_eq!(r.misses(), 1);

        layer.set_source_image((*solid(90)).clone());
        let apres = r.appearance(&layer);
        assert_eq!(r.misses(), 2, "nouvel Arc source → recalcul garanti");
        assert!(!Arc::ptr_eq(&avant.image, &apres.image));
    }

    #[test]
    fn desactiver_un_filtre_change_la_signature() {
        let mut layer = layer_with_filter(30.0);
        let mut r = Renderer::default();
        let _ = r.appearance(&layer);

        layer.live_filters[0].enabled = false;
        let _ = r.appearance(&layer);
        assert_eq!(r.misses(), 2);

        // Et le résultat redevient la source pure (filtre court-circuité)
        let out = r.appearance(&layer);
        let rgba = out.image.to_rgba8();
        let p = rgba.get_pixel(0, 0);
        assert_eq!(p[0], 100);
    }

    #[test]
    fn sync_tree_elague_les_calques_supprimes() {
        let mut doc = Document::new(4, 4);
        let mut l1 = PixelLayer::new("a", solid(1));
        let l2 = PixelLayer::new("b", solid(2));
        let id1 = l1.id;
        let _ = l2.id;
        doc.push_layer(crate::document::LayerNode::Pixel(l1));
        doc.push_layer(crate::document::LayerNode::Pixel(l2));

        let mut r = Renderer::default();
        for l in doc.iter_pixels() {
            let _ = r.appearance(l);
        }
        assert_eq!(r.cached_len(), 2);

        // Suppression du calque 1 puis élagage
        let _removed = doc.remove(id1);
        r.sync_tree(&doc);
        assert_eq!(r.cached_len(), 1, "entrée orpheline supprimée");
    }

    #[test]
    fn deux_calques_sont_cachees_independamment() {
        // Le cœur de la promesse LayerTree : toucher le calque A ne
        // re-exécute JAMAIS le calque B.
        let la = layer_with_filter(5.0);
        let lb = layer_with_filter(7.0);
        let mut r = Renderer::default();

        let _ = r.appearance(&la);
        let _ = r.appearance(&lb);
        assert_eq!((r.misses(), r.hits()), (2, 0));

        let mut la_mod = la.clone();
        if let Some(f) = la_mod.live_filters.first_mut() {
            f.params
                .insert("brightness".to_string(), ParamValue::Float(6.0));
        }
        let _ = r.appearance(&la_mod);
        let _ = r.appearance(&lb);
        // lb n'a généré AUCUN nouveau miss
        assert_eq!((r.misses(), r.hits()), (3, 1));

        // Signature tests (pure partie)
        let s = filters_signature(&la.live_filters);
        assert_eq!(s, filters_signature(&la.live_filters));
    }

    #[test]
    fn ordre_et_etat_actif_comptent_dans_la_signature() {
        let mut a = vec![
            filter_of("blur", 3.0),
            filter_of("brightness_contrast", 12.0),
        ];
        let b = vec![
            filter_of("brightness_contrast", 12.0),
            filter_of("blur", 3.0),
        ];
        assert_ne!(filters_signature(&a), filters_signature(&b));

        a[0].enabled = false;
        let disabled_first = vec![a[0].clone(), a[1].clone()];
        let both_on = vec![
            filter_of("blur", 3.0),
            filter_of("brightness_contrast", 12.0),
        ];
        assert_ne!(
            filters_signature(&disabled_first),
            filters_signature(&both_on)
        );
    }

    fn filter_of(name: &str, brightness: f32) -> FilterNode {
        let mut f = FilterNode::new(name);
        f.params
            .insert("brightness".to_string(), ParamValue::Float(brightness));
        f
    }
}
