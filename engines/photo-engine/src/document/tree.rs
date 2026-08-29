use super::compositing::{fold_scope, needs_fallback_in, scope_half_extents};
use super::model::{Appearance, FilterNode, GroupLayer, LayerNode, PixelLayer, RgbaBuf};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use std::cell::RefCell;
use std::sync::Arc;
use uuid::Uuid;

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
    #[must_use]
    pub fn iter_pixels(&self) -> Vec<&PixelLayer> {
        let mut out = Vec::with_capacity(self.root.len());
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

    /// Réordonne par drag & drop : déplace `dragged` avant ou après `target`.
    pub fn reorder_before(&mut self, dragged: Uuid, target: Uuid, before: bool) -> bool {
        if dragged == target {
            return false;
        }
        // Empêche de déplacer un groupe dans son propre sous-arbre
        if let Some(node) = self.find(dragged)
            && let LayerNode::Group(g) = node
            && Self::contains_id(&g.children, target)
        {
            return false;
        }
        let Some(node) = self.remove(dragged) else {
            return false;
        };
        let Some((list, idx)) = find_owner_list(&mut self.root, target) else {
            // target disparu ? restaure à la fin
            self.push_layer(node);
            return false;
        };
        let at = if before { idx } else { idx + 1 };
        let at = at.min(list.len());
        list.insert(at, node);
        true
    }

    fn contains_id(nodes: &[LayerNode], id: Uuid) -> bool {
        for n in nodes {
            if n.id() == id {
                return true;
            }
            if let LayerNode::Group(g) = n
                && Self::contains_id(&g.children, id)
            {
                return true;
            }
        }
        false
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
    ///
    /// # Errors
    /// Retourne une erreur si le calque n'existe pas.
    pub fn flip(&mut self, id: Uuid, horizontal: bool) -> Result<(), String> {
        let layer = self.pixel_layer_mut(id).ok_or("calque introuvable")?;
        let flipped = if horizontal {
            layer.source_image.fliph()
        } else {
            layer.source_image.flipv()
        };
        if let Some(mask) = layer.mask.as_mut() {
            let dyn_mask = DynamicImage::ImageRgba8((*mask.image).clone());
            let flipped_mask = if horizontal {
                dyn_mask.fliph()
            } else {
                dyn_mask.flipv()
            }
            .to_rgba8();
            mask.image = Arc::new(flipped_mask);
            mask.touch();
        }
        layer.set_source_image(flipped);
        Ok(())
    }

    /// Rogne le calque au rect (coordonnées CALQUE, pixels). Destructif :
    /// le contenu reste en place dans le monde (le transform compense
    /// l'origine du crop). Erreur descriptive si le rect est invalide.
    ///
    /// # Errors
    /// Retourne une erreur si le calque est introuvable ou si le rectangle dépasse les bords.
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
        // Rogne le masque lié aux mêmes coordonnées
        if let Some(mask) = layer.mask.as_mut() {
            let dyn_mask = DynamicImage::ImageRgba8((*mask.image).clone());
            let cropped_mask = dyn_mask.crop_imm(x as u32, y as u32, w, h).to_rgba8();
            mask.image = Arc::new(cropped_mask);
            mask.touch();
        }
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
            Command::SetMaskEnabled { node_id, old, new } => {
                if let Some(mask) = self
                    .find_mut(node_id)
                    .and_then(|n| n.mask_mut())
                    .and_then(|m| m.as_mut())
                {
                    mask.enabled = new;
                    mask.touch();
                }
                Command::SetMaskEnabled {
                    node_id,
                    old: new,
                    new: old,
                }
            }
            Command::SetMaskInverted { node_id, old, new } => {
                if let Some(mask) = self
                    .find_mut(node_id)
                    .and_then(|n| n.mask_mut())
                    .and_then(|m| m.as_mut())
                {
                    mask.inverted = new;
                    mask.touch();
                }
                Command::SetMaskInverted {
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
            LayerNode::Pixel(l) => {
                if l.visible && l.opacity > 0.01 {
                    out.push(l);
                }
            }
            LayerNode::Group(g) => {
                if !g.visible || g.opacity <= 0.01 {
                    continue;
                }
                collect_pixels(&g.children, out)
            }
            LayerNode::Adjustment(_) => {}
        }
    }
}
