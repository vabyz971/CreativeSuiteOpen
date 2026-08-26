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

//! Modèle document « LayerTree » (arbre hiérarchique, style Affinity) +
//! compositing rapide.
//!
//! Architecture hybride :
//! - [`Document`] possède un arbre de [`LayerNode`] : calques pixels,
//!   groupes et calques d'ajustement, imbriqués à volonté.
//! - Chaque [`PixelLayer`] garde son image SOURCE intacte ; les retouches
//!   vivent dans une chaîne linéaire de [`FilterNode`] (live filters)
//!   évaluée par le moteur nodal interne (voir [`crate::filters`]).
//! - L'apparence dérivée (source × filtres) est calculée paresseusement et
//!   mise en cache par version d'apparence : un réglage ne recalcule que la
//!   chaîne du calque concerné, l'undo/redo ne recalcule que si nécessaire.
//!
//! Conçu pour l'interaction temps réel, comme les éditeurs pro :
//! - fusion DIRECTE dans un buffer accumulateur (aucune copie intermédiaire)
//! - chemin CPU rayon RGBA8 : la fusion est memory-bound, le round-trip GPU
//!   coûtait plus cher que le calcul lui-même — le GPU reste réservé aux
//!   filtres lourds (blur…)
//!
//! Ce module est PUR : aucun framework UI. Les aperçus/miniatures sont des
//! [`RgbaBuf`] partageables sans copie ; l'app les convertit en textures.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tampon RGBA8 partageable avec l'UI SANS copie (l'app en dérive ses
/// textures via `Bytes::from_owner` sur l'Arc).
#[derive(Clone)]
pub struct RgbaBuf {
    pub width: u32,
    pub height: u32,
    pub data: Arc<[u8]>,
}

impl RgbaBuf {
    pub fn from_vec(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            data: data.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Types de base du modèle
// ---------------------------------------------------------------------------

/// Mode de fusion d'un calque ou d'un groupe.
///
/// La représentation sérialisée est le nom de la variante (« Normal »,
/// « Multiply »…) — identique aux libellés historiques de l'app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
}

impl std::fmt::Display for BlendMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl BlendMode {
    /// Ordre d'affichage dans l'UI (listes déroulantes).
    pub const ALL: [BlendMode; 6] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
            BlendMode::Overlay => "Overlay",
            BlendMode::Darken => "Darken",
            BlendMode::Lighten => "Lighten",
        }
    }

    /// Identifiant numérique (shader GPU + CPU). Doit rester aligné sur
    /// `SHADER_BLEND` (gpu.rs) et les tests golden.
    pub fn id(self) -> u32 {
        match self {
            BlendMode::Normal => 0,
            BlendMode::Multiply => 1,
            BlendMode::Screen => 2,
            BlendMode::Overlay => 3,
            BlendMode::Darken => 4,
            BlendMode::Lighten => 5,
        }
    }
}

/// Transformation affine simple appliquée AU DRAW (modèle « state-only » :
/// changer une valeur ne régénère jamais les pixels).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    pub offset_x: f32,
    pub offset_y: f32,
    /// Rotation en degrés (sens horaire), appliquée autour du centre
    pub rotation_deg: f32,
    /// Échelle uniforme (1.0 = 100 %)
    pub scale: f32,
}

impl Default for Transform2D {
    /// Transformation identité.
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            rotation_deg: 0.0,
            scale: 1.0,
        }
    }
}

impl Transform2D {
    /// Dimensions de la bounding box après scale + rotation (autour du centre).
    /// Même convention que [`prepare_top`] pour le cadrage du plan infini.
    fn transformed_extents(&self, w0: f32, h0: f32) -> (f32, f32) {
        let s = self.scale.clamp(0.05, 8.0);
        let mut tw = w0 * s;
        let mut th = h0 * s;
        let rot = self.rotation_deg.rem_euclid(360.0);
        let is_0 = !(0.01..=359.99).contains(&rot);
        let is_90 = (rot - 90.0).abs() < 0.01;
        let is_180 = (rot - 180.0).abs() < 0.01;
        let is_270 = (rot - 270.0).abs() < 0.01;
        if is_90 || is_270 {
            std::mem::swap(&mut tw, &mut th);
        } else if !is_0 && !is_180 {
            // Rotation arbitraire : bounding box englobante
            let rad = rot.to_radians();
            let cos = rad.cos().abs();
            let sin = rad.sin().abs();
            let bbox_w = tw * cos + th * sin;
            let bbox_h = tw * sin + th * cos;
            tw = bbox_w;
            th = bbox_h;
        }
        (tw, th)
    }
}

/// Un filtre dynamique (live filter) : référence nommée vers un effet du
/// registre nodal + ses paramètres. La chaîne d'un calque est évaluée comme
/// un mini-graphe interne (input → filtres → output), voir [`crate::filters`].
#[derive(Debug, Clone)]
pub struct FilterNode {
    pub id: Uuid,
    /// type_id du registre d'effets (ex. « brightness_contrast », « blur »)
    pub type_id: String,
    pub params: HashMap<String, datatypes::ParamValue>,
    pub enabled: bool,
}

impl FilterNode {
    pub fn new(type_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            type_id: type_id.into(),
            params: HashMap::new(),
            enabled: true,
        }
    }
}

/// Compteur global monotone des versions d'apparence : garantit qu'un numéro
/// ne désigne jamais deux contenus différents (undo/redo inclus).
static APPEARANCE_VERSION: AtomicU64 = AtomicU64::new(1);

fn next_appearance_version() -> u64 {
    APPEARANCE_VERSION.fetch_add(1, Ordering::Relaxed)
}

/// Calque pixels : image SOURCE non destructive + chaîne de live filters.
///
/// L'image source ne change que lors d'événements explicites (peinture,
/// import, édition destructive volontaire) ; tout le reste vit dans les
/// filtres ou la transformation.
#[derive(Clone, Debug)]
pub struct PixelLayer {
    pub id: Uuid,
    pub name: String,
    pub source_image: Arc<DynamicImage>,
    /// Chaîne linéaire de traitement non destructif (ordre = ordre d'application)
    pub live_filters: Vec<FilterNode>,
    pub transform: Transform2D,
    /// 0..=100
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub visible: bool,
    /// Incrémenté à TOUTE mutation affectant l'apparence (source ou filtres).
    /// Clé d'invalidation du cache d'apparence du document.
    pub appearance_version: u64,
}

impl PixelLayer {
    pub fn new(name: impl Into<String>, image: Arc<DynamicImage>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            source_image: image,
            live_filters: Vec::new(),
            transform: Transform2D::default(),
            opacity: 100.0,
            blend_mode: BlendMode::Normal,
            visible: true,
            appearance_version: next_appearance_version(),
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.source_image.dimensions()
    }

    /// Bump la version d'apparence (à appeler après toute mutation pixels/filtres)
    pub fn touch(&mut self) {
        self.appearance_version = next_appearance_version();
    }

    /// Remplace le contenu pixels (peinture, édition destructive…)
    pub fn set_source_image(&mut self, image: DynamicImage) {
        self.source_image = Arc::new(image);
        self.touch();
    }
}

/// Groupe de calques : ses enfants composent entre eux d'abord, puis le
/// résultat est fondu dans le parent avec l'opacité/mode DU GROUPE.
/// Les transforms des enfants restent en coordonnées canvas (pas de
/// transform héritée — comme Affinity).
#[derive(Clone, Debug)]
pub struct GroupLayer {
    pub id: Uuid,
    pub name: String,
    pub children: Vec<LayerNode>,
    /// État d'affichage du panneau Calques (replié/déplié)
    pub collapsed: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub visible: bool,
}

impl GroupLayer {
    pub fn new(name: impl Into<String>, children: Vec<LayerNode>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            children,
            collapsed: false,
            opacity: 100.0,
            blend_mode: BlendMode::Normal,
            visible: true,
        }
    }
}

/// Calque d'ajustement : sa chaîne de filtres s'applique à la COMPOSITE
/// accumulée en dessous de lui, dans la limite de son groupe.
#[derive(Clone, Debug)]
pub struct AdjustmentLayer {
    pub id: Uuid,
    pub name: String,
    pub filters: Vec<FilterNode>,
    /// Pondération de l'ajustement (0 = invisible, 100 = plein)
    pub opacity: f32,
    pub visible: bool,
}

impl AdjustmentLayer {
    pub fn new(name: impl Into<String>, filters: Vec<FilterNode>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            filters,
            opacity: 100.0,
            visible: true,
        }
    }
}

/// Nœud de l'arbre : calque pixels, groupe ou ajustement.
#[derive(Clone, Debug)]
pub enum LayerNode {
    Pixel(PixelLayer),
    Group(GroupLayer),
    Adjustment(AdjustmentLayer),
}

impl LayerNode {
    pub fn id(&self) -> Uuid {
        match self {
            LayerNode::Pixel(l) => l.id,
            LayerNode::Group(g) => g.id,
            LayerNode::Adjustment(a) => a.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            LayerNode::Pixel(l) => &l.name,
            LayerNode::Group(g) => &g.name,
            LayerNode::Adjustment(a) => &a.name,
        }
    }

    pub fn set_name(&mut self, name: String) {
        match self {
            LayerNode::Pixel(l) => l.name = name,
            LayerNode::Group(g) => g.name = name,
            LayerNode::Adjustment(a) => a.name = name,
        }
    }

    pub fn visible(&self) -> bool {
        match self {
            LayerNode::Pixel(l) => l.visible,
            LayerNode::Group(g) => g.visible,
            LayerNode::Adjustment(a) => a.visible,
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        match self {
            LayerNode::Pixel(l) => l.visible = visible,
            LayerNode::Group(g) => g.visible = visible,
            LayerNode::Adjustment(a) => a.visible = visible,
        }
    }

    pub fn opacity(&self) -> f32 {
        match self {
            LayerNode::Pixel(l) => l.opacity,
            LayerNode::Group(g) => g.opacity,
            LayerNode::Adjustment(a) => a.opacity,
        }
    }

    pub fn set_opacity(&mut self, opacity: f32) {
        let clamped = opacity.clamp(0.0, 100.0);
        match self {
            LayerNode::Pixel(l) => l.opacity = clamped,
            LayerNode::Group(g) => g.opacity = clamped,
            LayerNode::Adjustment(a) => a.opacity = clamped,
        }
    }

    /// Mode de fusion — absent des calques d'ajustement.
    pub fn blend_mode(&self) -> Option<BlendMode> {
        match self {
            LayerNode::Pixel(l) => Some(l.blend_mode),
            LayerNode::Group(g) => Some(g.blend_mode),
            LayerNode::Adjustment(_) => None,
        }
    }

    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        match self {
            LayerNode::Pixel(l) => l.blend_mode = mode,
            LayerNode::Group(g) => g.blend_mode = mode,
            LayerNode::Adjustment(_) => {}
        }
    }

    /// Filtres du nœud (live filters OU chaîne d'ajustement).
    pub fn filters(&self) -> Option<&Vec<FilterNode>> {
        match self {
            LayerNode::Pixel(l) => Some(&l.live_filters),
            LayerNode::Adjustment(a) => Some(&a.filters),
            LayerNode::Group(_) => None,
        }
    }

    pub fn filters_mut(&mut self) -> Option<&mut Vec<FilterNode>> {
        match self {
            LayerNode::Pixel(l) => Some(&mut l.live_filters),
            LayerNode::Adjustment(a) => Some(&mut a.filters),
            LayerNode::Group(_) => None,
        }
    }

    /// Réassigne des identifiants frais (duplication) et invalide l'apparence.
    fn regenerate_ids(&mut self) {
        match self {
            LayerNode::Pixel(l) => {
                l.id = Uuid::new_v4();
                for f in &mut l.live_filters {
                    f.id = Uuid::new_v4();
                }
                l.touch();
            }
            LayerNode::Group(g) => {
                g.id = Uuid::new_v4();
                for c in &mut g.children {
                    c.regenerate_ids();
                }
            }
            LayerNode::Adjustment(a) => {
                a.id = Uuid::new_v4();
                for f in &mut a.filters {
                    f.id = Uuid::new_v4();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Document : racine de l'arbre + cache d'apparence
// ---------------------------------------------------------------------------

/// Apparence dérivée d'un calque pixels (source × filtres actifs).
#[derive(Clone)]
pub struct Appearance {
    /// Image pleine résolution (export, transforms)
    pub image: Arc<DynamicImage>,
    /// Buffer d'affichage interactif (downscalé au-delà de 2048 px)
    pub preview: RgbaBuf,
    /// Miniature 48×32 pour le panneau Calques
    pub thumb: RgbaBuf,
}

pub struct Document {
    pub width: u32,
    pub height: u32,
    /// Pile racine — index 0 = BAS de la pile (premier dessiné)
    pub root: Vec<LayerNode>,
    /// Cache d'apparences par calque ([`crate::renderer::Renderer`]) :
    /// validité par signature de filtres + identité de source, alimenté
    /// par la chaîne GPU compute / CPU rayon. Interior mutability car le
    /// cache est un détail de performance invisible depuis l'API (&self).
    cache: RefCell<crate::renderer::Renderer>,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            root: Vec::new(),
            cache: RefCell::new(crate::renderer::Renderer::default()),
        }
    }

    /// Reconstruit le document depuis un état restauré (undo/redo, projet).
    /// Le cache d'apparence est vidé : les entrées restaurées se
    /// revalideront par signature à la première demande.
    pub fn restore(&mut self, width: u32, height: u32, root: Vec<LayerNode>) {
        self.width = width;
        self.height = height;
        self.root = root;
        self.cache.borrow_mut().invalidate_all();
    }

    // -- Recherche ----------------------------------------------------------

    pub fn find(&self, id: Uuid) -> Option<&LayerNode> {
        find_in(&self.root, id)
    }

    pub fn find_mut(&mut self, id: Uuid) -> Option<&mut LayerNode> {
        find_in_mut(&mut self.root, id)
    }

    /// Accès typé au calque pixels.
    pub fn pixel_layer(&self, id: Uuid) -> Option<&PixelLayer> {
        match self.find(id) {
            Some(LayerNode::Pixel(l)) => Some(l),
            _ => None,
        }
    }

    pub fn pixel_layer_mut(&mut self, id: Uuid) -> Option<&mut PixelLayer> {
        match self.find_mut(id) {
            Some(LayerNode::Pixel(l)) => Some(l),
            _ => None,
        }
    }

    /// Liste plate des calques pixels, ordre de dessin (bas → haut), DFS.
    pub fn iter_pixels(&self) -> Vec<&PixelLayer> {
        let mut out = Vec::new();
        collect_pixels(&self.root, &mut out);
        out
    }

    pub fn pixel_count(&self) -> usize {
        self.iter_pixels().len()
    }

    /// Le rendu rapide « 1 texture par calque » est-il possible, ou faut-il
    /// passer par la composite CPU ? Vrai dès qu'un groupe a un mode de
    /// fusion non-Normal ou qu'un calque d'ajustement agit sur la pile.
    pub fn needs_fallback(&self) -> bool {
        needs_fallback_in(&self.root)
    }

    // -- Mutations structurelles ---------------------------------------------

    /// Ajoute un nœud au SOMMET de la pile racine.
    pub fn push_layer(&mut self, node: LayerNode) {
        self.root.push(node);
    }

    /// Insère `node` juste au-dessus de `anchor` (même parent).
    pub fn insert_above(&mut self, anchor: Uuid, node: LayerNode) -> bool {
        match find_owner_list(&mut self.root, anchor) {
            Some((list, idx)) => {
                list.insert(idx + 1, node);
                true
            }
            None => false,
        }
    }

    /// Détache le sous-arbre `id` de son parent.
    pub fn remove(&mut self, id: Uuid) -> Option<LayerNode> {
        let (list, idx) = find_owner_list(&mut self.root, id)?;
        Some(list.remove(idx))
    }

    /// Duplique le sous-arbre (nouveaux ids partout) et l'insère au-dessus.
    pub fn duplicate(&mut self, id: Uuid) -> Option<Uuid> {
        let mut copy = self.find(id)?.clone();
        copy.regenerate_ids();
        let new_id = copy.id();
        if !self.insert_above(id, copy) {
            return None;
        }
        Some(new_id)
    }

    /// Monte d'un cran parmi les frères (vers le haut de la pile).
    pub fn move_up(&mut self, id: Uuid) -> bool {
        let Some((list, idx)) = find_owner_list(&mut self.root, id) else {
            return false;
        };
        if idx + 1 >= list.len() {
            return false;
        }
        list.swap(idx, idx + 1);
        true
    }

    /// Descend d'un cran parmi les frères.
    pub fn move_down(&mut self, id: Uuid) -> bool {
        let Some((list, idx)) = find_owner_list(&mut self.root, id) else {
            return false;
        };
        if idx == 0 {
            return false;
        }
        list.swap(idx, idx - 1);
        true
    }

    /// Regroupe les nœuds donnés (mêmes frères) dans un nouveau groupe
    /// inséré à la place du plus bas d'entre eux. L'ordre relatif de la
    /// pile est préservé. Retourne l'id du groupe créé.
    pub fn group(&mut self, ids: &[Uuid]) -> Option<Uuid> {
        let first = *ids.first()?;
        let (list, _) = find_owner_list(&mut self.root, first)?;
        // Positions une fois pour toutes, triées (ordre pile)
        let mut idxs: Vec<usize> = Vec::with_capacity(ids.len());
        for id in ids {
            idxs.push(list.iter().position(|n| n.id() == *id)?);
        }
        idxs.sort_unstable();
        // Extraction du plus haut vers le plus bas, puis remise en ordre
        let mut children: Vec<LayerNode> = Vec::with_capacity(idxs.len());
        for &i in idxs.iter().rev() {
            children.push(list.remove(i));
        }
        children.reverse();
        let group = LayerNode::Group(GroupLayer::new("Groupe", children));
        let gid = group.id();
        let at = (*idxs.first()?).min(list.len());
        list.insert(at, group);
        Some(gid)
    }

    /// Dissout un groupe : ses enfants remontent à sa place dans le parent.
    /// Retourne les ids des enfants libérés.
    pub fn ungroup(&mut self, id: Uuid) -> Option<Vec<Uuid>> {
        let (list, idx) = find_owner_list(&mut self.root, id)?;
        if !matches!(list.get(idx), Some(LayerNode::Group(_))) {
            return None;
        }
        let node = list.remove(idx);
        let LayerNode::Group(group) = node else {
            return None;
        };
        let child_ids: Vec<Uuid> = group.children.iter().map(LayerNode::id).collect();
        for (off, child) in group.children.into_iter().enumerate() {
            list.insert(idx + off, child);
        }
        Some(child_ids)
    }

    // -- Éditions destructives (pixels) ---------------------------------------

    /// Retourne le calque horizontalement/verticalement (destructif).
    pub fn flip(&mut self, id: Uuid, horizontal: bool) -> Result<(), String> {
        let layer = self.pixel_layer_mut(id).ok_or("calque introuvable")?;
        let flipped = if horizontal {
            layer.source_image.fliph()
        } else {
            layer.source_image.flipv()
        };
        layer.set_source_image(flipped);
        Ok(())
    }

    /// Rogne le calque au rect (coordonnées CALQUE, pixels). Destructif :
    /// le contenu reste en place dans le monde (le transform compense
    /// l'origine du crop). Erreur descriptive si le rect est invalide.
    pub fn crop(&mut self, id: Uuid, x: i32, y: i32, w: u32, h: u32) -> Result<(), String> {
        let layer = self.pixel_layer_mut(id).ok_or("calque introuvable")?;
        let (iw, ih) = layer.dimensions();
        if w == 0 || h == 0 {
            return Err("rogner : dimensions nulles".into());
        }
        if x < 0 || y < 0 || x + w as i32 > iw as i32 || y + h as i32 > ih as i32 {
            return Err("rogner : la sélection dépasse les bords du calque".into());
        }
        let cropped = layer.source_image.crop_imm(x as u32, y as u32, w, h);
        // Compense l'origine : le pixel (x,y) d'origine reste à sa place monde
        layer.transform.offset_x += x as f32;
        layer.transform.offset_y += y as f32;
        layer.set_source_image(cropped);
        Ok(())
    }

    /// Remplace l'image source d'un calque pixels (peinture…).
    pub fn set_source_image(&mut self, id: Uuid, image: DynamicImage) -> bool {
        match self.pixel_layer_mut(id) {
            Some(layer) => {
                layer.set_source_image(image);
                true
            }
            None => false,
        }
    }

    // -- Filtres dynamiques ----------------------------------------------------

    /// Ajoute un filtre en fin de chaîne d'un calque/ajustement.
    /// Retourne l'id du filtre inséré.
    pub fn add_filter(&mut self, layer_id: Uuid, filter: FilterNode) -> Option<Uuid> {
        let fid = filter.id;
        self.find_mut(layer_id)?.filters_mut()?.push(filter);
        self.touch_pixel(layer_id);
        Some(fid)
    }

    /// Retire un filtre de la chaîne d'un calque/ajustement.
    pub fn remove_filter(&mut self, layer_id: Uuid, filter_id: Uuid) -> Option<FilterNode> {
        let filters = self.find_mut(layer_id)?.filters_mut()?;
        let idx = filters.iter().position(|f| f.id == filter_id)?;
        let removed = filters.remove(idx);
        self.touch_pixel(layer_id);
        Some(removed)
    }

    /// Modifie un paramètre de filtre (geste continu : coalescence côté app).
    pub fn set_filter_param(
        &mut self,
        layer_id: Uuid,
        filter_id: Uuid,
        key: impl Into<String>,
        value: datatypes::ParamValue,
    ) -> bool {
        let key = key.into();
        let Some(node) = self.find_mut(layer_id) else {
            return false;
        };
        let Some(filters) = node.filters_mut() else {
            return false;
        };
        let Some(f) = filters.iter_mut().find(|f| f.id == filter_id) else {
            return false;
        };
        f.params.insert(key, value);
        self.touch_pixel(layer_id);
        true
    }

    /// Active/désactive un filtre sans perdre ses réglages.
    pub fn set_filter_enabled(&mut self, layer_id: Uuid, filter_id: Uuid, enabled: bool) -> bool {
        let Some(node) = self.find_mut(layer_id) else {
            return false;
        };
        let Some(filters) = node.filters_mut() else {
            return false;
        };
        let Some(f) = filters.iter_mut().find(|f| f.id == filter_id) else {
            return false;
        };
        if f.enabled != enabled {
            f.enabled = enabled;
            self.touch_pixel(layer_id);
        }
        true
    }

    fn touch_pixel(&mut self, layer_id: Uuid) {
        if let Some(LayerNode::Pixel(l)) = self.find_mut(layer_id) {
            l.touch();
        }
    }

    // -- Commandes d'historique ------------------------------------------------

    /// Applique `command.new` au document et retourne l'INVERSE
    /// (old/new échangés) prêt à empiler pour le redo.
    ///
    /// Routage systématique par les setters existants : les invariants du
    /// modèle sont préservés (clamp d'opacité, bump de version d'apparence
    /// pour l'invalidation ciblée du cache, clamp de scale).
    ///
    /// Si le nœud cible a disparu, la commande est retournée telle quelle :
    /// l'empiler reste sûr (réapplication = no-op).
    pub fn apply_command(&mut self, command: crate::command::Command) -> crate::command::Command {
        use crate::command::Command;
        match command {
            Command::SetOpacity { layer_id, old, new } => {
                if let Some(node) = self.find_mut(layer_id) {
                    node.set_opacity(new);
                }
                Command::SetOpacity {
                    layer_id,
                    old: new,
                    new: old,
                }
            }
            Command::SetTransform { layer_id, old, new } => {
                if let Some(LayerNode::Pixel(l)) = self.find_mut(layer_id) {
                    l.transform = new;
                }
                Command::SetTransform {
                    layer_id,
                    old: new,
                    new: old,
                }
            }
            Command::SetBlendMode { node_id, old, new } => {
                if let Some(node) = self.find_mut(node_id) {
                    node.set_blend_mode(new);
                }
                Command::SetBlendMode {
                    node_id,
                    old: new,
                    new: old,
                }
            }
            Command::SetVisibility { node_id, old, new } => {
                if let Some(node) = self.find_mut(node_id) {
                    node.set_visible(new);
                }
                Command::SetVisibility {
                    node_id,
                    old: new,
                    new: old,
                }
            }
            Command::SetFilterParam {
                layer_id,
                filter_id,
                param_name,
                old,
                new,
            } => {
                self.set_filter_param(layer_id, filter_id, param_name.clone(), new.clone());
                Command::SetFilterParam {
                    layer_id,
                    filter_id,
                    param_name,
                    old: new,
                    new: old,
                }
            }
            Command::RenameLayer { node_id, old, new } => {
                if let Some(node) = self.find_mut(node_id) {
                    node.set_name(new.clone());
                }
                Command::RenameLayer {
                    node_id,
                    old: new,
                    new: old,
                }
            }
        }
    }

    // -- Apparence ------------------------------------------------------------

    /// Apparence dérivée du calque (source × filtres actifs), servie par
    /// le [`crate::renderer::Renderer`] : HIT = zéro recalcul, MISS =
    /// exécution de la chaîne (compute shaders si GPU disponible).
    /// Retourne des clones bon marché (Arc/RgbaBuf).
    pub fn appearance(&self, id: Uuid) -> Option<Appearance> {
        let layer = self.pixel_layer(id)?;
        Some(self.cache.borrow_mut().appearance(layer))
    }

    /// Image seule (chemin compositing — évite de régénérer preview/thumb).
    pub fn appearance_image(&self, id: Uuid) -> Option<Arc<DynamicImage>> {
        self.appearance(id).map(|a| a.image)
    }

    /// Miniature pour le panneau Calques (apparence dérivée).
    pub fn thumb(&self, id: Uuid) -> Option<RgbaBuf> {
        self.appearance(id).map(|a| a.thumb)
    }

    // -- Historique -------------------------------------------------------------

    /// Instantané complet (pixels partagés par Arc — quasi gratuit).
    pub fn snapshot(&self) -> crate::history::Snapshot {
        crate::history::Snapshot {
            doc_size: (self.width, self.height),
            root: self.root.clone(),
        }
    }

    /// Restaure un instantané.
    pub fn restore_snapshot(&mut self, snap: crate::history::Snapshot) {
        self.restore(snap.doc_size.0, snap.doc_size.1, snap.root);
    }

    // -- Compositing ---------------------------------------------------------------

    /// Composite pour le plan de travail infini : aucun crop au document.
    /// Le document reste centré (comme Affinity/Photoshop) et les calques
    /// hors document restent visibles. Retourne None si rien n'est visible.
    pub fn composite_preview(&self) -> Option<DynamicImage> {
        self.composite_scope(&self.root)
    }

    /// Composite du plan infini SANS le sous-arbre donné — utilisé pour le
    /// fond pré-calculé pendant un drag. Réutilise le CACHE D'APPARENCES de
    /// CE document : tous les calques restants produisent des HIT, le coût
    /// se limite au blend lui-même (contrairement à un clonage dans un
    /// document neuf dont le cache est froid).
    pub fn composite_preview_without(&self, exclude_id: Uuid) -> Option<DynamicImage> {
        // Clone structurel bon marché (Arcs partagés), sous-arbre masqué,
        // puis composite via LE MÊME cache que le document vivant.
        let mut hidden = self.root.clone();
        hide_subtree(find_in_mut(&mut hidden, exclude_id));
        if hidden.is_empty() {
            return None;
        }
        self.composite_scope(&hidden)
    }

    fn composite_scope(&self, nodes: &[LayerNode]) -> Option<DynamicImage> {
        let resolver = |id: Uuid| self.appearance_image(id);
        let (half_w, half_h) = scope_half_extents(nodes, self.width, self.height, &resolver);
        // Clamp pour éviter OOM (16384 ≈ 1 Go RGBA)
        let w = ((half_w * 2.0).clamp(1.0, 16384.0)) as u32;
        let h = ((half_h * 2.0).clamp(1.0, 16384.0)) as u32;
        let mut acc = ImageBuffer::from_pixel(w.max(1), h.max(1), Rgba([0, 0, 0, 0]));
        // Origine monde (0,0) = coin du buffer moins demi-tailles
        let origin_x = half_w - self.width as f32 / 2.0;
        let origin_y = half_h - self.height as f32 / 2.0;
        if !fold_scope(nodes, &mut acc, origin_x, origin_y, &resolver) {
            return None; // aucun calque visible/contribuant
        }
        Some(DynamicImage::ImageRgba8(acc))
    }

    /// Composite CROPÉ aux dimensions du document — utilisé pour l'export.
    pub fn composite(&self) -> Option<DynamicImage> {
        let img = self.composite_preview()?;
        let (w, h) = img.dimensions();
        if w <= self.width && h <= self.height {
            return Some(img);
        }
        let x = w.saturating_sub(self.width) / 2;
        let y = h.saturating_sub(self.height) / 2;
        Some(img.crop_imm(x, y, self.width.max(1), self.height.max(1)))
    }

    /// Statistiques du renderer — instrumentation des tests (cache chaud).
    #[cfg(test)]
    pub fn renderer_stats(&self) -> (u64, u64) {
        let r = self.cache.borrow();
        (r.hits(), r.misses())
    }
}

fn find_in(nodes: &[LayerNode], id: Uuid) -> Option<&LayerNode> {
    for n in nodes {
        if n.id() == id {
            return Some(n);
        }
        if let LayerNode::Group(g) = n
            && let Some(found) = find_in(&g.children, id)
        {
            return Some(found);
        }
    }
    None
}

fn find_in_mut(nodes: &mut [LayerNode], id: Uuid) -> Option<&mut LayerNode> {
    for n in nodes {
        if n.id() == id {
            return Some(n);
        }
        if let LayerNode::Group(g) = n
            && let Some(found) = find_in_mut(&mut g.children, id)
        {
            return Some(found);
        }
    }
    None
}

/// Trouve la liste possédant `id` (racine ou enfants d'un groupe) + index.
fn find_owner_list(nodes: &mut Vec<LayerNode>, id: Uuid) -> Option<(&mut Vec<LayerNode>, usize)> {
    if let Some(idx) = nodes.iter().position(|n| n.id() == id) {
        return Some((nodes, idx));
    }
    for n in nodes.iter_mut() {
        if let LayerNode::Group(g) = n
            && let Some(found) = find_owner_list(&mut g.children, id)
        {
            return Some(found);
        }
    }
    None
}

/// Masque récursivement un sous-arbre (drag d'un groupe = tout le groupe).
fn hide_subtree(node: Option<&mut LayerNode>) {
    let Some(node) = node else { return };
    node.set_visible(false);
    if let LayerNode::Group(g) = node {
        for child in &mut g.children {
            hide_subtree(Some(child));
        }
    }
}

fn collect_pixels<'a>(nodes: &'a [LayerNode], out: &mut Vec<&'a PixelLayer>) {
    for n in nodes {
        match n {
            LayerNode::Pixel(l) => out.push(l),
            LayerNode::Group(g) => collect_pixels(&g.children, out),
            LayerNode::Adjustment(_) => {}
        }
    }
}

fn needs_fallback_in(nodes: &[LayerNode]) -> bool {
    for n in nodes {
        if !n.visible() || n.opacity() <= 0.01 {
            continue;
        }
        match n {
            LayerNode::Pixel(l) => {
                if l.blend_mode != BlendMode::Normal {
                    return true;
                }
            }
            LayerNode::Group(g) => {
                if g.blend_mode != BlendMode::Normal || needs_fallback_in(&g.children) {
                    return true;
                }
            }
            LayerNode::Adjustment(a) => {
                if a.filters.iter().any(|f| f.enabled) {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Buffers d'affichage
// ---------------------------------------------------------------------------

/// Buffer d'affichage interactif (downscale au-delà de 2048 px).
pub fn preview_buf(image: &DynamicImage) -> RgbaBuf {
    const MAX_PREVIEW: u32 = 2048;
    let (w, h) = image.dimensions();
    if w.max(h) <= MAX_PREVIEW {
        let rgba = image.to_rgba8();
        RgbaBuf::from_vec(rgba.width(), rgba.height(), rgba.into_raw())
    } else {
        let preview = image.resize(
            MAX_PREVIEW,
            MAX_PREVIEW,
            ::image::imageops::FilterType::Triangle,
        );
        let rgba = preview.to_rgba8();
        RgbaBuf::from_vec(rgba.width(), rgba.height(), rgba.into_raw())
    }
}

/// Miniature 48×32 pour le panneau Calques.
pub fn thumb_buf(img: &DynamicImage) -> RgbaBuf {
    let t = img.resize(48, 32, ::image::imageops::FilterType::Triangle);
    let rgba = t.to_rgba8();
    RgbaBuf::from_vec(rgba.width(), rgba.height(), rgba.into_raw())
}

// ---------------------------------------------------------------------------
// Primitives de fusion CPU (inchangées, éprouvées par les tests golden)
// ---------------------------------------------------------------------------

/// Fusion d'un pixel : renvoie la couleur composée (top sur base)
#[inline]
fn blend_pixel(b: [f32; 4], t: [f32; 4], mode: u32) -> [f32; 4] {
    let blended = match mode {
        1 => [b[0] * t[0], b[1] * t[1], b[2] * t[2]], // Multiply
        2 => [
            // Screen
            1.0 - (1.0 - b[0]) * (1.0 - t[0]),
            1.0 - (1.0 - b[1]) * (1.0 - t[1]),
            1.0 - (1.0 - b[2]) * (1.0 - t[2]),
        ],
        3 => [
            // Overlay
            if b[0] < 0.5 {
                2.0 * b[0] * t[0]
            } else {
                1.0 - 2.0 * (1.0 - b[0]) * (1.0 - t[0])
            },
            if b[1] < 0.5 {
                2.0 * b[1] * t[1]
            } else {
                1.0 - 2.0 * (1.0 - b[1]) * (1.0 - t[1])
            },
            if b[2] < 0.5 {
                2.0 * b[2] * t[2]
            } else {
                1.0 - 2.0 * (1.0 - b[2]) * (1.0 - t[2])
            },
        ],
        4 => [b[0].min(t[0]), b[1].min(t[1]), b[2].min(t[2])], // Darken
        5 => [b[0].max(t[0]), b[1].max(t[1]), b[2].max(t[2])], // Lighten
        _ => [t[0], t[1], t[2]],                               // Normal
    };
    // Alpha compositing : top au-dessus de base, pondéré par l'alpha du top
    let ta = t[3];
    let a = ta + b[3] * (1.0 - ta);
    let mut out = [b[0], b[1], b[2], a];
    if a > 0.0001 {
        for c in 0..3 {
            out[c] = blended[c] * ta + b[c] * b[3] * (1.0 - ta);
            out[c] /= a;
        }
    }
    out
}

/// Fusionne `top` DANS `base` en place — zéro allocation intermédiaire.
fn blend_into(
    base: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    top: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    opacity: f32,
    blend_mode: BlendMode,
    offset_x: f32,
    offset_y: f32,
) {
    let op = (opacity / 100.0).clamp(0.0, 1.0);
    if op <= 0.0 {
        return;
    }
    let mode = blend_mode.id();
    let w = base.width();
    let (tw, th) = (top.width() as i32, top.height() as i32);
    let ox = offset_x.round() as i32;
    let oy = offset_y.round() as i32;
    let t_raw = top.as_raw();

    base.as_flat_samples_mut()
        .samples
        .par_chunks_exact_mut(4)
        .enumerate()
        .for_each(|(i, px)| {
            let x = (i as u32 % w) as i32;
            let y = (i as u32 / w) as i32;
            // Échantillonne le dessus à (x - ox, y - oy) — transparent hors bornes
            let tx = x - ox;
            let ty = y - oy;
            let t: [f32; 4] = if tx >= 0 && tx < tw && ty >= 0 && ty < th {
                let o = (ty as u32 * tw as u32 + tx as u32) as usize * 4;
                [
                    t_raw[o] as f32 / 255.0,
                    t_raw[o + 1] as f32 / 255.0,
                    t_raw[o + 2] as f32 / 255.0,
                    t_raw[o + 3] as f32 / 255.0 * op,
                ]
            } else {
                [0.0, 0.0, 0.0, 0.0]
            };
            // Pixel du dessus absent/translucide nul : base inchangée (skip rapide)
            if t[3] <= 0.001 {
                return;
            }
            let b: [f32; 4] = [
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
                px[3] as f32 / 255.0,
            ];
            let o = blend_pixel(b, t, mode);
            px[0] = (o[0].clamp(0.0, 1.0) * 255.0) as u8;
            px[1] = (o[1].clamp(0.0, 1.0) * 255.0) as u8;
            px[2] = (o[2].clamp(0.0, 1.0) * 255.0) as u8;
            px[3] = (o[3].clamp(0.0, 1.0) * 255.0) as u8;
        });
}

/// Item prêt à dessiner : image d'apparence + transformation.
struct DrawItem<'a> {
    image: &'a DynamicImage,
    transform: Transform2D,
}

/// Applique scale + rotation (autour du centre, comme le canvas) à l'image.
/// Retourne (buffer transformé, offset_x ajusté, offset_y ajusté).
fn prepare_top(item: &DrawItem<'_>) -> (ImageBuffer<Rgba<u8>, Vec<u8>>, f32, f32) {
    let (w0, h0) = item.image.dimensions();
    let scale = item.transform.scale.clamp(0.05, 8.0);
    let mut buf: ImageBuffer<Rgba<u8>, Vec<u8>> = match item.image {
        DynamicImage::ImageRgba8(b) => {
            if (scale - 1.0).abs() > 0.001 {
                let nw = ((w0 as f32 * scale).round() as u32).max(1);
                let nh = ((h0 as f32 * scale).round() as u32).max(1);
                image::imageops::resize(b, nw, nh, image::imageops::FilterType::Triangle)
            } else {
                b.clone()
            }
        }
        other => {
            let rgba = other.to_rgba8();
            if (scale - 1.0).abs() > 0.001 {
                let nw = ((w0 as f32 * scale).round() as u32).max(1);
                let nh = ((h0 as f32 * scale).round() as u32).max(1);
                image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle)
            } else {
                rgba
            }
        }
    };
    let (tw, th) = (buf.width() as f32, buf.height() as f32);
    let mut ox = item.transform.offset_x;
    let mut oy = item.transform.offset_y;
    let rot = item.transform.rotation_deg.rem_euclid(360.0); // ∈ [0, 360)
    // Multiples de 90° avec epsilon (les rotations libres arrondies
    // silencieusement seraient une erreur visuelle)
    let is_0 = !(0.01..=359.99).contains(&rot);
    let is_90 = (rot - 90.0).abs() < 0.01;
    let is_180 = (rot - 180.0).abs() < 0.01;
    let is_270 = (rot - 270.0).abs() < 0.01;
    if is_90 || is_270 {
        // 90° / 270° — swap via imageops (rapide, sans interpolation)
        let rotated = if is_90 {
            image::imageops::rotate90(&buf)
        } else {
            image::imageops::rotate270(&buf)
        };
        let (nw, nh) = (rotated.width() as f32, rotated.height() as f32);
        ox += (tw - nw) / 2.0;
        oy += (th - nh) / 2.0;
        buf = rotated;
    } else if is_180 {
        buf = image::imageops::rotate180(&buf);
    } else if !is_0 {
        // Rotation arbitraire : bounding box + échantillonnage bilinéaire
        let rad = rot.to_radians();
        let cos = rad.cos().abs();
        let sin = rad.sin().abs();
        let bbox_w = (tw * cos + th * sin).ceil().max(1.0) as u32;
        let bbox_h = (tw * sin + th * cos).ceil().max(1.0) as u32;
        let mut out = ImageBuffer::from_pixel(bbox_w, bbox_h, Rgba([0, 0, 0, 0]));
        let cx0 = tw / 2.0;
        let cy0 = th / 2.0;
        let cx1 = bbox_w as f32 / 2.0;
        let cy1 = bbox_h as f32 / 2.0;
        let cos_r = rad.cos();
        let sin_r = rad.sin();
        // Remplissage en parallèle (rayon)
        out.enumerate_pixels_mut()
            .par_bridge()
            .for_each(|(x, y, px)| {
                // Destination -> source (rotation inverse)
                let dx = x as f32 - cx1;
                let dy = y as f32 - cy1;
                let sx = dx * cos_r + dy * sin_r + cx0;
                let sy = -dx * sin_r + dy * cos_r + cy0;
                if sx >= 0.0 && sy >= 0.0 && sx < tw - 1.0 && sy < th - 1.0 {
                    // Bilinéaire
                    let x0 = sx.floor() as u32;
                    let y0 = sy.floor() as u32;
                    let fx = sx - x0 as f32;
                    let fy = sy - y0 as f32;
                    let p00 = buf.get_pixel(x0, y0);
                    let p10 = buf.get_pixel((x0 + 1).min(buf.width() - 1), y0);
                    let p01 = buf.get_pixel(x0, (y0 + 1).min(buf.height() - 1));
                    let p11 = buf.get_pixel(
                        (x0 + 1).min(buf.height() - 1),
                        (y0 + 1).min(buf.height() - 1),
                    );
                    for c in 0..4 {
                        let v = (p00[c] as f32 * (1.0 - fx) * (1.0 - fy)
                            + p10[c] as f32 * fx * (1.0 - fy)
                            + p01[c] as f32 * (1.0 - fx) * fy
                            + p11[c] as f32 * fx * fy)
                            .round() as u8;
                        px[c] = v;
                    }
                }
            });
        ox += (tw - bbox_w as f32) / 2.0;
        oy += (th - bbox_h as f32) / 2.0;
        buf = out;
    }
    (buf, ox, oy)
}

// ---------------------------------------------------------------------------
// Compositing récursif (groupes + ajustements)
// ---------------------------------------------------------------------------

type Resolver<'a> = &'a dyn Fn(Uuid) -> Option<Arc<DynamicImage>>;

/// Demi-extents nécessaires pour couvrir tous les items visibles d'une
/// portée, autour du centre document (plan infini). Récursif : un groupe
/// contribue via ses enfants (mêmes coordonnées canvas).
fn scope_half_extents(
    nodes: &[LayerNode],
    doc_w: u32,
    doc_h: u32,
    resolve: Resolver<'_>,
) -> (f32, f32) {
    let doc_cx = doc_w as f32 / 2.0;
    let doc_cy = doc_h as f32 / 2.0;
    let mut half = (doc_w as f32 / 2.0, doc_h as f32 / 2.0);
    extents_visit(nodes, doc_cx, doc_cy, resolve, &mut half);
    half
}

fn extents_visit(
    nodes: &[LayerNode],
    doc_cx: f32,
    doc_cy: f32,
    resolve: Resolver<'_>,
    half: &mut (f32, f32),
) {
    for node in nodes {
        match node {
            LayerNode::Group(g) => {
                if g.visible && g.opacity > 0.01 {
                    extents_visit(&g.children, doc_cx, doc_cy, resolve, half);
                }
            }
            LayerNode::Adjustment(_) => {}
            LayerNode::Pixel(l) => {
                if !l.visible || l.opacity <= 0.01 {
                    continue;
                }
                let Some(img) = resolve(l.id) else {
                    continue;
                };
                let (w0, h0) = (img.width() as f32, img.height() as f32);
                let s = l.transform.scale.clamp(0.05, 8.0);
                let (tw, th) = l.transform.transformed_extents(w0, h0);
                let cx = l.transform.offset_x + w0 * s / 2.0;
                let cy = l.transform.offset_y + h0 * s / 2.0;
                half.0 = half.0.max((cx - doc_cx).abs() + tw / 2.0);
                half.1 = half.1.max((cy - doc_cy).abs() + th / 2.0);
            }
        }
    }
}

/// Fond récursivement une portée dans l'accumulateur (déjà dimensionné).
/// Retourne true si au moins un élément a contribué.
///
/// Pixel : transform + blend direct. Groupe : les enfants composent d'abord
/// dans un buffer transparent, puis le résultat est fondu avec l'opacité et
/// le mode du groupe. Ajustement : la chaîne s'applique à l'accumulateur,
/// pondérée par opacité.
fn fold_scope(
    nodes: &[LayerNode],
    acc: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    origin_x: f32,
    origin_y: f32,
    resolve: Resolver<'_>,
) -> bool {
    let mut contributed = false;
    for node in nodes {
        match node {
            LayerNode::Pixel(l) => {
                if !l.visible || l.opacity <= 0.01 {
                    continue;
                }
                let Some(img) = resolve(l.id) else {
                    continue;
                };
                let item = DrawItem {
                    image: &img,
                    transform: l.transform,
                };
                let (top, ox, oy) = prepare_top(&item);
                blend_into(
                    acc,
                    &top,
                    l.opacity,
                    l.blend_mode,
                    ox + origin_x,
                    oy + origin_y,
                );
                contributed = true;
            }
            LayerNode::Group(g) => {
                if !g.visible || g.opacity <= 0.01 {
                    continue;
                }
                let mut sub = ImageBuffer::from_pixel(
                    acc.width().max(1),
                    acc.height().max(1),
                    Rgba([0, 0, 0, 0]),
                );
                if fold_scope(&g.children, &mut sub, origin_x, origin_y, resolve) {
                    blend_into(acc, &sub, g.opacity, g.blend_mode, 0.0, 0.0);
                    contributed = true;
                }
            }
            LayerNode::Adjustment(a) => {
                if !a.visible || a.opacity <= 0.01 {
                    continue;
                }
                if apply_adjustment(acc, &a.filters, a.opacity) {
                    contributed = true;
                }
            }
        }
    }
    contributed
}

/// Applique une chaîne d'ajustements à l'accumulateur, pondérée par
/// l'opacité (mix linéaire original ↔ ajusté). Retourne true si appliqué.
fn apply_adjustment(
    acc: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    filters: &[FilterNode],
    opacity: f32,
) -> bool {
    let weight = (opacity / 100.0).clamp(0.0, 1.0);
    if weight <= 0.0 || !filters.iter().any(|f| f.enabled) || acc.width() == 0 {
        return false;
    }
    let original = acc.clone();
    let adjusted = crate::filters::render_chain(
        &Arc::new(DynamicImage::ImageRgba8(original.clone())),
        filters,
    );
    let DynamicImage::ImageRgba8(adjusted_buf) = adjusted.as_ref() else {
        return false;
    };
    if adjusted_buf.dimensions() != acc.dimensions() {
        return false; // défense : effet exotique ayant redimensionné — ignoré
    }
    let raw_acc = acc.as_flat_samples_mut().samples;
    let raw_orig = original.as_raw();
    let raw_adj = adjusted_buf.as_raw();
    raw_acc
        .par_chunks_exact_mut(4)
        .zip(raw_orig.par_chunks_exact(4))
        .zip(raw_adj.par_chunks_exact(4))
        .for_each(|((dst, src), adj)| {
            for c in 0..4 {
                dst[c] = (src[c] as f32 * (1.0 - weight) + adj[c] as f32 * weight).round() as u8;
            }
        });
    true
}

#[cfg(test)]
mod tests {
    //! Tests « golden » du compositing CPU : images synthétiques minuscules
    //! dont la sortie est vérifiée pixel par pixel (tolérance ±1 pour les
    //! arrondis f32→u8). Toute régression de blend/transform est visible ici.
    //! Portés tels quels sur le modèle LayerTree, plus couverture arbre.

    use super::*;
    use crate::history::Snapshot;
    use datatypes::ParamValue;

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> DynamicImage {
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(w, h, Rgba(rgba)))
    }

    fn arc(img: &DynamicImage) -> Arc<DynamicImage> {
        Arc::new(img.clone())
    }

    fn pixel_node(
        img: &DynamicImage,
        opacity: f32,
        mode: BlendMode,
        ox: f32,
        oy: f32,
    ) -> LayerNode {
        let mut l = PixelLayer::new("test", arc(img));
        l.opacity = opacity;
        l.blend_mode = mode;
        l.transform.offset_x = ox;
        l.transform.offset_y = oy;
        LayerNode::Pixel(l)
    }

    fn doc_of(nodes: Vec<LayerNode>, w: u32, h: u32) -> Document {
        let mut doc = Document::new(w, h);
        doc.root = nodes;
        doc
    }

    fn px(img: &DynamicImage, x: u32, y: u32) -> [u8; 4] {
        let rgba = img.to_rgba8();
        let p = rgba.get_pixel(x, y);
        [p[0], p[1], p[2], p[3]]
    }

    fn assert_close(got: [u8; 4], exp: [u8; 4]) {
        for c in 0..4 {
            assert!(
                (got[c] as i16 - exp[c] as i16).abs() <= 1,
                "canal {c} : {got:?} ≠ {exp:?}"
            );
        }
    }

    #[test]
    fn normal_opaque_recouvre_et_deborde() {
        let base = solid(4, 4, [255, 0, 0, 255]);
        let top = solid(2, 2, [0, 255, 0, 255]);
        // Pile : rouge en bas, vert au-dessus décalé en (2,2)
        let doc = doc_of(
            vec![
                pixel_node(&base, 100.0, BlendMode::Normal, 0.0, 0.0),
                pixel_node(&top, 100.0, BlendMode::Normal, 2.0, 2.0),
            ],
            4,
            4,
        );
        let out = doc.composite().expect("composite non vide");
        assert_close(px(&out, 3, 3), [0, 255, 0, 255]); // zone recouverte
        assert_close(px(&out, 0, 0), [255, 0, 0, 255]); // zone de base
    }

    #[test]
    fn calque_hors_document_n_influence_pas_le_crop() {
        let base = solid(4, 4, [10, 20, 30, 255]);
        let top = solid(2, 2, [255, 255, 255, 255]);
        let doc = doc_of(
            vec![
                pixel_node(&base, 100.0, BlendMode::Normal, 0.0, 0.0),
                pixel_node(&top, 100.0, BlendMode::Normal, -10.0, -10.0),
            ],
            4,
            4,
        );
        let out = doc.composite().expect("composite non vide");
        assert_close(px(&out, 1, 1), [10, 20, 30, 255]);
    }

    #[test]
    fn modes_de_fusion_valeurs_connues() {
        // Base grise 50 % + top gris clair : valeurs canoniques des modes
        let base = solid(1, 1, [128, 128, 128, 255]);
        let top = solid(1, 1, [192, 192, 192, 255]);
        let stack_with = |mode: BlendMode| {
            vec![
                pixel_node(&base, 100.0, BlendMode::Normal, 0.0, 0.0),
                pixel_node(&top, 100.0, mode, 0.0, 0.0),
            ]
        };

        let cases = [
            (BlendMode::Multiply, (128 * 192) / 255), // ≈ 96
            (BlendMode::Screen, 255 - ((255 - 128) * (255 - 192)) / 255), // ≈ 224
            (BlendMode::Darken, 128),
            (BlendMode::Lighten, 192),
        ];
        for (mode, expected) in cases {
            let doc = doc_of(stack_with(mode), 1, 1);
            let out = doc.composite().expect("composite");
            let got = px(&out, 0, 0);
            assert_close(got, [expected as u8, expected as u8, expected as u8, 255]);
        }

        // Overlay sur base < 0.5 : 2·b·t
        let dark = solid(1, 1, [64, 64, 64, 255]);
        let doc = doc_of(
            vec![
                pixel_node(&dark, 100.0, BlendMode::Normal, 0.0, 0.0),
                pixel_node(&top, 100.0, BlendMode::Overlay, 0.0, 0.0),
            ],
            1,
            1,
        );
        let out = doc.composite().expect("composite");
        let exp = (2 * 64 * 192 / 255) as u8;
        assert_close(px(&out, 0, 0), [exp, exp, exp, 255]);
    }

    #[test]
    fn opacite_50_normal_sur_blanc() {
        let base = solid(2, 2, [255, 255, 255, 255]);
        let top = solid(2, 2, [0, 0, 0, 255]);
        let doc = doc_of(
            vec![
                pixel_node(&base, 100.0, BlendMode::Normal, 0.0, 0.0),
                pixel_node(&top, 50.0, BlendMode::Normal, 0.0, 0.0),
            ],
            2,
            2,
        );
        let out = doc.composite().expect("composite");
        assert_close(px(&out, 0, 0), [127, 127, 127, 255]);
    }

    #[test]
    fn calque_seul_translucide_sur_transparent() {
        // Un seul calque 50 % au-dessus du vide : l'alpha de sortie est
        // semi-transparent (plan de travail infini) — pas de fond magique.
        let top = solid(2, 2, [0, 0, 0, 255]);
        let doc = doc_of(
            vec![pixel_node(&top, 50.0, BlendMode::Normal, 0.0, 0.0)],
            2,
            2,
        );
        let out = doc.composite_preview().expect("composite");
        assert_close(px(&out, 0, 0), [0, 0, 0, 127]);
    }

    #[test]
    fn opacite_nulle_ou_cache_ignores() {
        let base = solid(2, 2, [9, 9, 9, 255]);
        let top = solid(2, 2, [250, 250, 250, 255]);
        let mut hidden = pixel_node(&top, 100.0, BlendMode::Normal, 0.0, 0.0);
        hidden.set_visible(false);
        let doc = doc_of(
            vec![
                pixel_node(&base, 100.0, BlendMode::Normal, 0.0, 0.0),
                hidden,
            ],
            2,
            2,
        );
        let out = doc.composite().expect("composite");
        assert_close(px(&out, 0, 0), [9, 9, 9, 255]);

        let transparent = doc_of(
            vec![pixel_node(&top, 0.0, BlendMode::Normal, 0.0, 0.0)],
            2,
            2,
        );
        assert!(transparent.composite_preview().is_none(), "rien de visible");
    }

    #[test]
    fn groupe_opacite_s_applique_aux_enfants_composes() {
        let rouge = solid(2, 2, [200, 0, 0, 255]);
        let bleu = solid(2, 2, [0, 0, 200, 255]);
        let mut doc = Document::new(2, 2);
        doc.push_layer(pixel_node(&rouge, 100.0, BlendMode::Normal, 0.0, 0.0));
        let mut group = GroupLayer::new(
            "g",
            vec![pixel_node(&bleu, 100.0, BlendMode::Normal, 0.0, 0.0)],
        );
        group.opacity = 50.0;
        doc.push_layer(LayerNode::Group(group));

        let out = doc.composite().expect("composite");
        // Groupe Normal 50 % sur rouge : mix (200,0,0)/(0,0,200) → (100,0,100)
        assert_close(px(&out, 0, 0), [100, 0, 100, 255]);
    }

    #[test]
    fn composite_sans_sous_arbre_reste_en_cache_chaud() {
        // Deux calques filtrés : le premier warm-up paie les MISS, ensuite
        // une composite EXCLUANT un sous-arbre ne doit générer AUCUN nouveau
        // miss — c'est ce qui rend le fond de drag instantané.
        use datatypes::ParamValue;
        let img = solid(2, 2, [120, 120, 120, 255]);
        let mut doc = Document::new(2, 2);
        for name in ["a", "b"] {
            let mut l = PixelLayer::new(name, arc(&img));
            let mut f = FilterNode::new("brightness_contrast");
            f.params
                .insert("brightness".into(), ParamValue::Float(10.0));
            l.live_filters.push(f);
            doc.push_layer(LayerNode::Pixel(l));
        }
        let id_b = doc.root[1].id();

        let _ = doc.composite_preview(); // warm-up : remplit le cache
        let (hits0, misses0) = doc.renderer_stats();
        assert_eq!(misses0, 2, "warm-up = un miss par calque filtré");

        let bg = doc
            .composite_preview_without(id_b)
            .expect("composite d'exclusion");
        let p = px(&bg, 0, 0);
        // Le calque b est masqué : seul a (+ son filtre) reste → 120+25 = 145
        assert_close(p, [145, 145, 145, 255]);

        // ZÉRO nouveau miss : tout est servi depuis le cache chaud
        let (_, misses1) = doc.renderer_stats();
        assert_eq!(misses1, misses0, "composite d'exclusion sans recalcul");
        assert!(
            doc.renderer_stats().0 > hits0,
            "les résolutions sont des hits"
        );
    }

    #[test]
    fn groupe_multiply_fond_la_composite_des_enfants() {
        // Enfant blanc seul dans un groupe Multiply → blanc × base = base
        let base = solid(2, 2, [128, 128, 128, 255]);
        let blanc = solid(2, 2, [255, 255, 255, 255]);
        let mut doc = Document::new(2, 2);
        doc.push_layer(pixel_node(&base, 100.0, BlendMode::Normal, 0.0, 0.0));
        let mut group = GroupLayer::new(
            "g",
            vec![pixel_node(&blanc, 100.0, BlendMode::Normal, 0.0, 0.0)],
        );
        group.blend_mode = BlendMode::Multiply;
        doc.push_layer(LayerNode::Group(group));

        let out = doc.composite().expect("composite");
        assert_close(px(&out, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn ajustement_applique_son_effet_a_la_pile_dessous() {
        let gris = solid(1, 1, [100, 100, 100, 255]);
        let mut doc = Document::new(1, 1);
        doc.push_layer(pixel_node(&gris, 100.0, BlendMode::Normal, 0.0, 0.0));
        let mut f = FilterNode::new("brightness_contrast");
        f.params
            .insert("brightness".into(), ParamValue::Float(40.0));
        doc.push_layer(LayerNode::Adjustment(AdjustmentLayer::new(
            "ajust",
            vec![f],
        )));
        let out = doc.composite().expect("composite");
        // 100 + 40*2.55 = 202
        assert_close(px(&out, 0, 0), [202, 202, 202, 255]);
    }

    #[test]
    fn ajustement_opacite_mixe_lineairement() {
        let gris = solid(1, 1, [100, 100, 100, 255]);
        let mut doc = Document::new(1, 1);
        doc.push_layer(pixel_node(&gris, 100.0, BlendMode::Normal, 0.0, 0.0));
        let mut f = FilterNode::new("brightness_contrast");
        f.params
            .insert("brightness".into(), ParamValue::Float(40.0));
        let mut adj = AdjustmentLayer::new("ajust", vec![f]);
        adj.opacity = 50.0;
        doc.push_layer(LayerNode::Adjustment(adj));
        let out = doc.composite().expect("composite");
        // mix 100 ↔ 202 à 50 % ≈ 151
        assert_close(px(&out, 0, 0), [151, 151, 151, 255]);
    }

    #[test]
    fn needs_fallback_detecte_groupes_et_ajustements() {
        let img = solid(1, 1, [1, 1, 1, 255]);

        // Pile plate Normal : rendu rapide possible
        let mut doc = Document::new(1, 1);
        doc.push_layer(pixel_node(&img, 100.0, BlendMode::Normal, 0.0, 0.0));
        assert!(!doc.needs_fallback());

        // Mode non-Normal sur un calque
        doc.root[0].set_blend_mode(BlendMode::Screen);
        assert!(doc.needs_fallback());

        // Groupe en mode non-Normal (même vide d'enfants)
        doc.root[0].set_blend_mode(BlendMode::Normal);
        let mut group = GroupLayer::new("g", vec![]);
        group.blend_mode = BlendMode::Overlay;
        doc.push_layer(LayerNode::Group(group));
        assert!(doc.needs_fallback());

        // Ajustement actif au-dessus d'une pile Normal
        let mut doc2 = Document::new(1, 1);
        doc2.push_layer(pixel_node(&img, 100.0, BlendMode::Normal, 0.0, 0.0));
        doc2.push_layer(LayerNode::Adjustment(AdjustmentLayer::new(
            "a",
            vec![FilterNode::new("blur")],
        )));
        assert!(doc2.needs_fallback());
    }

    #[test]
    fn arbre_operations_structurelles() {
        let a = solid(1, 1, [1, 0, 0, 255]);
        let b = solid(1, 1, [2, 0, 0, 255]);
        let c = solid(1, 1, [3, 0, 0, 255]);
        let mut doc = Document::new(4, 4);
        let na = pixel_node(&a, 100.0, BlendMode::Normal, 0.0, 0.0);
        let nb = pixel_node(&b, 100.0, BlendMode::Normal, 0.0, 0.0);
        let nc = pixel_node(&c, 100.0, BlendMode::Normal, 0.0, 0.0);
        let (ida, idb, idc) = (na.id(), nb.id(), nc.id());
        doc.push_layer(na);
        doc.push_layer(nb);
        doc.push_layer(nc);

        // move_up : b passe au-dessus de c
        assert!(doc.move_up(idb));
        assert_eq!(doc.root[2].id(), idb);
        // move_down deux fois : b revient en bas
        assert!(doc.move_down(idb));
        assert!(doc.move_down(idb));
        assert_eq!(doc.root[0].id(), idb);
        assert!(!doc.move_down(idb), "déjà en bas");

        // group(c, b) donné en désordre → ordre pile préservé [b, c]? Non :
        // b est en bas (index 0), c au-dessus (index 2 après moves ? vérifions)
        // État courant : [b, a, c] → grouper b et c donne groupe [b, c]
        let gid = doc.group(&[idc, idb]).expect("group");
        let LayerNode::Group(g) = &doc.root[0] else {
            panic!("groupe attendu en bas");
        };
        assert_eq!(g.children.len(), 2);
        assert_eq!(g.children[0].id(), idb, "ordre relatif préservé");
        assert_eq!(g.children[1].id(), idc);
        assert_eq!(doc.root[1].id(), ida);

        // duplicate du groupe : nouveaux ids partout
        let first_child_id = match &doc.root[0] {
            LayerNode::Group(g) => g.children[0].id(),
            _ => panic!("groupe attendu"),
        };
        let dup = doc.duplicate(gid).expect("duplicate");
        assert_ne!(dup, gid);
        let LayerNode::Group(g2) = doc.find(dup).expect("copie") else {
            panic!("groupe dupliqué attendu");
        };
        assert_ne!(g2.children[0].id(), first_child_id);

        // ungroup → enfants remontés, plus de groupe
        let freed = doc.ungroup(gid).expect("ungroup");
        assert_eq!(freed.len(), 2);
        assert!(doc.find(gid).is_none());
        // État : [b, c, copie_du_groupe(2 px), a]
        assert_eq!(doc.pixel_count(), 5);
        assert_eq!(doc.root.len(), 4);

        // remove d'une feuille racine
        let removed = doc.remove(ida).expect("remove");
        assert_eq!(removed.id(), ida);
        assert!(doc.find(ida).is_none());
        assert_eq!(doc.pixel_count(), 4);
    }

    #[test]
    fn snapshot_aller_retour_conserve_l_arbre() {
        let img = solid(2, 2, [7, 7, 7, 255]);
        let mut doc = Document::new(2, 2);
        let mut l = PixelLayer::new("fond", arc(&img));
        l.opacity = 80.0;
        l.blend_mode = BlendMode::Multiply;
        l.transform.offset_x = 1.0;
        doc.push_layer(LayerNode::Pixel(l));
        let id = doc.root[0].id();

        let snap: Snapshot = doc.snapshot();
        let mut restored = Document::new(0, 0);
        restored.restore_snapshot(snap);

        assert_eq!((restored.width, restored.height), (2, 2));
        assert_eq!(restored.pixel_count(), 1);
        let l = restored.pixel_layer(id).expect("calque restauré");
        assert_eq!(l.opacity, 80.0);
        assert_eq!(l.blend_mode, BlendMode::Multiply);
        assert_eq!(l.transform.offset_x, 1.0);
        assert_eq!(*l.source_image, *arc(&img));
    }

    #[test]
    fn live_filter_modifie_l_apparence_pas_la_source() {
        let img = solid(2, 2, [100, 100, 100, 255]);
        let mut doc = Document::new(2, 2);
        doc.push_layer(LayerNode::Pixel(PixelLayer::new("filtre", arc(&img))));
        let id = doc.root[0].id();

        let fid = doc
            .add_filter(id, FilterNode::new("brightness_contrast"))
            .expect("add_filter");
        assert!(
            doc.set_filter_param(id, fid, "brightness", ParamValue::Float(50.0)),
            "set_filter_param"
        );

        let appearance = doc.appearance(id).expect("apparence");
        assert_close(px(&appearance.image, 0, 0), [227, 227, 227, 255]); // 100 + 50*2.55
        // La source reste intacte (non destructif)
        assert_eq!(*doc.pixel_layer(id).unwrap().source_image, *arc(&img));

        // Désactivation → retour à la source
        assert!(doc.set_filter_enabled(id, fid, false));
        let off = doc.appearance(id).expect("apparence off");
        assert_close(px(&off.image, 0, 0), [100, 100, 100, 255]);

        // Suppression du filtre
        assert!(doc.remove_filter(id, fid).is_some());
        assert!(doc.pixel_layer(id).unwrap().live_filters.is_empty());
    }

    #[test]
    fn filtre_inconnu_est_transparent() {
        let img = solid(2, 2, [42, 42, 42, 255]);
        let mut doc = Document::new(2, 2);
        doc.push_layer(LayerNode::Pixel(PixelLayer::new("x", arc(&img))));
        let id = doc.root[0].id();
        doc.add_filter(id, FilterNode::new("effet_qui_n_existe_pas"));
        let appearance = doc.appearance(id).expect("apparence");
        assert_close(px(&appearance.image, 0, 0), [42, 42, 42, 255]);
    }

    #[test]
    fn crop_compense_le_transform_monde() {
        let mut doc = Document::new(4, 4);
        let mut b = ImageBuffer::from_pixel(4, 2, Rgba([0, 0, 0, 255]));
        b.put_pixel(3, 0, Rgba([200, 10, 20, 255]));
        let mut l = PixelLayer::new("crop", Arc::new(DynamicImage::ImageRgba8(b)));
        let id = l.id;
        doc.push_layer(LayerNode::Pixel(l));

        doc.crop(id, 2, 0, 2, 2).expect("crop valide");
        let l = doc.pixel_layer(id).expect("calque");
        assert_eq!((l.transform.offset_x, l.transform.offset_y), (2.0, 0.0));
        let img = l.source_image.to_rgba8();
        assert_eq!((img.width(), img.height()), (2, 2));
        // Le pixel rouge d'origine (3,0) devient (1,0) dans le calque rogné
        let p = img.get_pixel(1, 0);
        assert_eq!([p[0], p[1], p[2]], [200, 10, 20]);

        // Crop hors bornes → erreur propre
        assert!(doc.crop(id, -1, 0, 1, 1).is_err());
        assert!(doc.crop(id, 0, 0, 0, 5).is_err());
    }

    #[test]
    fn plan_infini_agrandit_autour_du_document() {
        let doc_img = solid(4, 4, [0, 0, 0, 255]);
        let big = solid(8, 8, [255, 255, 255, 255]);
        // Calque dépassant à gauche/haut : le composite preview ne doit pas rogner
        let doc = doc_of(
            vec![
                pixel_node(&doc_img, 100.0, BlendMode::Normal, 0.0, 0.0),
                pixel_node(&big, 100.0, BlendMode::Normal, -6.0, -6.0),
            ],
            4,
            4,
        );
        let out = doc.composite_preview().expect("preview non vide");
        let rgba = out.to_rgba8();
        assert!(
            rgba.width() >= 8 && rgba.height() >= 8,
            "plan infini trop petit"
        );
        // Coin haut-gauche du grand calque visible hors document :
        // le centre du buffer correspond au centre document → pixel blanc à (0,0)
        assert_eq!(rgba.get_pixel(0, 0)[0], 255);
    }

    #[test]
    fn flip_est_destructif_et_symetrique() {
        let mut doc = Document::new(4, 4);
        let mut b = ImageBuffer::from_pixel(2, 1, Rgba([0, 0, 0, 255]));
        b.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let mut l = PixelLayer::new("flip", Arc::new(DynamicImage::ImageRgba8(b)));
        let id = l.id;
        doc.push_layer(LayerNode::Pixel(l));
        doc.flip(id, true).expect("flip");
        let l = doc.pixel_layer(id).expect("calque");
        let img = l.source_image.to_rgba8();
        let avant_gauche = [255, 0, 0];
        // Après miroir horizontal, le rouge est passé à droite
        let p0 = img.get_pixel(0, 0);
        assert_ne!([p0[0], p0[1], p0[2]], avant_gauche);
        let p1 = img.get_pixel(1, 0);
        assert_eq!([p1[0], p1[1], p1[2]], avant_gauche);
    }
}
