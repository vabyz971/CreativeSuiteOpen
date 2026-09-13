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

//! Nœuds d'effets Photo — un fichier par effet.
//! Chaque effet expose :
//!  - `definition()` : sa définition (sockets + params) pour le registre UI
//!  - `apply(ctx)` : son évaluation image -> image dans une chaîne linéaire
//!    (voir [`crate::filters`]).
//!
//! Ajouter un nouvel effet = créer un fichier ici + l'enregistrer dans `all()`.

pub mod blur;
pub mod brightness_contrast;
pub mod color_correct;
pub mod input;
pub mod layer;
pub mod mix;
pub mod output;

use crate::gpu;
use datatypes::{NodeDefinition, ParamValue};
use image::DynamicImage;
use std::collections::HashMap;

/// Contexte passé à chaque effet lors de l'évaluation d'une chaîne linéaire
/// de filtres : les paramètres du nœud courant + l'image produite par
/// l'étape précédente. Aucune structure de graphe — `filters.rs` plie la
/// chaîne séquentiellement.
pub struct NodeCtx<'a> {
    /// Paramètres du nœud courant (ex. `{"brightness": Float(50.0)}`)
    pub params: &'a HashMap<String, ParamValue>,
    /// Image produite par l'étape précédente de la chaîne
    pub input_image: Option<&'a DynamicImage>,
    /// Image source d'origine (fallback si pas d'entrée)
    pub original: &'a DynamicImage,
}

impl NodeCtx<'_> {
    /// Image arrivant de l'étape précédente de la chaîne
    pub fn input(&self) -> Option<&DynamicImage> {
        self.input_image
    }

    /// Valeur float d'un paramètre avec défaut
    pub fn param(&self, key: &str, default: f32) -> f32 {
        self.params
            .get(key)
            .and_then(|v| v.as_float())
            .unwrap_or(default)
    }
}

/// Un effet enregistré : définition UI + fonction d'évaluation
pub struct Effect {
    pub definition: NodeDefinition,
    pub apply: fn(&NodeCtx) -> Option<DynamicImage>,
}

/// Tous les effets du moteur — point d'entrée unique pour registre et processeur.
pub fn all() -> Vec<Effect> {
    vec![
        input::effect(),
        output::effect(),
        brightness_contrast::effect(),
        blur::effect(),
        mix::effect(),
        layer::effect(),
        color_correct::effect(),
    ]
}

/// Retrouve un effet par type_id
pub fn find(type_id: &str) -> Option<Effect> {
    all().into_iter().find(|e| e.definition.type_id == type_id)
}

// ---------------------------------------------------------------------------
// Helpers partagés par les effets CPU/GPU
// ---------------------------------------------------------------------------

/// Convertit une DynamicImage en RGBA8 brut
pub fn to_rgba8(img: &DynamicImage) -> image::ImageBuffer<image::Rgba<u8>, Vec<u8>> {
    img.to_rgba8()
}

pub use gpu::{apply_blur_gpu, apply_brightness_contrast_gpu, apply_mix_gpu, apply_saturation_gpu};
