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

//! Moteur d'évaluation nodal CPU/GPU — dispatch vers `nodes/`
//! Évaluation topologique avec cache ; les nœuds désactivés (`enabled = false`)
//! sont bypassés : leur première entrée image traverse telle quelle.

use crate::nodes::{self, NodeCtx};
use datatypes::NodeId;
use image::DynamicImage;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use suite_core::Graph;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// N'évalue que les ancêtres du nœud Output (chemin critique)
    OutputOnly,
    /// Ancêtres + nœuds preview déconnectés (aperçus Blender-like)
    WithPreviews,
}

/// Boucle d'évaluation partagée.
/// `base` contient les résultats précédents encore valides : ils sont
/// réutilisés tels quels au lieu d'être recalculés.
fn run(
    graph: &Graph,
    original: &DynamicImage,
    sources: &HashMap<NodeId, Arc<DynamicImage>>,
    base: &HashMap<NodeId, DynamicImage>,
    cache: &mut HashMap<NodeId, DynamicImage>,
    mode: Mode,
) {
    let Ok(order) = graph.topological_order() else {
        return;
    };
    let ancestors = graph.output_ancestors();

    for id in order {
        let Some(node) = graph.get(id) else { continue };

        let eval = match mode {
            Mode::OutputOnly => ancestors.contains(&id),
            Mode::WithPreviews => {
                ancestors.contains(&id) || node.preview_enabled || node.type_id == "input_image"
            }
        };
        if !eval {
            continue;
        }

        // Réutilise le résultat précédent s'il est encore valide
        if let Some(img) = base.get(&id) {
            cache.insert(id, img.clone());
            continue;
        }

        let ctx = NodeCtx {
            graph,
            cache,
            original,
            sources,
        };

        // Bypass : nœud désactivé → la première image d'entrée traverse telle quelle
        let img = if !node.enabled {
            match ctx.any_input(id).cloned() {
                Some(img) => img,
                None => continue,
            }
        } else {
            match nodes::find(&node.type_id) {
                Some(effect) => match (effect.apply)(&ctx, id) {
                    Some(img) => img,
                    None => continue,
                },
                // Type inconnu : propage l'entrée si elle existe
                None => match ctx.any_input(id).cloned() {
                    Some(img) => img,
                    None => continue,
                },
            }
        };
        cache.insert(id, img);
    }
}

/// Évalue le graphe complet et retourne l'image du nœud Output.
pub fn evaluate(
    graph: &Graph,
    original: &DynamicImage,
    sources: &HashMap<NodeId, Arc<DynamicImage>>,
) -> Option<DynamicImage> {
    evaluate_incremental(
        graph,
        original,
        sources,
        &Default::default(),
        &Default::default(),
    )
}

/// Évaluation incrémentale : réutilise `prev_cache` sauf pour les nœuds de
/// `affected` (+ leurs descendants déjà retirés par l'app).
pub fn evaluate_incremental(
    graph: &Graph,
    original: &DynamicImage,
    sources: &HashMap<NodeId, Arc<DynamicImage>>,
    prev_cache: &HashMap<NodeId, DynamicImage>,
    affected: &HashSet<NodeId>,
) -> Option<DynamicImage> {
    let base = valid_base(graph, prev_cache, affected);
    let mut cache = HashMap::new();
    run(
        graph,
        original,
        sources,
        &base,
        &mut cache,
        Mode::OutputOnly,
    );
    cache.get(&graph.find_output_node()?).cloned()
}

/// Évalue tous les ancêtres (+ nœuds preview) et retourne le cache complet
/// (pour les aperçus Blender-like).
pub fn evaluate_with_cache(
    graph: &Graph,
    original: &DynamicImage,
    sources: &HashMap<NodeId, Arc<DynamicImage>>,
) -> HashMap<NodeId, DynamicImage> {
    let empty = HashMap::new();
    let mut cache = HashMap::new();
    run(
        graph,
        original,
        sources,
        &empty,
        &mut cache,
        Mode::WithPreviews,
    );
    cache
}

/// Version incrémentale de [`evaluate_with_cache`] : réutilise `prev_cache`
/// sauf pour les nœuds de `affected` (+ leurs descendants retirés par l'app).
pub fn evaluate_with_cache_incremental(
    graph: &Graph,
    original: &DynamicImage,
    sources: &HashMap<NodeId, Arc<DynamicImage>>,
    prev_cache: &HashMap<NodeId, DynamicImage>,
    affected: &HashSet<NodeId>,
) -> HashMap<NodeId, DynamicImage> {
    let base: HashMap<NodeId, DynamicImage> = valid_base(graph, prev_cache, affected);
    let mut cache = HashMap::new();
    run(
        graph,
        original,
        sources,
        &base,
        &mut cache,
        Mode::WithPreviews,
    );
    cache
}

/// Entrées du cache précédent encore valides (nœud existant, non affecté)
fn valid_base(
    graph: &Graph,
    prev_cache: &HashMap<NodeId, DynamicImage>,
    affected: &HashSet<NodeId>,
) -> HashMap<NodeId, DynamicImage> {
    prev_cache
        .iter()
        .filter(|(id, _)| graph.get(**id).is_some() && !affected.contains(*id))
        .map(|(id, img)| (*id, img.clone()))
        .collect()
}

/// Convertit une DynamicImage en Handle natif iced (texture GPU).
pub fn to_handle(img: &DynamicImage) -> iced::widget::image::Handle {
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
}
