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

//! Types partagés du canvas image pour le chemin GPU `layer_canvas` :
//! outils, événements, aperçu de trait tuilé, géométrie des poignées de
//! transformation et constantes associées.
//!
//! L'ancien programme `iced::widget::canvas` (`ImageCanvas`, `CanvasLayer`,
//! `view_with_tool`) a été supprimé : l'affichage est assuré par
//! `layer_canvas`, qui réutilise les types de ce module pour rester
//! compatible avec la logique applicative existante.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

use iced::mouse;
use iced::{Point, Rectangle, Size, Vector};
use uuid::Uuid;

/// Tool active on the image canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasTool {
    Hand,
    Zoom,
    Select,
    Move,
    /// Brush: paints on selected layer
    Brush,
    /// Eraser: erases (reduces alpha) on selected layer
    Eraser,
    /// Eyedropper: pick a color from the composited canvas
    Eyedropper,
}

/// Poignée du visualiseur de transformation (Affinity-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformHandle {
    /// Déplacer la sélection (intérieur de la boîte)
    Move,
    /// Redimensionner via une poignée d'angle
    Corner(Corner),
    /// Rotation autour du centre (poignée au-dessus du bord haut)
    Rotate,
    /// Côté droit → cisaille X selon Y (inclinaison horizontale)
    SkewX,
    /// Côté bas → cisaille Y selon X (inclinaison verticale)
    SkewY,
    /// Échelle proportionnelle — poignée carrée 0.12× au-delà du coin
    /// bas-droite (déplace l'image dans son échelle, aspect conservé)
    Scale,
}

/// Coin du rectangle sélectionné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl TransformHandle {
    /// Identifiant stable pour l'ancre de geste côté app.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Corner(_) => "resize",
            Self::Rotate => "rotate",
            Self::SkewX => "skew_x",
            Self::SkewY => "skew_y",
            Self::Scale => "scale",
        }
    }
}

/// Events emitted by the image canvas.
#[derive(Debug, Clone)]
pub enum ImageCanvasEvent {
    Pan(Vector),
    ZoomPan {
        zoom: f32,
        pan: Vector,
    },
    ZoomAt {
        zoom: f32,
        pan: Vector,
    },
    SelectRect(Option<Rectangle>),
    /// Canvas viewport size (to compute "fit to image")
    Viewport(Size),
    /// Clic sur zone vide (outil Déplacer) → désélectionner
    ClearSelection,
    /// Début d'un geste de transformation (poignée + curseur en doc).
    /// `id` = Some(layer) quand le clic sélectionne un autre calque
    /// (Pick) → le geste Déplacement commence en même temps.
    TransformStart {
        id: Option<Uuid>,
        kind: TransformHandle,
        doc: (f32, f32),
    },
    /// Curseur pendant un geste (coordonnées document).
    /// `uniform` = Ctrl enfoncé → redimensionnement PROPORTIONNEL.
    /// `snap` = Shift enfoncé → rotation aimantée (multiples de l'angle).
    TransformCursor {
        doc: (f32, f32),
        uniform: bool,
        snap: bool,
    },
    /// Fin de geste — l'app commit la transformation
    TransformEnd,
    /// Stroke start (document coordinates) — brush or eraser
    BrushStart {
        x: f32,
        y: f32,
        /// true = eraser (destination-out), false = brush
        erase: bool,
    },
    /// Stroke end — polyline (commit pixels) + frozen preview texture
    /// until pixels are actually applied.
    BrushEnd {
        points: Vec<(f32, f32)>,
        tex: Option<StrokeTex>,
        /// true = eraser (destination-out), false = brush
        erase: bool,
    },
    /// Clic pipette : l'app doit échantillonner la couleur compositée au
    /// point document donné et l'appliquer comme couleur globale.
    ColorPick {
        x: f32,
        y: f32,
    },
    /// Survol pipette (loupe active) — coordonnées document. Émis par
    /// quanta de `PICK_HOVER_STEP` px pour éviter la rafale de messages.
    PickHover {
        x: f32,
        y: f32,
    },
}

/// Brush/eraser style for live preview (document space).
#[derive(Clone, Copy, Debug)]
pub struct BrushStyle {
    /// RGB color 0-255 (ignored for eraser: ring preview)
    pub color: [u8; 3],
    /// Radius in DOCUMENT pixels (= size / 2)
    pub radius: f32,
    pub opacity: f32,
    /// true = eraser → preview is a RING (imprint) instead of disc
    pub erase: bool,
}

/// Stroke preview — 512×512 TILES in document coordinates.
///
/// Why a texture and not vector circles? The iced engine
/// enforces per-layer fixed render order quads -> meshes -> images:
/// vector geometry would go UNDER layer textures.
/// An image, however, is drawn after layer images.
///
/// Why tiles? The `iced_wgpu` texture atlas limits an image to
/// 2048×2048 (`atlas::MAX_SIZE`): a large stroke in a single texture
/// exceeded the limit and DISAPPEARED from preview. Each tile stays
/// well under the limit, whatever the stroke extent. Tiles
/// being aligned on an integer grid, no resampling occurs
/// when extending the stroke — preview no longer "crawls".
#[derive(Clone, Debug, Default)]
pub struct StrokeTex {
    tiles: Vec<Tile>,
}

/// Side of a preview tile (document pixels).
const TILE: u32 = 512;

#[derive(Clone, Debug)]
struct Tile {
    /// Tile coordinates on grid (× TILE = document origin)
    tx: i32,
    ty: i32,
    rgba: Vec<u8>,
}

impl StrokeTex {
    fn tile_mut(&mut self, tx: i32, ty: i32) -> &mut Tile {
        if let Some(pos) = self.tiles.iter().position(|t| t.tx == tx && t.ty == ty) {
            &mut self.tiles[pos]
        } else {
            self.tiles.push(Tile {
                tx,
                ty,
                rgba: vec![0; (TILE * TILE * 4) as usize],
            });
            self.tiles.last_mut().expect("vient d'être poussée")
        }
    }

    /// Stamp a disc (color) or ring (eraser) centered at (cx, cy)
    /// document — writes to all overlapped tiles.
    fn stamp_disc(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        ring: bool,
        col: [u8; 3],
        opacity: f32,
    ) {
        let pad = radius + 1.5;
        let tx0 = ((cx - pad).floor() as i32).div_euclid(TILE as i32);
        let tx1 = ((cx + pad).floor() as i32).div_euclid(TILE as i32);
        let ty0 = ((cy - pad).floor() as i32).div_euclid(TILE as i32);
        let ty1 = ((cy + pad).floor() as i32).div_euclid(TILE as i32);
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let tile = self.tile_mut(tx, ty);
                let lx = cx - tx as f32 * TILE as f32;
                let ly = cy - ty as f32 * TILE as f32;
                if ring {
                    let thickness = (radius * 0.16).max(1.5);
                    stamp_ring(tile, lx, ly, radius, thickness, col, opacity);
                } else {
                    stamp_circle(tile, lx, ly, radius, col, opacity);
                }
            }
        }
    }

    /// Stamp discs/rings along segment (step ~ radius/3).
    fn stamp_segment(&mut self, from: (f32, f32), to: (f32, f32), b: &BrushStyle) {
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let dist = (dx * dx + dy * dy).sqrt();
        let step = (b.radius * 0.35).max(0.5);
        let n = ((dist / step).ceil() as usize).max(1);
        for i in 0..=n {
            let k = i as f32 / n as f32;
            self.stamp_disc(
                from.0 + dx * k,
                from.1 + dy * k,
                b.radius,
                b.erase,
                b.color,
                b.opacity,
            );
        }
    }

    /// Itère les tuiles touchées : (origine document x, y, pixels RGBA).
    /// Clone destiné à l'upload GPU du chemin `layer_canvas` (aperçu de
    /// trait sans retour applicatif) — usage transitoire pendant le geste.
    pub(crate) fn tiles_cloned(&self) -> Vec<(f32, f32, Vec<u8>)> {
        self.tiles().map(|(x, y, px)| (x, y, px.to_vec())).collect()
    }

    fn tiles(&self) -> impl Iterator<Item = (f32, f32, &[u8])> {
        self.tiles.iter().map(|t| {
            (
                t.tx as f32 * TILE as f32,
                t.ty as f32 * TILE as f32,
                t.rgba.as_slice(),
            )
        })
    }
}

/// Géométrie écran du visualiseur de transformation.
/// Partagée avec `layer_canvas` (même boîte, dessinée en shader).
#[derive(Clone, Copy)]
pub struct BoxUi {
    /// tl, tr, br, bl (écran)
    pub corners: [Point; 4],
    pub center: Point,
    /// Poignée de rotation (au-dessus du bord haut)
    pub rot_pos: Point,
    /// Milieu côté droit (inclinaison X) et côté bas (inclinaison Y)
    pub right_mid: Point,
    pub bottom_mid: Point,
    /// Poignée d'ÉCHELLE : 0.12× au-delà du coin bas-droite, le long de la
    /// diagonale centre → coin (façon « resize » Photoshop/Affinity).
    pub scale_pos: Point,
}

impl BoxUi {
    /// Coins écran tl, tr, br, bl → boîte complète. `pub(crate)` : le chemin
    /// GPU calcule ses poignées dans le même repère.
    #[must_use]
    pub(crate) fn new(corners: [Point; 4]) -> Self {
        let center = Point::new(
            corners.iter().map(|c| c.x).sum::<f32>() / 4.0,
            corners.iter().map(|c| c.y).sum::<f32>() / 4.0,
        );
        let mid = |a: Point, b: Point| Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let top_mid = mid(corners[0], corners[1]);
        let mut dir = Vector::new(top_mid.x - center.x, top_mid.y - center.y);
        let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
        if len > 1e-6 {
            dir /= len;
        } else {
            dir = Vector::new(0.0, -1.0);
        }
        let rot_pos = Point::new(top_mid.x + dir.x * ROT_STEM, top_mid.y + dir.y * ROT_STEM);
        let br = corners[2];
        let mut sdir = Vector::new(br.x - center.x, br.y - center.y);
        let slen = (sdir.x * sdir.x + sdir.y * sdir.y).sqrt();
        if slen > 1e-6 {
            sdir /= slen;
        } else {
            sdir = Vector::new(0.707, 0.707);
        }
        let scale_pos = Point::new(
            br.x + sdir.x * slen * SCALE_OFFSET,
            br.y + sdir.y * slen * SCALE_OFFSET,
        );
        Self {
            corners,
            center,
            rot_pos,
            right_mid: mid(corners[1], corners[2]),
            bottom_mid: mid(corners[3], corners[2]),
            scale_pos,
        }
    }
}

/// Tige de rotation (écran). Partagée avec `layer_canvas`.
pub(crate) const ROT_STEM: f32 = 24.0;
/// Rayon de hit des poignées (écran). Partagé avec `layer_canvas`.
pub(crate) const HANDLE_HIT: f32 = 8.0;
/// Distance de la poignée d'échelle : 0.12× la demi-diagonale, au-delà du coin
pub(crate) const SCALE_OFFSET: f32 = 0.12;
/// Quantum de mouvement doc avant de publier un `PickHover` (évite la rafale).
/// Partagé avec `layer_canvas` (même cadence pipette sur les deux chemins).
pub(crate) const PICK_HOVER_STEP: f32 = 4.0;
/// Côté (px doc) du patch échantillonné par la loupe : 15 px à ×8 = 120 px
/// écran, chaque pixel document bien visible (visée précise).
pub const LOUPE_PATCH_SIDE: u32 = 15;

/// Point dans un quadrilatère convexe (test de signe des produits
/// vectoriels, tolérant aux deux orientations). Partagé avec `layer_canvas`
/// (pick des calques sur le chemin GPU).
pub(crate) fn point_in_quad(p: Point, q: [Point; 4]) -> bool {
    let cross = |a: Point, b: Point, c: Point| -> f32 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    };
    let mut all_ccw = true;
    let mut all_cw = true;
    for i in 0..4 {
        let s = cross(q[i], q[(i + 1) % 4], p);
        all_ccw &= s >= 0.0;
        all_cw &= s <= 0.0;
    }
    all_ccw || all_cw
}

/// Curseur correspondant à une poignée de transformation.
/// `pub(crate)` : survol des poignées sur le chemin GPU.
pub(crate) fn transform_cursor(kind: TransformHandle) -> mouse::Interaction {
    match kind {
        TransformHandle::Move | TransformHandle::Rotate => mouse::Interaction::Move,
        TransformHandle::Corner(Corner::TopLeft) | TransformHandle::Corner(Corner::BottomRight) => {
            mouse::Interaction::ResizingDiagonallyUp
        }
        TransformHandle::Corner(Corner::TopRight) | TransformHandle::Corner(Corner::BottomLeft) => {
            mouse::Interaction::ResizingDiagonallyDown
        }
        TransformHandle::Scale => mouse::Interaction::ResizingDiagonallyDown,
        TransformHandle::SkewX => mouse::Interaction::ResizingHorizontally,
        TransformHandle::SkewY => mouse::Interaction::ResizingVertically,
    }
}

// ---------------------------------------------------------------------------
// Stroke preview rasterization (per 512×512 tile)
// ---------------------------------------------------------------------------

/// Rastérise le segment `from -> to` (coordonnées document) dans l'aperçu.
/// Les tuiles manquantes sont créées à la volée ; les existantes ne sont
/// jamais déplacées (grille entière) → aperçu sans dérive.
/// `pub(crate)` : réutilisé tel quel par le chemin GPU `layer_canvas`.
pub(crate) fn rasterize_segment(
    tex: &mut Option<StrokeTex>,
    from: (f32, f32),
    to: (f32, f32),
    brush: &BrushStyle,
) {
    let t = tex.get_or_insert_with(StrokeTex::default);
    t.stamp_segment(from, to, brush);
}

/// Disc with 1px soft edge; final alpha = coverage x opacity
fn stamp_circle(t: &mut Tile, cx: f32, cy: f32, r: f32, col: [u8; 3], opacity: f32) {
    let w = i64::from(TILE);
    let h = i64::from(TILE);
    let x0 = ((cx - r - 1.0).floor() as i64).clamp(0, w.saturating_sub(1));
    let y0 = ((cy - r - 1.0).floor() as i64).clamp(0, h.saturating_sub(1));
    let x1 = ((cx + r + 1.0).ceil() as i64).clamp(0, w - 1);
    let y1 = ((cy + r + 1.0).ceil() as i64).clamp(0, h - 1);
    if w == 0 || h == 0 {
        return;
    }
    for py in y0..=y1 {
        for px in x0..=x1 {
            let ddx = px as f32 + 0.5 - cx;
            let ddy = py as f32 + 0.5 - cy;
            let d = (ddx * ddx + ddy * ddy).sqrt();
            let cov = if d <= r {
                255.0
            } else if d < r + 1.0 {
                (r + 1.0 - d) * 255.0
            } else {
                continue;
            };
            let a = ((cov * opacity.clamp(0.0, 1.0)).round() as u32).min(255) as u8;
            if a == 0 {
                continue;
            }
            let idx = ((py * w + px) * 4) as usize;
            // Transparent background: source-over == MAX; avoids darkening
            // aux recouvrements de disques successifs.
            if a > t.rgba[idx + 3] {
                t.rgba[idx] = col[0];
                t.rgba[idx + 1] = col[1];
                t.rgba[idx + 2] = col[2];
                t.rgba[idx + 3] = a;
            }
        }
    }
}

/// Anneau blanc semi-transparent : empreinte visuelle de la eraser.
/// Interior stays TRANSPARENT — show WHERE erasure will happen,
/// not a paint color. White to stay readable on any background.
fn stamp_ring(t: &mut Tile, cx: f32, cy: f32, r: f32, thickness: f32, col: [u8; 3], opacity: f32) {
    const RING_ALPHA: f32 = 0.85;
    let inner = (r - thickness).max(0.0);
    let outer = r + 1.0; // bord adouci externe 1 px
    let w = i64::from(TILE);
    let h = i64::from(TILE);
    let x0 = ((cx - outer).floor() as i64).clamp(0, w.saturating_sub(1));
    let y0 = ((cy - outer).floor() as i64).clamp(0, h.saturating_sub(1));
    let x1 = ((cx + outer).ceil() as i64).clamp(0, w - 1);
    let y1 = ((cy + outer).ceil() as i64).clamp(0, h - 1);
    if w == 0 || h == 0 {
        return;
    }
    for py in y0..=y1 {
        for px in x0..=x1 {
            let ddx = px as f32 + 0.5 - cx;
            let ddy = py as f32 + 0.5 - cy;
            let d = (ddx * ddx + ddy * ddy).sqrt();
            // Band [inner, r] solid, softened by 1px on each side;
            // INTERIOR (d < inner-1) stays transparent — ring shows
            // eraser footprint, not paint.
            let cov = if d >= inner && d <= r {
                255.0
            } else if d < inner {
                if d >= inner - 1.0 {
                    (d - (inner - 1.0)) * 255.0 // fondu interne court
                } else {
                    continue;
                }
            } else if d < outer {
                (outer - d) * 255.0
            } else {
                continue;
            };
            let a = ((cov * opacity.clamp(0.0, 1.0) * RING_ALPHA).round() as u32).min(255) as u8;
            if a == 0 {
                continue;
            }
            let idx = ((py * w + px) * 4) as usize;
            if a > t.rgba[idx + 3] {
                t.rgba[idx] = col[0];
                t.rgba[idx + 1] = col[1];
                t.rgba[idx + 2] = col[2];
                t.rgba[idx + 3] = a;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(tile: &Tile, local_x: u32, local_y: u32) -> u8 {
        tile.rgba[((local_y * TILE + local_x) * 4 + 3) as usize]
    }

    #[test]
    fn disque_sur_frontiere_de_tuiles_ecrit_dans_les_deux() {
        let mut tex = StrokeTex::default();
        // Disc centered on border x = 512
        tex.stamp_disc(512.0, 100.0, 20.0, false, [255, 0, 0], 1.0);
        assert_eq!(tex.tiles.len(), 2, "tuiles (0,0) et (1,0) touchées");

        let right = &tex.tiles.iter().find(|t| t.tx == 1).expect("tuile droite");
        // Disc center: local (0, 100) in right tile
        assert_eq!(alpha_at(right, 0, 100), 255);
        let left = &tex.tiles.iter().find(|t| t.tx == 0).expect("tuile gauche");
        // Left edge of disc: local (511, 100) in left tile
        assert!(alpha_at(left, 511, 100) > 0);
    }

    #[test]
    fn trait_transfrontalier_sans_limite_de_taille() {
        // Stroke from (0,0) to (3000,3000): far exceeds old
        // limite atlas de 2048 — chaque tuile reste ≤ 512×512
        let mut tex = StrokeTex::default();
        tex.stamp_segment(
            (0.0, 0.0),
            (3000.0, 3000.0),
            &BrushStyle {
                color: [10, 20, 30],
                radius: 8.0,
                opacity: 1.0,
                erase: false,
            },
        );
        // Diagonal crosses tiles (0,0)…(5,5) + touched neighbors
        // par le rayon du disque
        assert!(
            tex.tiles.len() >= 12,
            "au moins la bande diagonale + voisines"
        );
        for d in 0..=5 {
            assert!(
                tex.tiles.iter().any(|t| t.tx == d && t.ty == d),
                "tuile diagonale ({d},{d}) manquante"
            );
        }
        for t in &tex.tiles {
            assert_eq!(t.rgba.len() as u32, TILE * TILE * 4);
        }
        // Start and end points well stamped
        let first = &tex
            .tiles
            .iter()
            .find(|t| t.tx == 0 && t.ty == 0)
            .expect("tile (0,0) should exist");
        assert!(alpha_at(first, 0, 0) > 0);
        let last = &tex
            .tiles
            .iter()
            .find(|t| t.tx == 5 && t.ty == 5)
            .expect("tile (5,5) should exist");
        // 3000 - 5*512 = 440 : le centre du disque final est en (440,440) local
        assert!(alpha_at(last, 440, 440) > 0);
    }

    #[test]
    fn extension_du_trait_ne_deplace_pas_les_tuiles_existantes() {
        // Drift bug regression: stamp near, then segment far —
        // first stamp pixels stay EXACTLY in place.
        let mut tex = StrokeTex::default();
        tex.stamp_segment(
            (100.0, 100.0),
            (110.0, 100.0),
            &BrushStyle {
                color: [1, 2, 3],
                radius: 6.0,
                opacity: 1.0,
                erase: false,
            },
        );
        let before = tex.tiles.clone();

        // Segment far from first stamp: no possible retouch
        // de la tuile (0,0) — elle doit rester byte-identique.
        tex.stamp_segment(
            (1200.0, 1200.0),
            (1500.0, 900.0),
            &BrushStyle {
                color: [1, 2, 3],
                radius: 6.0,
                opacity: 1.0,
                erase: false,
            },
        );

        for old in &before {
            let now = tex
                .tiles
                .iter()
                .find(|t| t.tx == old.tx && t.ty == old.ty)
                .expect("tuile existante conservée");
            assert_eq!(old.rgba, now.rgba, "tuile ({},{}) modifiée", old.tx, old.ty);
        }
    }

    #[test]
    fn gomme_produit_un_anneau_interieur_transparent() {
        let mut tex = StrokeTex::default();
        tex.stamp_disc(100.0, 100.0, 20.0, true, [255, 255, 255], 1.0);
        let t = &tex.tiles[0];
        // Center: interior of ring → transparent
        assert_eq!(alpha_at(t, 100, 100), 0);
        // Ring band (d ≤ r): opaque at 85% (RING_ALPHA)
        assert_eq!(alpha_at(t, 119, 100), 217);
    }
}
