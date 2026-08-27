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

//! Historique document HYBRIDE (undo/redo) — indépendant de l'UI.
//!
//! Deux types d'entrées selon la nature de l'édition :
//! - [`HistoryEntry::Snapshot`] — opérations destructives/structurelles
//!   (peinture, flip, crop, ajout/suppression/réordonnancement de calques,
//!   ouverture projet). Un snapshot est quasi gratuit pour les PIXELS
//!   (partagés via `Arc`), mais clone la structure d'arbre.
//! - [`HistoryEntry::Command`] — micro-éditions de paramètres (opacité,
//!   transform, fusion, visibilité, renommage, paramètre de filtre).
//!   Quelques octets par entrée : ZÉRO clonage d'arbre, et le retour
//!   [`UndoAction`] permet une invalidation rendu CIBLÉE.
//!
//! Coalescence temporelle (800 ms) conservée pour les gestes continus,
//! côté commandes ET snapshots. Particularité commandes : la coalescence
//! FUSIONNE les transitions (garde l'`old` du début du geste, met à jour
//! le `new` final) afin que undo restaure le début du geste ET redo
//! réapplique sa valeur FINALE.

use std::time::{Duration, Instant};

use crate::command::Command;
use crate::document::{Document, LayerNode};

/// État restaurable complet du document (arbre + dimensions).
#[derive(Clone)]
pub struct Snapshot {
    pub doc_size: (u32, u32),
    pub root: Vec<LayerNode>,
}

/// Entrée d'historique hybride.
#[derive(Clone)]
pub enum HistoryEntry {
    /// Snapshot complet (opérations destructives / structurelles)
    Snapshot(Snapshot),
    /// Commande légère old→new (micro-éditions)
    Command(Command),
}

/// Ce que l'app doit faire sur le rendu après un undo/redo :
/// - [`UndoAction::FullRestore`] : re-rendre TOUT (structure changée)
/// - [`UndoAction::Applied`] : la commande désormais en vigueur —
///   `cmd.render_event()` distingue invalidation ciblée vs recomposite
#[derive(Debug)]
pub enum UndoAction {
    FullRestore,
    Applied(Command),
}

/// Fenêtre de coalescence : deux poussées de même clé dans cette fenêtre ne
/// créent qu'un seul point de restauration (celui du début du geste).
const COALESCE_WINDOW: Duration = Duration::from_millis(800);

#[derive(Default)]
pub struct History {
    /// États historiques STRICTS (l'état courant n'y figure jamais),
    /// du plus ancien au plus récent.
    undo: Vec<HistoryEntry>,
    /// États futurs annulés (du plus récent au plus ancien).
    redo: Vec<HistoryEntry>,
    limit: usize,
    last_key: Option<(u64, Instant)>,
}

impl History {
    /// Create a new history with default limit.
    #[must_use]
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: 50,
            last_key: None,
        }
    }

    /// Réinitialise l'historique (nouveau document, ouverture de projet) :
    /// aucun retour arrière possible avant la prochaine poussée.
    pub fn reset(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last_key = None;
    }

    // -- Poussées ------------------------------------------------------------

    /// Pousse un snapshot PRÉ-mutation (opérations destructives).
    /// À appeler AVANT de modifier le document.
    pub fn push_snapshot(&mut self, pre: Snapshot) {
        self.push_inner(HistoryEntry::Snapshot(pre));
        self.last_key = None;
    }

    /// Pousse une commande légère AVEC coalescence (gestes continus :
    /// sliders, saisie de nom, réglage de filtre…).
    ///
    /// La commande doit décrire la transition old→new COURANTE ; si la
    /// même clé est repoussée dans la fenêtre, l'entrée existante est
    /// FUSIONNÉE (`old` initial conservé, `new` rafraîchi) plutôt que
    /// dupliquée — sinon le redo restaurerait une valeur intermédiaire.
    ///
    /// L'application au document reste à la charge de l'appelant
    /// ([`Document::apply_command`]), AVANT ou APRÈS cette poussée.
    pub fn push_command(&mut self, key: u64, command: Command) {
        match self.last_key {
            Some((k, t)) if k == key && t.elapsed() < COALESCE_WINDOW => {
                let merged = match self.undo.last_mut() {
                    Some(HistoryEntry::Command(top)) => top.merge_forward(&command),
                    _ => false,
                };
                if !merged {
                    // Même clé mais édition incompatible : entrée distincte
                    self.push_inner(HistoryEntry::Command(command));
                }
                self.last_key = Some((k, Instant::now()));
            }
            _ => {
                self.push_inner(HistoryEntry::Command(command));
                self.last_key = Some((key, Instant::now()));
            }
        }
    }

    /// Pousse une commande légère SANS coalescence (éditions discrètes :
    /// bascule de visibilité, rotation ±90°, changement de mode…).
    pub fn push_command_immediate(&mut self, command: Command) {
        self.push_inner(HistoryEntry::Command(command));
        self.last_key = None;
    }

    fn push_inner(&mut self, entry: HistoryEntry) {
        self.redo.clear();
        self.undo.push(entry);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    // -- Annulation / rétablissement ------------------------------------------

    /// Annule : applique l'INVERSE de la dernière commande au document
    /// (ou restaure le snapshot précédent) et renvoie l'action de rendu.
    ///
    /// Convention : les deux piles stockent des commandes FORWARD (la
    /// transition qui mène vers le futur). Undo applique donc
    /// [`Command::inverse`], redo réapplique telle quelle.
    pub fn undo(&mut self, doc: &mut Document) -> Option<UndoAction> {
        match self.undo.pop()? {
            HistoryEntry::Snapshot(snapshot) => {
                let current = doc.snapshot();
                self.redo.push(HistoryEntry::Snapshot(current));
                doc.restore_snapshot(snapshot);
                self.last_key = None;
                Some(UndoAction::FullRestore)
            }
            HistoryEntry::Command(forward) => {
                let backward = forward.inverse();
                doc.apply_command(backward.clone());
                self.redo.push(HistoryEntry::Command(forward.clone()));
                self.last_key = None;
                Some(UndoAction::Applied(backward))
            }
        }
    }

    /// Rétablit : réapplique la dernière commande annulée (ou restaure le
    /// snapshot annulé) et pousse l'entrée sur la pile undo.
    pub fn redo(&mut self, doc: &mut Document) -> Option<UndoAction> {
        match self.redo.pop()? {
            HistoryEntry::Snapshot(snapshot) => {
                let current = doc.snapshot();
                self.undo.push(HistoryEntry::Snapshot(current));
                doc.restore_snapshot(snapshot);
                self.last_key = None;
                Some(UndoAction::FullRestore)
            }
            HistoryEntry::Command(forward) => {
                doc.apply_command(forward.clone());
                self.undo.push(HistoryEntry::Command(forward.clone()));
                self.last_key = None;
                Some(UndoAction::Applied(forward))
            }
        }
    }

    /// Whether an undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether a redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::PixelLayer;
    use datatypes::ParamValue;
    use image::DynamicImage;
    use std::sync::Arc;
    use uuid::Uuid;

    fn pixel(value: u8) -> LayerNode {
        let img = DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
            2,
            2,
            image::Rgba([value, 0, 0, 255]),
        ));
        LayerNode::Pixel(PixelLayer::new(format!("c{value}"), Arc::new(img)))
    }

    fn snap(values: &[u8]) -> Snapshot {
        Snapshot {
            doc_size: (4, 4),
            root: values.iter().map(|&v| pixel(v)).collect(),
        }
    }

    fn doc_with(values: &[u8]) -> Document {
        let mut doc = Document::new(4, 4);
        doc.restore_snapshot(snap(values));
        doc
    }

    fn opacity_cmd(layer_id: Uuid, old: f32, new: f32) -> Command {
        Command::SetOpacity { layer_id, old, new }
    }

    #[test]
    fn snapshot_undo_redo_aller_retour() {
        let mut h = History::new();
        let mut doc = doc_with(&[1]);
        assert!(!h.can_undo());

        h.push_snapshot(doc.snapshot());
        assert!(h.can_undo());

        // L'app a entre-temps ajouté un calque ; on annule
        doc.push_layer(pixel(2));
        let action = h.undo(&mut doc).expect("undo");
        assert!(matches!(action, UndoAction::FullRestore));
        assert_eq!(doc.root.len(), 1);
        assert!(!h.can_undo());
        assert!(h.can_redo());

        // Rétablissement
        let action = h.redo(&mut doc).expect("redo");
        assert!(matches!(action, UndoAction::FullRestore));
        assert_eq!(doc.root.len(), 2);
        assert!(!h.can_redo());
        assert!(h.can_undo(), "va-et-vient possible");
    }

    #[test]
    fn commande_undo_redo_retablit_les_deux_valeurs() {
        let mut h = History::new();
        let mut doc = doc_with(&[1]);
        let id = doc.root[0].id();

        // Édition 50 → 80 enregistrée en commande
        let cmd = opacity_cmd(id, 50.0, 80.0);
        h.push_command_immediate(cmd);
        let _inverse = doc.apply_command(opacity_cmd(id, 50.0, 80.0));
        assert!((doc.find(id).unwrap().opacity() - 80.0).abs() < f32::EPSILON);

        // Undo → 50, Redo → 80
        let action = h.undo(&mut doc).expect("undo");
        assert!(matches!(&action, UndoAction::Applied(c)
                if c.render_event() == crate::command::RenderEvent::NodeInvalidated(id)));
        assert!((doc.find(id).unwrap().opacity() - 50.0).abs() < f32::EPSILON);

        let action = h.redo(&mut doc).expect("redo");
        assert!(matches!(action, UndoAction::Applied(_)));
        assert!((doc.find(id).unwrap().opacity() - 80.0).abs() < f32::EPSILON);
    }

    #[test]
    fn coalescence_fusionne_et_le_restaurateur_est_correct() {
        let mut h = History::new();
        let mut doc = doc_with(&[1]);
        let id = doc.root[0].id();

        // Geste continu 50→60→70→80 : trois poussées même clé
        for (old, new) in [(50.0, 60.0), (60.0, 70.0), (70.0, 80.0)] {
            h.push_command(coalesce(id), opacity_cmd(id, old, new));
            let _ = doc.apply_command(opacity_cmd(id, old, new));
        }
        // Une seule entrée empilée
        assert_eq!(h.undo.len(), 1, "coalescence = une seule entrée");

        // Undo → début du geste (50)
        h.undo(&mut doc).expect("undo");
        assert!((doc.find(id).unwrap().opacity() - 50.0).abs() < f32::EPSILON);
        assert!(!h.can_undo());

        // Redo → valeur FINALE du geste (80), pas la première (60)
        h.redo(&mut doc).expect("redo");
        assert!((doc.find(id).unwrap().opacity() - 80.0).abs() < f32::EPSILON);
    }

    #[test]
    fn coalescence_snapshot_conservee_aussi() {
        // Les snapshots gardent leur propre fenêtre via last_key partagé :
        // deux poussées snapshot ne coalescent PAS (push_snapshot reset),
        // mais une commande suivie d'un snapshot démarre un nouveau point.
        let mut h = History::new();
        h.push_command(coalesce(Uuid::nil()), opacity_cmd(Uuid::nil(), 1.0, 2.0));
        h.reset(); // isole le test suivant
        h.push_snapshot(snap(&[1]));
        h.push_snapshot(snap(&[2]));
        assert_eq!(h.undo.len(), 2, "snapshots jamais fusionnés");
    }

    #[test]
    fn melange_hybride_sequencement_coherent() {
        let mut h = History::new();
        let mut doc = doc_with(&[1]);
        let id = doc.root[0].id();

        // 1) snapshot pré-structurel puis ajout d'un calque
        h.push_snapshot(doc.snapshot());
        doc.push_layer(pixel(2));

        // 2) micro-édition opacité 100 → 40 en commande
        h.push_command_immediate(opacity_cmd(id, 100.0, 40.0));
        let _ = doc.apply_command(opacity_cmd(id, 100.0, 40.0));

        // Undo #1 : opacité revient à 100 (commande)
        h.undo(&mut doc).expect("undo cmd");
        assert!((doc.find(id).unwrap().opacity() - 100.0).abs() < f32::EPSILON);

        // Undo #2 : le calque ajouté disparaît (snapshot)
        h.undo(&mut doc).expect("undo snap");
        assert_eq!(doc.root.len(), 1);
        assert!(!h.can_undo());

        // Redo ×2 : snapshot (calque revient) puis commande (opacité 40)
        h.redo(&mut doc).expect("redo snap");
        assert_eq!(doc.root.len(), 2);
        h.redo(&mut doc).expect("redo cmd");
        assert!((doc.find(id).unwrap().opacity() - 40.0).abs() < f32::EPSILON);
    }

    #[test]
    fn commande_sur_noeud_absent_est_un_no_op_sur() {
        let mut h = History::new();
        let mut doc = doc_with(&[1]);
        let fantome = uuid::Uuid::new_v4();

        h.push_command_immediate(opacity_cmd(fantome, 10.0, 90.0));
        h.undo(&mut doc).expect("undo s'exécute sans panic");
        // Rien n'a bougé, ni plantage
        assert_eq!(doc.root.len(), 1);
    }

    #[test]
    fn limite_memoire_et_base_preservee_hybride() {
        let mut h = History::new();
        let mut doc = doc_with(&[1]);
        for i in 0..200u64 {
            if i % 2 == 0 {
                h.push_snapshot(doc.snapshot());
            } else {
                let id = doc.root[0].id();
                let v = (i % 100) as f32;
                h.push_command_immediate(opacity_cmd(id, v, v + 1.0));
            }
        }
        let mut steps = 0;
        while h.undo(&mut doc).is_some() {
            steps += 1;
            assert!(steps < 300, "boucle infinie");
        }
        assert!(steps >= 40 && steps <= 51, "limite non respectée: {steps}");
    }

    #[test]
    fn filtre_param_commande_invalide_le_cache_apparence() {
        use crate::document::FilterNode;
        let mut doc = Document::new(2, 2);
        let img = DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
            2,
            2,
            image::Rgba([100, 100, 100, 255]),
        ));
        let mut layer = PixelLayer::new("f", Arc::new(img));
        layer
            .live_filters
            .push(FilterNode::new("brightness_contrast"));
        doc.push_layer(LayerNode::Pixel(layer));
        let layer_id = doc.root[0].id();
        let filter_id = doc.pixel_layer(layer_id).unwrap().live_filters[0].id;

        let version_avant = doc.pixel_layer(layer_id).unwrap().appearance_version;
        let cmd = Command::SetFilterParam {
            layer_id,
            filter_id,
            param_name: "brightness".into(),
            old: ParamValue::Float(0.0),
            new: ParamValue::Float(50.0),
        };
        let inverse = doc.apply_command(cmd);
        // Version bumpée → le cache d'apparence se sait obsolète
        assert!(
            doc.pixel_layer(layer_id).unwrap().appearance_version > version_avant,
            "apply_command doit invalider l'apparence"
        );
        // Inverse cohérent
        match inverse {
            Command::SetFilterParam { old, new, .. } => {
                assert_eq!(old, ParamValue::Float(50.0));
                assert_eq!(new, ParamValue::Float(0.0));
            }
            other => panic!("variante inattendue : {other:?}"),
        }
    }

    // -- Helpers -----------------------------------------------------------

    fn coalesce(id: Uuid) -> u64 {
        (id.as_u128() as u64).wrapping_mul(16).wrapping_add(1)
    }
}
