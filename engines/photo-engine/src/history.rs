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

//! Historique document (undo/redo) — indépendant de l'UI.
//!
//! Stratégie : SNAPSHOTS complets du document. Un snapshot est quasi gratuit
//! (les pixels des calques sont partagés via `Arc`, seules les métadonnées
//! sont clonées) ; une édition destructive remplace l'Arc, donc les anciens
//! états gardent leurs pixels vivants à moindre coût mémoire borné par
//! [`History::limit`].
//!
//! Les gestes continus (sliders, saisie de nom) sont COALESCÉS par clé :
//! un seul point de restauration par geste au lieu d'un par pixel de slider.

use std::time::{Duration, Instant};

use crate::document::Layer;

/// État restaurable du document (calques + dimensions).
#[derive(Clone)]
pub struct Snapshot {
    pub doc_size: Option<(u32, u32)>,
    pub layers: Vec<Layer>,
}

/// Fenêtre de coalescence : deux poussées de même clé dans cette fenêtre ne
/// créent qu'un seul point de restauration (celui du début du geste).
const COALESCE_WINDOW: Duration = Duration::from_millis(800);

#[derive(Default)]
pub struct History {
    /// États historiques STRICTS (l'état courant n'y figure jamais),
    /// du plus ancien au plus récent.
    undo: Vec<Snapshot>,
    /// États futurs annulés (du plus récent au plus ancien).
    redo: Vec<Snapshot>,
    limit: usize,
    last_key: Option<(u64, Instant)>,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: 50,
            last_key: None,
        }
    }

    /// Réinitialise l'historique (nouveau document, ouverture de projet) :
    /// aucun retour arrière possible avant le prochain [`History::push`].
    pub fn reset(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last_key = None;
    }

    /// Pousse l'état PRÉ-mutation comme point de restauration.
    /// À appeler AVANT de modifier l'état courant.
    pub fn push(&mut self, pre: Snapshot) {
        self.push_inner(pre);
        self.last_key = None;
    }

    /// Variante pour les gestes continus : si `key` a déjà poussé il y a
    /// moins de [`COALESCE_WINDOW`], on garde le point de restauration
    /// EXISTANT (début du geste) et on rafraîchit seulement l'horodatage.
    pub fn push_coalesced(&mut self, key: u64, pre: Snapshot) {
        match self.last_key {
            Some((k, t)) if k == key && t.elapsed() < COALESCE_WINDOW => {
                self.last_key = Some((k, Instant::now()));
            }
            _ => {
                self.push_inner(pre);
                self.last_key = Some((key, Instant::now()));
            }
        }
    }

    fn push_inner(&mut self, pre: Snapshot) {
        self.redo.clear();
        self.undo.push(pre);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    /// Annule : échange l'état courant contre le précédent.
    /// Renvoie l'état à appliquer, ou None si rien à annuler.
    pub fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let target = self.undo.pop()?;
        self.redo.push(current);
        self.last_key = None;
        Some(target)
    }

    /// Rétablit : échange l'état courant contre le dernier annulé.
    pub fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let target = self.redo.pop()?;
        self.undo.push(current);
        self.last_key = None;
        Some(target)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Layer;
    use image::DynamicImage;
    use std::sync::Arc;

    fn layer(id: u64, value: u8) -> Layer {
        let img = DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
            2,
            2,
            image::Rgba([value, 0, 0, 255]),
        ));
        Layer::new(id, format!("c{id}"), Arc::new(img))
    }

    fn snap(values: &[u64]) -> Snapshot {
        Snapshot {
            doc_size: Some((4, 4)),
            layers: values.iter().map(|&id| layer(id, id as u8)).collect(),
        }
    }

    #[test]
    fn undo_redo_aller_retour() {
        let mut h = History::new();
        h.reset();
        assert!(!h.can_undo());

        let pre = snap(&[1]);
        h.push(pre);
        assert!(h.can_undo());
        // L'app a entre-temps muté vers [1, 2] ; on annule
        let restored = h.undo(snap(&[1, 2])).expect("undo");
        assert_eq!(restored.layers.len(), 1);
        assert!(!h.can_undo());

        // Rétablissement : l'état courant redevient un état passé
        let cur = snap(&[1]);
        let redone = h.redo(cur).expect("redo");
        assert_eq!(redone.layers.len(), 2);
        assert!(!h.can_redo());
        // Après un redo on peut naturellement re-annuler (va-et-vient A↔B)
        assert!(h.can_undo());
    }

    #[test]
    fn coalescence_un_seul_point_par_geste() {
        let mut h = History::new();
        h.reset();

        // Geste continu : 3 poussées rapprochées de même clé
        h.push_coalesced(7, snap(&[1]));
        h.push_coalesced(7, snap(&[1]));
        h.push_coalesced(7, snap(&[1]));

        // Une seule annulation disponible (le début du geste)
        let once = h.undo(snap(&[9]));
        assert!(once.is_some());
        assert!(!h.can_undo());
        assert!(h.undo(snap(&[9])).is_none());
    }

    #[test]
    fn limite_memoire_et_base_preservee() {
        let mut h = History::new();
        h.reset();
        for i in 0..200u64 {
            h.push(snap(&[i + 1]));
        }
        // On peut dépiler jusqu'à la base mais pas au-delà
        let mut steps = 0;
        while h.undo(snap(&[255])).is_some() {
            steps += 1;
            assert!(steps < 300, "boucle infinie");
        }
        assert!(steps >= 40 && steps <= 51, "limite non respectée: {steps}");
    }
}
