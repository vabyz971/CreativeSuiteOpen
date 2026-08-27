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

//! Moteur interne des live filters : la chaîne linéaire d'un calque est
//! traduite à la volée en mini-graphe nodal (`input_image → f₁ → … → fₙ →
//! output`) évalué par [`crate::processor`]. Le DAG existant ne disparaît
//! donc pas — il devient l'exécuteur des filtres dynamiques.
//!
//! Un type d'effet inconnu (projet d'une version plus récente, effet retiré)
//! est transparent : le processeur propage son entrée telle quelle.

use std::collections::HashMap;
use std::sync::Arc;

use datatypes::{NodeId, ParamValue, SocketType, Vec2};
use image::DynamicImage;
use suite_core::{Connection, Graph, Node};

use crate::document::FilterNode;

/// Position fictive des nœuds du mini-graphe (sans importance pour
/// l'évaluation, requise par le modèle de données).
const GRAPH_POS: Vec2 = Vec2 { x: 0.0, y: 0.0 };

/// Crée un filtre avec les paramètres PAR DÉFAUT de sa définition.
/// Retourne None si le type_id n'est pas dans le registre.
#[must_use]
pub fn new_filter(type_id: &str) -> Option<FilterNode> {
    let def = crate::registry::definition_for(type_id)?;
    let mut filter = FilterNode::new(def.type_id.clone());
    filter.params = def.default_params.clone();
    Some(filter)
}

/// Types d'effets éligibles en live filter / calque d'ajustement :
/// uniquement les effets image → image mono-entrée (pas input/output/mix/layer).
pub fn filterable_types() -> Vec<datatypes::NodeDefinition> {
    crate::registry::all_definitions()
        .into_iter()
        .filter(|d| {
            let mono_image = d.inputs.len() == 1
                && d.outputs.len() == 1
                && d.inputs[0].socket_type == SocketType::Image
                && d.outputs[0].socket_type == SocketType::Image;
            mono_image
                && matches!(
                    d.category,
                    datatypes::NodeCategory::Color | datatypes::NodeCategory::Filter
                )
        })
        .collect()
}

fn chain_node(graph: &mut Graph, type_id: &str, params: HashMap<String, ParamValue>) -> NodeId {
    let mut node = Node::new(NodeId(0), type_id.to_string(), String::new(), GRAPH_POS);
    node.params = params;
    node.preview_enabled = false;
    graph.add_node(node)
}

/// Applique la chaîne de filtres ACTIFS à `source`.
///
/// - Chaîne vide ou tout désactivé → retourne `source` tel quel (zéro coût).
/// - Sinon : construction du mini-graphe + évaluation complète.
/// - Échec d'évaluation ou effet inconnu → dégradation gracieuse sur
///   l'entrée (le processeur propage l'image à travers les effets inconnus).
pub fn render_chain(source: &Arc<DynamicImage>, filters: &[FilterNode]) -> Arc<DynamicImage> {
    let active: Vec<&FilterNode> = filters.iter().filter(|f| f.enabled).collect();
    if active.is_empty() {
        return Arc::clone(source);
    }

    let mut graph = Graph::new();
    let mut sources: HashMap<NodeId, Arc<DynamicImage>> = HashMap::with_capacity(1);

    let input = chain_node(&mut graph, "input_image", HashMap::new());
    sources.insert(input, Arc::clone(source));

    let mut prev = input;
    for f in &active {
        let node = chain_node(&mut graph, &f.type_id, f.params.clone());
        let _ = graph.connect(Connection::new(
            prev,
            "image",
            node,
            "image",
            SocketType::Image,
        ));
        prev = node;
    }
    let output = chain_node(&mut graph, "output", HashMap::new());
    let _ = graph.connect(Connection::new(
        prev,
        "image",
        output,
        "image",
        SocketType::Image,
    ));

    match crate::processor::evaluate(&graph, source, &sources) {
        Some(img) => Arc::new(img),
        None => Arc::clone(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datatypes::ParamValue;
    use image::ImageBuffer;
    use image::Rgba;

    fn grey(value: u8) -> Arc<DynamicImage> {
        Arc::new(DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            2,
            2,
            Rgba([value, value, value, 255]),
        )))
    }

    #[test]
    fn chaine_vide_renvoie_la_source_partagee() {
        let src = grey(10);
        // Arc identique : zéro recopie
        let out = render_chain(&src, &[]);
        assert!(Arc::ptr_eq(&src, &out));
    }

    #[test]
    fn filtre_desactive_est_transparent() {
        let src = grey(10);
        let mut f = new_filter("brightness_contrast").expect("définition connue");
        f.enabled = false;
        f.params
            .insert("brightness".into(), ParamValue::Float(100.0));
        let out = render_chain(&src, &[f]);
        assert!(Arc::ptr_eq(&src, &out));
    }

    #[test]
    fn brightness_applique_le_parametre() {
        let src = grey(100);
        let mut f = new_filter("brightness_contrast").expect("définition connue");
        f.params
            .insert("brightness".into(), ParamValue::Float(50.0));
        let contrast_default = f.params.get("contrast").cloned();
        let out = render_chain(&src, &[f]);
        let rgba = out.to_rgba8();
        let p = rgba.get_pixel(0, 0);
        // 100 + 50*2.55 = 227 (CPU path : image trop petite pour le GPU)
        assert!((p[0] as i16 - 227).abs() <= 2);
        // Les paramètres par défaut de la définition sont bien repartis
        assert_eq!(
            contrast_default,
            Some(ParamValue::Float(0.0)),
            "new_filter doit copier default_params"
        );
    }

    #[test]
    fn chaine_sequentielle_compose_les_effets() {
        let src = grey(100);
        let mut f1 = new_filter("brightness_contrast").expect("connu");
        f1.params
            .insert("brightness".into(), ParamValue::Float(40.0));
        let mut f2 = new_filter("color_correct").expect("connu");
        f2.params
            .insert("saturation".into(), ParamValue::Float(1.5));
        let out = render_chain(&src, &[f1, f2]);
        // Gris saturé reste gris ; la luminosité a bien été appliquée avant
        let rgba = out.to_rgba8();
        let p = rgba.get_pixel(0, 0);
        let expected = 100.0 + 40.0 * 2.55;
        assert!((p[0] as f32 - expected).abs() <= 3.0);
    }

    #[test]
    fn effet_inconnu_propage_son_entree() {
        let src = grey(42);
        let f = FilterNode::new("effet_inexistant");
        let out = render_chain(&src, &[f]);
        let rgba = out.to_rgba8();
        let p = rgba.get_pixel(0, 0);
        assert_eq!(p[0], 42);
    }

    #[test]
    fn filterable_types_exclut_entrees_sorties() {
        let ids: Vec<String> = filterable_types().into_iter().map(|d| d.type_id).collect();
        assert!(!ids.contains(&"input_image".to_string()));
        assert!(!ids.contains(&"output".to_string()));
        assert!(ids.contains(&"brightness_contrast".to_string()));
        assert!(ids.contains(&"blur".to_string()) || ids.contains(&"color_correct".to_string()));
    }
}
