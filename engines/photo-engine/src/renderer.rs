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

//! Squelette du pipeline de rendu GPU ciblé.
//!
//! Objectif final : rendre un [`crate::document::Document`] calque par
//! calque — si les live filters d'un calque n'ont pas changé, sa texture
//! GPU est réutilisée depuis le cache (clé = id du calque + SIGNATURE de
//! sa chaîne de filtres) ; seuls les calques modifiés passent par les
//! compute shaders.
//!
//! État actuel : la GESTION DU CACHE est fonctionnelle et testable
//! ([`Renderer::invalidate_layer`], [`Renderer::invalidate_all`],
//! signature stable des chaînes de filtres), mais le rendu lui-même n'est
//! pas implémenté — [`Renderer::render`] retourne `None` sans jamais
//! paniquer. Le rendu effectif passera par les pipelines compute déjà
//! présents dans [`crate::gpu`] et restera aligné sur le modèle
//! « state-only » (opacité/transform au draw, zéro régénération).

use std::collections::HashMap;

use uuid::Uuid;

use crate::document::{Document, FilterNode};

/// Clé d'entrée du cache de textures :
/// - identité du calque (stable, Uuid)
/// - signature de sa chaîne de live filters (voir [`filters_signature`])
pub type TextureCacheKey = (Uuid, u64);

/// Rendu GPU avec cache de textures par calque.
///
/// Possède son propre couple Device/Queue pour rester autonome ; le
/// branchement futur sur [`crate::gpu::GpuContext`] (device partagé,
/// zéro duplication) ne changera pas cette API.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    cache: HashMap<TextureCacheKey, wgpu::Texture>,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            cache: HashMap::new(),
        }
    }

    /// Accès au device (branchement des compute pipelines à venir).
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Accès à la file (soumissions à venir).
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Nombre d'entrées actuellement en cache (observabilité/tests).
    pub fn cached_len(&self) -> usize {
        self.cache.len()
    }

    /// Invalide UNIQUEMENT les textures du calque donné — appelé après une
    /// micro-édition (opacité, transform, paramètre de filtre…).
    pub fn invalidate_layer(&mut self, layer_id: Uuid) {
        self.cache.retain(|(id, _), _| *id != layer_id);
    }

    /// Invalide tout le cache — appelé après une opération structurelle
    /// (ajout/suppression/réordonnancement, undo snapshot, ouverture projet).
    pub fn invalidate_all(&mut self) {
        self.cache.clear();
    }

    /// Insère une texture produite pour la clé donnée (réservé au rendu).
    pub fn store(&mut self, key: TextureCacheKey, texture: wgpu::Texture) {
        self.cache.insert(key, texture);
    }

    /// Texture en cache pour cette clé, si présente et encore valide.
    pub fn cached(&self, key: &TextureCacheKey) -> Option<&wgpu::Texture> {
        self.cache.get(key)
    }

    /// Rend l'arbre complet : PAS ENCORE IMPLÉMENTÉ.
    ///
    /// Contrat prévu (voir doc de module) : pour chaque calque visible de
    /// bas en haut — 1) clé `(id, filters_signature)` → texture en cache ;
    /// 2) sinon compute shaders sur la source puis insertion en cache ;
    /// 3) blit final avec opacité + mode de fusion.
    ///
    /// Retourne `None` tant que le pipeline n'existe pas — aucun panic,
    /// les appelants gardent leur chemin de secours (composite CPU).
    pub fn render(&mut self, tree: &Document) -> Option<wgpu::TextureView> {
        let _ = tree;
        None
    }
}

/// Signature ORDONNÉE d'une chaîne de live filters : deux chaînes ont la
/// même signature ssi mêmes filtres (id, type, état actif) avec mêmes
/// paramètres dans le même ordre. L'ordre est significatif — c'est une
/// CHAINE de traitement, pas un ensemble.
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
        let mut keys: Vec<&str> = f.params.keys().map(String::as_str).collect();
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

    fn filter(name: &str, brightness: f32) -> FilterNode {
        let mut f = FilterNode::new(name);
        f.params
            .insert("brightness".to_string(), ParamValue::Float(brightness));
        f
    }

    #[test]
    fn signature_stable_pour_chaine_identique() {
        let chain = vec![filter("brightness_contrast", 12.0), filter("blur", 3.0)];
        assert_eq!(filters_signature(&chain), filters_signature(&chain));
    }

    #[test]
    fn parametre_change_change_la_signature() {
        let a = vec![filter("brightness_contrast", 12.0)];
        let b = vec![filter("brightness_contrast", 13.0)];
        assert_ne!(filters_signature(&a), filters_signature(&b));
    }

    #[test]
    fn ordre_de_la_chaine_est_significatif() {
        let a = vec![filter("brightness_contrast", 12.0), filter("blur", 3.0)];
        let b = vec![filter("blur", 3.0), filter("brightness_contrast", 12.0)];
        assert_ne!(filters_signature(&a), filters_signature(&b));
    }

    #[test]
    fn etat_actif_compte_dans_la_signature() {
        let mut disabled = vec![filter("blur", 3.0)];
        disabled[0].enabled = false;
        let enabled = vec![filter("blur", 3.0)];
        assert_ne!(filters_signature(&disabled), filters_signature(&enabled));
    }

    #[test]
    fn chaine_vide_a_une_signature_distincte() {
        let empty: Vec<FilterNode> = Vec::new();
        let one = vec![filter("blur", 3.0)];
        assert_ne!(filters_signature(&empty), filters_signature(&one));
    }
}
