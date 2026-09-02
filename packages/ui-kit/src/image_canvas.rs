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

//! Interactive image canvas with pan/zoom — uses `iced::widget::canvas` native
//! Inspired by `examples/bezier_tool` and `game_of_life` (infinite pan/zoom)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

use iced::mouse;
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke};
use iced::widget::image;
use iced::{Point, Rectangle, Size, Theme, Vector};
use uuid::Uuid;

use crate::theme::colors;

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
    /// Curseur pendant un geste (coordonnées document). `uniform` = Ctrl
    /// enfoncé → redimensionnement PROPORTIONNEL (aspect conservé).
    TransformCursor {
        doc: (f32, f32),
        uniform: bool,
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

    /// Iterate touched tiles: (document origin x, y, RGBA pixels).
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

/// A displayable layer on canvas — drawn at its world position.
/// Moving = change offset → simple redraw, zero recomposite.
/// Opacity/rotation/scale are applied AT DRAW (GPU) — zero
/// pixel regeneration.
#[derive(Clone)]
pub struct CanvasLayer {
    /// Identifiant du calque applicatif (None = composite fallback)
    pub id: Option<Uuid>,
    pub handle: image::Handle,
    pub width: f32,
    pub height: f32,
    /// World position (0,0) = top-left corner of document
    pub offset_x: f32,
    pub offset_y: f32,
    /// Opacity 0..1 applied at draw (without touching pixels)
    pub opacity: f32,
    /// Rotation in degrees (around layer center)
    pub rotation_deg: f32,
    /// Non-uniform scale (1.0 = 100%)
    pub scale_x: f32,
    pub scale_y: f32,
    /// Inclinaison horizontale en degrés (cisaille X selon Y)
    pub skew_x: f32,
    /// Inclinaison verticale en degrés (cisaille Y selon X)
    pub skew_y: f32,
}

impl CanvasLayer {
    /// Les 4 coins de la carte locale → doc (même convention que
    /// `Transform2D::local_to_doc` du moteur : offset = coin supérieur-gauche
    /// du rectangle scalé, cisaillement/rotation autour du centre scalé).
    ///
    /// ui-kit ne peut pas dépendre de `photo-engine` (couches : packages
    /// jamais dépendantes des engines) : CETTE implémentation est donc une
    /// copie de la convention affine canonique. Toute évolution du modèle
    /// transform doit rester en sync ici et dans `prepare_top_affine`
    /// (compositing moteur).
    #[must_use]
    pub fn corners(&self) -> [(f32, f32); 4] {
        let sx = self.scale_x;
        let sy = self.scale_y;
        let kx = self.skew_x.to_radians().tan();
        let ky = self.skew_y.to_radians().tan();
        let r = self.rotation_deg.to_radians();
        let (cos, sin) = (r.cos(), r.sin());
        let cx = self.width / 2.0;
        let cy = self.height / 2.0;
        let transform = |x: f32, y: f32| -> (f32, f32) {
            let ux = (x - cx) * sx;
            let uy = (y - cy) * sy;
            let tx = ux + kx * uy;
            let ty = ky * ux + uy;
            (
                tx * cos - ty * sin + cx * sx + self.offset_x,
                tx * sin + ty * cos + cy * sy + self.offset_y,
            )
        };
        [
            transform(0.0, 0.0),
            transform(self.width, 0.0),
            transform(self.width, self.height),
            transform(0.0, self.height),
        ]
    }

    /// Centre du parallélogramme affiché, en coordonnées doc (moyenne des 4
    /// coins → exact même avec rotation/cisaillement).
    #[must_use]
    pub fn center(&self) -> Point {
        let c = self.corners();
        Point::new(
            (c[0].0 + c[1].0 + c[2].0 + c[3].0) / 4.0,
            (c[0].1 + c[1].1 + c[2].1 + c[3].1) / 4.0,
        )
    }
}

/// Interactive image canvas program.
pub struct ImageCanvas {
    pub layers: Vec<CanvasLayer>,
    /// Calques pour le pick (clic → sélection) — en mode fallback la liste
    /// affichée n'est qu'un seul composite sans identité ; celle-ci garde la
    /// hiérarchie réelle pour le hit-testing.
    pub hit_layers: Vec<CanvasLayer>,
    /// Calque sélectionné : dessine le visualiseur de transformation et
    /// reçoit les gestes (move/resize/rotate/skew). Optionnel = aucun overlay.
    pub transform_target: Option<CanvasLayer>,
    /// Document dimensions (ground reference, drawn in world space)
    pub doc_size: Option<Size>,
    pub pan: Vector,
    pub zoom: f32,
    pub tool: CanvasTool,
    pub selection: Option<Rectangle>,
    /// Brush style (live preview rasterization)
    pub brush: BrushStyle,
    /// Brush allowed (false = hidden layer → no preview nor interaction)
    pub can_paint: bool,
    /// Frozen preview during async commit (after release)
    pub pending_preview: Option<StrokeTex>,
}

impl ImageCanvas {
    #[must_use]
    pub fn new(doc_size: Option<Size>, pan: Vector, zoom: f32) -> Self {
        Self {
            layers: Vec::new(),
            hit_layers: Vec::new(),
            transform_target: None,
            doc_size,
            pan,
            zoom: zoom.clamp(0.08, 6.0),
            tool: CanvasTool::Hand,
            selection: None,
            brush: BrushStyle {
                color: [30, 30, 34],
                radius: 6.0,
                opacity: 1.0,
                erase: false,
            },
            can_paint: true,
            pending_preview: None,
        }
    }
    #[must_use]
    pub fn with_brush(mut self, brush: BrushStyle) -> Self {
        self.brush = brush;
        self
    }
    #[must_use]
    pub fn with_can_paint(mut self, can: bool) -> Self {
        self.can_paint = can;
        self
    }
    #[must_use]
    pub fn with_pending_preview(mut self, preview: Option<StrokeTex>) -> Self {
        self.pending_preview = preview;
        self
    }

    /// Convert canvas screen position to DOCUMENT coordinates
    /// (inverse exact du transform de draw : centre + pan + zoom).
    fn screen_to_doc(&self, p: Point, bounds: Rectangle) -> Point {
        let center = Point::new(
            bounds.width / 2.0 + self.pan.x,
            bounds.height / 2.0 + self.pan.y,
        );
        let (hw, hh) = self
            .doc_size
            .map_or((0.0, 0.0), |s| (s.width / 2.0, s.height / 2.0));
        Point::new(
            (p.x - center.x) / self.zoom + hw,
            (p.y - center.y) / self.zoom + hh,
        )
    }
    #[must_use]
    pub fn with_layers(mut self, layers: Vec<CanvasLayer>) -> Self {
        self.layers = layers;
        self
    }
    #[must_use]
    pub fn with_hit_layers(mut self, hit_layers: Vec<CanvasLayer>) -> Self {
        self.hit_layers = hit_layers;
        self
    }
    #[must_use]
    pub fn with_transform_target(mut self, target: Option<CanvasLayer>) -> Self {
        self.transform_target = target;
        self
    }
    #[must_use]
    pub fn with_tool(mut self, tool: CanvasTool) -> Self {
        self.tool = tool;
        self
    }
    #[must_use]
    pub fn with_selection(mut self, sel: Option<Rectangle>) -> Self {
        self.selection = sel;
        self
    }

    /// Point écran → coordonnées document (centre doc = (w/2, h/2)).
    fn doc_to_screen(
        &self,
        doc: (f32, f32),
        center: Point,
        doc_half_w: f32,
        doc_half_h: f32,
    ) -> Point {
        Point::new(
            center.x + (doc.0 - doc_half_w) * self.zoom,
            center.y + (doc.1 - doc_half_h) * self.zoom,
        )
    }

    /// Centre/pan/zoom → centre écran du document.
    fn center_of(&self, bounds: Rectangle) -> Point {
        Point::new(
            bounds.width / 2.0 + self.pan.x,
            bounds.height / 2.0 + self.pan.y,
        )
    }

    /// Les 4 coins (écran) du parallélogramme d'un calque donné.
    fn screen_corners(&self, l: &CanvasLayer, bounds: Rectangle) -> [Point; 4] {
        let center = self.center_of(bounds);
        let (dhw, dhh) = self
            .doc_size
            .map_or((0.0, 0.0), |s| (s.width / 2.0, s.height / 2.0));
        l.corners()
            .map(|(dx, dy)| self.doc_to_screen((dx, dy), center, dhw, dhh))
    }

    /// Calque le plus au-dessus contenant le point écran (dans l'ordre de
    /// dessin : dernier = dessus).
    fn pick_layer(&self, p: Point, bounds: Rectangle) -> Option<Uuid> {
        self.hit_layers
            .iter()
            .rev()
            .find(|l| {
                let quad = self.screen_corners(l, bounds);
                point_in_quad(p, quad)
            })
            .and_then(|l| l.id)
    }

    /// Poignée transformée sous le curseur (ou `Move` si l'intérieur de la
    /// boîte est touché). `None` = hors visualiseur.
    fn hit_transform_handle(
        &self,
        p: Point,
        bounds: Rectangle,
    ) -> Option<(TransformHandle, BoxUi)> {
        let target = self.transform_target.as_ref()?;
        let corners = self.screen_corners(target, bounds);
        let ui = BoxUi::new(corners);
        // Rotation d'abord (grande zone), puis coins, puis inclinaisons,
        // puis intérieur
        if ui.rot_pos.distance(p) <= HANDLE_HIT {
            return Some((TransformHandle::Rotate, ui));
        }
        let corner_kinds = [
            (Corner::TopLeft, ui.corners[0]),
            (Corner::TopRight, ui.corners[1]),
            (Corner::BottomRight, ui.corners[2]),
            (Corner::BottomLeft, ui.corners[3]),
        ];
        for (kind, c) in corner_kinds {
            if c.distance(p) <= HANDLE_HIT {
                return Some((TransformHandle::Corner(kind), ui));
            }
        }
        if ui.right_mid.distance(p) <= HANDLE_HIT {
            return Some((TransformHandle::SkewX, ui));
        }
        if ui.bottom_mid.distance(p) <= HANDLE_HIT {
            return Some((TransformHandle::SkewY, ui));
        }
        if point_in_quad(p, corners) {
            return Some((TransformHandle::Move, ui));
        }
        None
    }

    /// Curseur dans l'INTERIEUR de la boîte du calque sélectionné (zone de
    /// déplacement — jamais prioritaire sur la sélection d'un autre calque).
    fn in_transform_box(&self, p: Point, bounds: Rectangle) -> bool {
        let Some(target) = self.transform_target.as_ref() else {
            return false;
        };
        point_in_quad(p, self.screen_corners(target, bounds))
    }

    /// Overlay « poignées de transformation » du calque sélectionné.
    /// Marquee de sélection + visualiseur de transformation (Affinity).
    /// Dans une 2e géométrie → rendue APRÈS les images de la 1re géométrie :
    /// visible au premier plan, même par-dessus le composite fallback.
    fn draw_overlay(
        &self,
        renderer: &iced::Renderer,
        bounds: Rectangle,
        state: &State,
    ) -> Option<Geometry> {
        let active = state.transform_handle;
        let show_box = self.transform_target.is_some()
            && matches!(self.tool, CanvasTool::Select | CanvasTool::Move);
        if active.is_none() && !show_box && !(state.selecting.is_some() || self.selection.is_some())
        {
            return None;
        }
        let mut frame = Frame::new(renderer, bounds.size());

        // Marquee de sélection (outils Sélection/Zoom) et rectangle sélectionné
        if let Some((start, current)) = state.selecting {
            let sel = Rectangle::new(start, Size::new(current.x - start.x, current.y - start.y));
            let norm = Rectangle::new(
                Point::new(sel.x.min(sel.x + sel.width), sel.y.min(sel.y + sel.height)),
                Size::new(sel.width.abs(), sel.height.abs()),
            );
            frame.fill_rectangle(norm.position(), norm.size(), colors::SELECTION_FILL);
            let path = Path::rectangle(norm.position(), norm.size());
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(1.0)
                    .with_color(colors::SELECTION_STROKE),
            );
        } else if let Some(sel) = self.selection {
            frame.fill_rectangle(sel.position(), sel.size(), colors::SELECTION_FILL);
            let path = Path::rectangle(sel.position(), sel.size());
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(1.0)
                    .with_color(colors::SELECTION_STROKE),
            );
        }

        if show_box {
            let target = self.transform_target.as_ref()?;
            let corners = self.screen_corners(target, bounds);
            let ui = BoxUi::new(corners);

            let quad = Path::new(|p| {
                for (i, c) in corners.iter().enumerate() {
                    if i == 0 {
                        p.move_to(*c);
                    } else {
                        p.line_to(*c);
                    }
                }
                p.close();
            });
            frame.stroke(
                &quad,
                Stroke::default()
                    .with_width(1.0)
                    .with_color(colors::SELECTION_STROKE),
            );

            // Poignée de rotation : tige + cercle au-dessus du bord haut
            let top_mid = Point::new(
                (corners[0].x + corners[1].x) / 2.0,
                (corners[0].y + corners[1].y) / 2.0,
            );
            let stem = Path::new(|p| {
                p.move_to(top_mid);
                p.line_to(ui.rot_pos);
            });
            frame.stroke(
                &stem,
                Stroke::default()
                    .with_width(1.0)
                    .with_color(colors::SELECTION_STROKE),
            );
            let rot_fill = if active == Some(TransformHandle::Rotate) {
                Fill::from(colors::ACCENT)
            } else {
                Fill::from(colors::TEXT_ON_ACCENT)
            };
            let rot_circle = Path::circle(ui.rot_pos, 5.0);
            frame.fill(&rot_circle, rot_fill);
            frame.stroke(
                &rot_circle,
                Stroke::default()
                    .with_width(1.2)
                    .with_color(colors::SELECTION_STROKE),
            );
            frame.fill(
                &Path::circle(top_mid, 1.8),
                Fill::from(colors::SELECTION_STROKE),
            );

            // Poignées d'angle (redimensionner) : CERCLES
            let corner_kinds = [
                (Corner::TopLeft, ui.corners[0]),
                (Corner::TopRight, ui.corners[1]),
                (Corner::BottomRight, ui.corners[2]),
                (Corner::BottomLeft, ui.corners[3]),
            ];
            for (kind, c) in corner_kinds {
                let fill = if active == Some(TransformHandle::Corner(kind)) {
                    Fill::from(colors::ACCENT)
                } else {
                    Fill::from(colors::TEXT_ON_ACCENT)
                };
                let circle = Path::circle(c, HANDLE_HALF);
                frame.fill(&circle, fill);
                frame.stroke(
                    &circle,
                    Stroke::default()
                        .with_width(1.0)
                        .with_color(colors::SELECTION_STROKE),
                );
            }

            // Poignées d'inclinaison (losanges, milieux des côtés droit et bas)
            for (kind, m) in [
                (TransformHandle::SkewX, ui.right_mid),
                (TransformHandle::SkewY, ui.bottom_mid),
            ] {
                let di = HANDLE_HALF;
                let diamond = Path::new(|p| {
                    p.move_to(Point::new(m.x, m.y - di));
                    p.line_to(Point::new(m.x + di, m.y));
                    p.line_to(Point::new(m.x, m.y + di));
                    p.line_to(Point::new(m.x - di, m.y));
                    p.close();
                });
                let fill = if active == Some(kind) {
                    colors::ACCENT
                } else {
                    colors::TEXT_ON_ACCENT
                };
                frame.fill(&diamond, Fill::from(fill));
                frame.stroke(
                    &diamond,
                    Stroke::default()
                        .with_width(1.0)
                        .with_color(colors::SELECTION_STROKE),
                );
            }
        }

        Some(frame.into_geometry())
    }
}

/// Géométrie écran du visualiseur de transformation.
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
}

impl BoxUi {
    #[must_use]
    fn new(corners: [Point; 4]) -> Self {
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
        Self {
            corners,
            center,
            rot_pos,
            right_mid: mid(corners[1], corners[2]),
            bottom_mid: mid(corners[3], corners[2]),
        }
    }
}

/// Rayon de hit des poignées (écran)
const HANDLE_HIT: f32 = 8.0;
/// Longueur de la tige de rotation
const ROT_STEM: f32 = 24.0;
/// Demi-côté des poignées dessinées (écran)
const HANDLE_HALF: f32 = 5.0;

/// Point dans un quadrilatère convexe (test de signe des produits
/// vectoriels, tolérant aux deux orientations).
fn point_in_quad(p: Point, q: [Point; 4]) -> bool {
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

#[derive(Default)]
pub struct State {
    pub dragging: Option<(Point, Vector)>,
    /// Geste de transformation actif (poignée saisie)
    pub transform_handle: Option<TransformHandle>,
    /// Current stroke points (document coords) — stored in canvas
    /// for preview without round-trip to app (zero latency,
    /// explicit redraw on each move).
    pub stroke: Vec<(f32, f32)>,
    /// Rasterized stroke version (doc-space texture, above layers)
    pub stroke_tex: Option<StrokeTex>,
    pub selecting: Option<(Point, Point)>, // start, current
    /// Scrubby zoom drag — screen anchor + initial zoom/pan
    pub zoom_dragging: Option<(Point, f32, Vector)>,
    /// Screen cursor position for tool size preview
    pub cursor_pos: Option<Point>,
    /// Common keyboard modifiers (Alt = inverted zoom with magnifier tool)
    pub modifiers: iced::keyboard::Modifiers,
    /// Space held → temporary pan (Photoshop)
    pub space_held: bool,
    /// Last published viewport size (avoids event spam)
    pub prev_bounds: Option<Size>,
}

/// Curseur correspondant à une poignée de transformation.
fn transform_cursor(kind: TransformHandle) -> mouse::Interaction {
    match kind {
        TransformHandle::Move | TransformHandle::Rotate => mouse::Interaction::Move,
        TransformHandle::Corner(Corner::TopLeft) | TransformHandle::Corner(Corner::BottomRight) => {
            mouse::Interaction::ResizingDiagonallyUp
        }
        TransformHandle::Corner(Corner::TopRight) | TransformHandle::Corner(Corner::BottomLeft) => {
            mouse::Interaction::ResizingDiagonallyDown
        }
        TransformHandle::SkewX => mouse::Interaction::ResizingHorizontally,
        TransformHandle::SkewY => mouse::Interaction::ResizingVertically,
    }
}

impl canvas::Program<ImageCanvasEvent> for ImageCanvas {
    type State = State;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<ImageCanvasEvent>> {
        // Redraw requested each frame: use it to publish viewport
        if let canvas::Event::Window(iced::window::Event::RedrawRequested(_)) = event {
            if state.prev_bounds != Some(bounds.size()) {
                state.prev_bounds = Some(bounds.size());
                return Some(canvas::Action::publish(ImageCanvasEvent::Viewport(
                    bounds.size(),
                )));
            }
            return None;
        }

        // Track keyboard modifiers (before early-return on cursor)
        if let canvas::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) = event {
            state.modifiers = *m;
            return None;
        }
        if let canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event
            && *key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Space)
        {
            state.space_held = true;
            return None;
        }
        if let canvas::Event::Keyboard(iced::keyboard::Event::KeyReleased { key, .. }) = event
            && *key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Space)
        {
            state.space_held = false;
            // Cancel ongoing drag if space is released
            if state.dragging.is_some() {
                state.dragging = None;
                return Some(canvas::Action::capture());
            }
            return None;
        }

        // Release: MUST be handled even outside widget bounds
        // (mouse is captured during drag) — otherwise drag state
        // stays armed and move events keep arriving.
        if let canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
            if state.transform_handle.take().is_some() {
                return Some(canvas::Action::publish(ImageCanvasEvent::TransformEnd).and_capture());
            }
            if let Some((anchor, start_zoom, start_pan)) = state.zoom_dragging.take() {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return Some(canvas::Action::capture());
                };
                let dx = cursor_pos.x - anchor.x;
                let dy = cursor_pos.y - anchor.y;
                let drag_dist = (dx * dx + dy * dy).sqrt();
                // Click without significant move = point zoom on anchor
                if drag_dist < 4.0 {
                    let base_factor = 1.4_f32;
                    let factor = if state.modifiers.alt() {
                        1.0 / base_factor
                    } else {
                        base_factor
                    };
                    let new_zoom = (start_zoom * factor).clamp(0.08, 6.0);
                    let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
                    let factor_ratio = new_zoom / start_zoom;
                    let new_pan = Vector::new(
                        anchor.x - center.x - (anchor.x - center.x - start_pan.x) * factor_ratio,
                        anchor.y - center.y - (anchor.y - center.y - start_pan.y) * factor_ratio,
                    );
                    return Some(canvas::Action::publish(ImageCanvasEvent::ZoomAt {
                        zoom: new_zoom,
                        pan: new_pan,
                    }));
                }
                return Some(canvas::Action::capture());
            }
            if let Some((start, end)) = state.selecting.take() {
                let Some(_cursor_pos) = cursor.position_in(bounds) else {
                    // Released outside canvas: cancel selection
                    return Some(canvas::Action::publish(ImageCanvasEvent::SelectRect(None)));
                };
                let rect = Rectangle::new(start, Size::new(end.x - start.x, end.y - start.y));
                // Normalise
                let norm = Rectangle::new(
                    Point::new(
                        rect.x.min(rect.x + rect.width),
                        rect.y.min(rect.y + rect.height),
                    ),
                    Size::new(rect.width.abs(), rect.height.abs()),
                );
                if norm.width > 5.0 && norm.height > 5.0 {
                    return Some(canvas::Action::publish(ImageCanvasEvent::SelectRect(Some(
                        norm,
                    ))));
                }
                return Some(canvas::Action::publish(ImageCanvasEvent::SelectRect(None)));
            }
            if let Some((_start, _orig_pan)) = state.dragging.take() {
                if self.tool == CanvasTool::Brush || self.tool == CanvasTool::Eraser {
                    let points = std::mem::take(&mut state.stroke);
                    let tex = state.stroke_tex.take();
                    let erase = self.tool == CanvasTool::Eraser;
                    return Some(
                        canvas::Action::publish(ImageCanvasEvent::BrushEnd { points, tex, erase })
                            .and_capture(),
                    );
                }
                return Some(canvas::Action::capture());
            }
            return None;
        }

        let cursor_pos = cursor.position_in(bounds)?;

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // Espace maintenu → pan temporaire quel que soit l'outil
                if state.space_held {
                    state.dragging = Some((cursor_pos, self.pan));
                    return Some(canvas::Action::capture());
                }
                // Visualiseur de transformation. Ordre de priorité :
                // 1) POIGNÉES (rotation/coins/inclinaisons) du calque
                //    sélectionné — geste dédié.
                // 2) Clic sur UN CALQUE (autre ou même) → sélection + geste
                //    Déplacement (jamais bloqué par une grosse sélection).
                // 3) Intérieur vide de la boîte → déplacer la sélection.
                // 4) Zone vide → marquee (Sélection) / désélection (Déplacer).
                if matches!(self.tool, CanvasTool::Select | CanvasTool::Move)
                    && let Some((kind, _ui)) = self.hit_transform_handle(cursor_pos, bounds)
                    && !matches!(kind, TransformHandle::Move)
                {
                    state.transform_handle = Some(kind);
                    let doc = self.screen_to_doc(cursor_pos, bounds);
                    return Some(
                        canvas::Action::publish(ImageCanvasEvent::TransformStart {
                            id: None,
                            kind,
                            doc: (doc.x, doc.y),
                        })
                        .and_capture(),
                    );
                }
                // Clic sur un calque : on le sélectionne puis on le déplace.
                if matches!(self.tool, CanvasTool::Select | CanvasTool::Move)
                    && let Some(id) = self.pick_layer(cursor_pos, bounds)
                {
                    state.transform_handle = Some(TransformHandle::Move);
                    let doc = self.screen_to_doc(cursor_pos, bounds);
                    // Même calque que la sélection courante → id: None (pas
                    // de re-sélection), le geste reste un Déplacement.
                    let same = self
                        .transform_target
                        .as_ref()
                        .is_some_and(|t| t.id == Some(id));
                    let id = if same { None } else { Some(id) };
                    return Some(
                        canvas::Action::publish(ImageCanvasEvent::TransformStart {
                            id,
                            kind: TransformHandle::Move,
                            doc: (doc.x, doc.y),
                        })
                        .and_capture(),
                    );
                }
                // Intérieur de la boîte de l'élément sélectionné (aucun calque
                // sous le curseur) → déplacement de la sélection.
                if matches!(self.tool, CanvasTool::Select | CanvasTool::Move)
                    && self.in_transform_box(cursor_pos, bounds)
                {
                    state.transform_handle = Some(TransformHandle::Move);
                    let doc = self.screen_to_doc(cursor_pos, bounds);
                    return Some(
                        canvas::Action::publish(ImageCanvasEvent::TransformStart {
                            id: None,
                            kind: TransformHandle::Move,
                            doc: (doc.x, doc.y),
                        })
                        .and_capture(),
                    );
                }
                // Tools
                match self.tool {
                    CanvasTool::Hand => {
                        state.dragging = Some((cursor_pos, self.pan));
                        Some(canvas::Action::capture())
                    }
                    CanvasTool::Move => {
                        // Zone vide → désélection
                        Some(
                            canvas::Action::publish(ImageCanvasEvent::ClearSelection).and_capture(),
                        )
                    }
                    CanvasTool::Select => {
                        // Zone vide → marquee
                        state.selecting = Some((cursor_pos, cursor_pos));
                        Some(canvas::Action::capture())
                    }
                    CanvasTool::Brush | CanvasTool::Eraser => {
                        if !self.can_paint {
                            return Some(canvas::Action::capture());
                        }
                        let doc = self.screen_to_doc(cursor_pos, bounds);
                        state.stroke = vec![(doc.x, doc.y)];
                        state.stroke_tex = None;
                        state.dragging = Some((cursor_pos, self.pan));
                        let erase = self.tool == CanvasTool::Eraser;
                        Some(
                            canvas::Action::publish(ImageCanvasEvent::BrushStart {
                                x: doc.x,
                                y: doc.y,
                                erase,
                            })
                            .and_capture(),
                        )
                    }
                    CanvasTool::Zoom => {
                        // Start scrubby zoom — anchor = click point
                        state.zoom_dragging = Some((cursor_pos, self.zoom, self.pan));
                        Some(canvas::Action::capture())
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // Update tool size preview and scrubby zoom
                state.cursor_pos = Some(cursor_pos);
                if state.transform_handle.is_some() {
                    let doc = self.screen_to_doc(cursor_pos, bounds);
                    return Some(
                        canvas::Action::publish(ImageCanvasEvent::TransformCursor {
                            doc: (doc.x, doc.y),
                            // Ctrl maintenu pendant le geste → échelle uniforme
                            uniform: state.modifiers.control(),
                        })
                        .and_capture(),
                    );
                }
                if let Some((anchor, start_zoom, start_pan)) = state.zoom_dragging {
                    // Scrubby zoom: vertical = forward/back, anchored on click point
                    let dy = anchor.y - cursor_pos.y; // monter = zoom +
                    let dx = cursor_pos.x - anchor.x;
                    let delta = dy + dx * 0.5; // influence horizontale douce
                    let factor = (1.008_f32).powf(delta);
                    let new_zoom = (start_zoom * factor).clamp(0.08, 6.0);
                    let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
                    let ratio = new_zoom / start_zoom;
                    let new_pan = Vector::new(
                        anchor.x - center.x - (anchor.x - center.x - start_pan.x) * ratio,
                        anchor.y - center.y - (anchor.y - center.y - start_pan.y) * ratio,
                    );
                    return Some(canvas::Action::publish(ImageCanvasEvent::ZoomAt {
                        zoom: new_zoom,
                        pan: new_pan,
                    }));
                }
                if let Some((start, _)) = state.selecting {
                    state.selecting = Some((start, cursor_pos));
                    // preview via request_redraw
                    return Some(canvas::Action::request_redraw().and_capture());
                }
                if let Some((start, orig_pan)) = state.dragging {
                    if state.space_held {
                        let delta = Vector::new(cursor_pos.x - start.x, cursor_pos.y - start.y);
                        let new_pan = Vector::new(orig_pan.x + delta.x, orig_pan.y + delta.y);
                        return Some(canvas::Action::publish(ImageCanvasEvent::Pan(new_pan)));
                    }
                    if self.tool == CanvasTool::Hand {
                        let delta = Vector::new(cursor_pos.x - start.x, cursor_pos.y - start.y);
                        let new_pan = Vector::new(orig_pan.x + delta.x, orig_pan.y + delta.y);
                        return Some(canvas::Action::publish(ImageCanvasEvent::Pan(new_pan)));
                    } else if self.tool == CanvasTool::Brush || self.tool == CanvasTool::Eraser {
                        let doc = self.screen_to_doc(cursor_pos, bounds);
                        let last = *state.stroke.last().unwrap_or(&(doc.x, doc.y));
                        let dist = ((doc.x - last.0).powi(2) + (doc.y - last.1).powi(2)).sqrt();
                        // Sampling: one point every ~1/3 radius
                        if dist >= (self.brush.radius * 0.35).max(1.0) {
                            state.stroke.push((doc.x, doc.y));
                            rasterize_segment(
                                &mut state.stroke_tex,
                                last,
                                (doc.x, doc.y),
                                &self.brush,
                            );
                        }
                        // Purely local preview: redraw without app round-trip
                        return Some(canvas::Action::request_redraw().and_capture());
                    }
                }
                // Brush/eraser hover: redraw to move preview circle
                if matches!(self.tool, CanvasTool::Brush | CanvasTool::Eraser) {
                    return Some(canvas::Action::request_redraw().and_capture());
                }
                None
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let delta_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 20.0,
                };
                if delta_y.abs() < 0.01 {
                    return None;
                }
                // Alt held + magnifier tool: invert zoom direction
                let delta_y = if self.tool == CanvasTool::Zoom && state.modifiers.alt() {
                    -delta_y
                } else {
                    delta_y
                };
                let factor = (1.12_f32).powf(delta_y);
                let new_zoom = (self.zoom * factor).clamp(0.08, 6.0);
                let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
                let factor_ratio = new_zoom / self.zoom;
                let new_pan = Vector::new(
                    cursor_pos.x - center.x - (cursor_pos.x - center.x - self.pan.x) * factor_ratio,
                    cursor_pos.y - center.y - (cursor_pos.y - center.y - self.pan.y) * factor_ratio,
                );
                Some(canvas::Action::publish(ImageCanvasEvent::ZoomPan {
                    zoom: new_zoom,
                    pan: new_pan,
                }))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Solid background — checker removed (caused slowdowns when zoomed out and resize bugs)
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), colors::BG_APP);

        let center = Point::new(
            bounds.width / 2.0 + self.pan.x,
            bounds.height / 2.0 + self.pan.y,
        );

        // Each layer drawn at its world position — drag only changes
        // an offset, no recomposite (Affinity model).
        // Convention : offset (0,0) = coin haut-gauche DU DOCUMENT
        // (same semantics as CPU composite and Properties panel).
        let (doc_half_w, doc_half_h) = self
            .doc_size
            .map_or((0.0, 0.0), |s| (s.width / 2.0, s.height / 2.0));
        for l in &self.layers {
            // Rotation applied around rect center by iced —
            // rect keeps original size (w_s×h_s), corners
            // naturally overflow without clipping. Convention
            // affine : offset = coin supérieur-gauche du rectangle
            // scalé, rotation autour de son centre.
            let w = l.width * l.scale_x * self.zoom;
            let h = l.height * l.scale_y * self.zoom;
            let top_left = Point::new(
                center.x + (l.offset_x - doc_half_w) * self.zoom,
                center.y + (l.offset_y - doc_half_h) * self.zoom,
            );
            frame.draw_image(
                Rectangle::new(top_left, Size::new(w, h)),
                iced_core::Image::new(l.handle.clone())
                    .opacity(l.opacity)
                    .rotation(iced::Radians(l.rotation_deg.to_radians())),
            );
        }

        // Brush preview: TEXTURES (one per 512×512 tile) drawn after
        // layer images. Fixed iced engine order per layer:
        // quads -> meshes -> images; vector geometry would go
        // UNDER layers. Priority to live stroke (drag); otherwise frozen
        // commit preview.
        let preview = state.stroke_tex.as_ref().or(self.pending_preview.as_ref());
        if let Some(t) = preview {
            for (x, y, rgba) in t.tiles() {
                let tl = Point::new(
                    center.x + (x - doc_half_w) * self.zoom,
                    center.y + (y - doc_half_h) * self.zoom,
                );
                frame.draw_image(
                    Rectangle::new(
                        tl,
                        Size::new(TILE as f32 * self.zoom, TILE as f32 * self.zoom),
                    ),
                    iced_core::Image::new(image::Handle::from_rgba(TILE, TILE, rgba.to_vec())),
                );
            }
        }

        // Document marker drawn IN world space → zoom-insensitive,
        // perfectly in sync with pan/zoom (overlay widget was distorting).
        if let Some(ds) = self.doc_size {
            let dw = ds.width * self.zoom;
            let dh = ds.height * self.zoom;
            let tl = Point::new(center.x - dw / 2.0, center.y - dh / 2.0);
            let outline = Path::rectangle(tl, Size::new(dw, dh));
            frame.stroke(
                &outline,
                canvas::Stroke::default()
                    .with_width(1.0)
                    .with_color(colors::BORDER_PANEL),
            );
            // Label dimensions au-dessus du coin haut-gauche
            frame.fill_text(iced::widget::canvas::Text {
                content: format!("{} × {}", ds.width as u32, ds.height as u32),
                position: Point::new(tl.x + 2.0, tl.y - 16.0),
                color: colors::TEXT_MUTED,
                size: iced::Pixels(10.0),
                ..Default::default()
            });
        }

        if self.layers.is_empty() && self.doc_size.is_none() {
            // No image: centered text
            frame.fill_text(iced::widget::canvas::Text {
                content: "Aucune image - Fichier > Ouvrir".into(),
                position: Point::new(bounds.width / 2.0, bounds.height / 2.0),
                color: colors::TEXT_MUTED,
                size: iced::Pixels(14.0),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                max_width: bounds.width,
                ..Default::default()
            });
        }

        // Grid removed — solid background only

        // Tool size preview — IMAGE above layers (hidden if layer masked)
        if self.can_paint
            && matches!(self.tool, CanvasTool::Brush | CanvasTool::Eraser)
            && let Some(pos) = state.cursor_pos
            && bounds.contains(pos)
        {
            let r = (self.brush.radius * self.zoom).max(2.0);
            // Generate small RGBA texture with grey circle — drawn as IMAGE
            // to go above layers (iced order: quads->meshes->images)
            let size = ((r * 2.0 + 6.0).ceil() as u32).clamp(8, 512);
            let mut rgba = vec![0u8; (size * size * 4) as usize];
            let cx = size as f32 / 2.0;
            let cy = size as f32 / 2.0;
            let thickness = if self.brush.erase {
                (r * 0.18).max(1.5)
            } else {
                1.4
            };
            for y in 0..size {
                for x in 0..size {
                    let dx = x as f32 + 0.5 - cx;
                    let dy = y as f32 + 0.5 - cy;
                    let d = (dx * dx + dy * dy).sqrt();
                    let (inner, outer) = if self.brush.erase {
                        let inner = (r - thickness).max(0.0);
                        (inner, r)
                    } else {
                        (r - thickness, r)
                    };
                    let cov = if d >= inner - 0.5 && d <= outer + 0.5 {
                        if d < inner {
                            (d - (inner - 0.5)).clamp(0.0, 1.0)
                        } else if d > outer {
                            (outer + 0.5 - d).clamp(0.0, 1.0)
                        } else {
                            1.0
                        }
                    } else {
                        continue;
                    };
                    if cov <= 0.01 {
                        continue;
                    }
                    let idx = ((y * size + x) * 4) as usize;
                    // Gris moyen 0.5 visible sur blanc et noir + alpha par couverture
                    let a = (cov * 230.0).round() as u8;
                    rgba[idx] = 128;
                    rgba[idx + 1] = 128;
                    rgba[idx + 2] = 128;
                    rgba[idx + 3] = a;
                    // White/black edge softened via alpha — grey stays readable
                }
            }
            // Center point for brush
            if !self.brush.erase {
                let dot_r = 1.2;
                for y in 0..size {
                    for x in 0..size {
                        let dx = x as f32 + 0.5 - cx;
                        let dy = y as f32 + 0.5 - cy;
                        if (dx * dx + dy * dy).sqrt() <= dot_r {
                            let idx = ((y * size + x) * 4) as usize;
                            rgba[idx] = 128;
                            rgba[idx + 1] = 128;
                            rgba[idx + 2] = 128;
                            rgba[idx + 3] = 255;
                        }
                    }
                }
            }
            let handle = image::Handle::from_rgba(size, size, rgba);
            let tl = Point::new(pos.x - cx, pos.y - cy);
            frame.draw_image(
                Rectangle::new(tl, Size::new(size as f32, size as f32)),
                iced_core::Image::new(handle),
            );
            let label = format!("{} px", (self.brush.radius * 2.0).round() as u32);
            let label_pos = Point::new(pos.x, pos.y - r - 10.0);
            // Semi-transparent background behind label for readability on white/black
            let label_bg = Rectangle::new(
                Point::new(label_pos.x - 22.0, label_pos.y - 7.0),
                Size::new(44.0, 14.0),
            );
            frame.fill_rectangle(
                label_bg.position(),
                label_bg.size(),
                colors::CANVAS_LABEL_BG,
            );
            frame.fill_text(iced::widget::canvas::Text {
                content: label,
                position: label_pos,
                color: colors::CANVAS_LABEL_FG,
                size: iced::Pixels(10.0),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..Default::default()
            });
        }

        // Marquee et visualiseur de transformation du calque sélectionné.
        // Dessinés dans une 2e géométrie → forcément AU-DESSUS des couches
        // (même du composite fallback plein cadre) : une géométrie ultérieure
        // est rendue APRÈS les images de la 1re.
        let overlay = self.draw_overlay(renderer, bounds, state);

        vec![Some(frame.into_geometry()), overlay]
            .into_iter()
            .flatten()
            .collect()
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if let Some(kind) = state.transform_handle {
            return transform_cursor(kind);
        }
        if state.dragging.is_some() {
            return mouse::Interaction::Grabbing;
        }
        if state.selecting.is_some() {
            return mouse::Interaction::Crosshair;
        }
        if cursor.is_over(bounds) {
            // Sur le visualiseur de transformation : curseurs dédiés
            // (POIGNÉES uniquement — l'intérieur est couvert plus bas).
            if matches!(self.tool, CanvasTool::Select | CanvasTool::Move)
                && let Some(pos) = cursor.position_in(bounds)
                && let Some((kind, _)) = self.hit_transform_handle(pos, bounds)
                && !matches!(kind, TransformHandle::Move)
            {
                return transform_cursor(kind);
            }
            // Sur un calque (sélectionnable) ou dans la boîte de la sélection :
            // curseur Déplacement identique pour Sélection et Déplacer.
            if matches!(self.tool, CanvasTool::Select | CanvasTool::Move)
                && let Some(pos) = cursor.position_in(bounds)
                && (self.pick_layer(pos, bounds).is_some() || self.in_transform_box(pos, bounds))
            {
                return mouse::Interaction::Move;
            }
            if matches!(self.tool, CanvasTool::Brush | CanvasTool::Eraser) && !self.can_paint {
                return mouse::Interaction::NotAllowed;
            }
            if state.space_held {
                return mouse::Interaction::Grab;
            }
            return match self.tool {
                CanvasTool::Hand => mouse::Interaction::Grab,
                CanvasTool::Move => mouse::Interaction::Move,
                CanvasTool::Brush | CanvasTool::Eraser => mouse::Interaction::Hidden,
                CanvasTool::Zoom => mouse::Interaction::ZoomIn,
                CanvasTool::Select => mouse::Interaction::Crosshair,
            };
        }
        mouse::Interaction::default()
    }
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn view_with_tool<'a>(
    doc_size: Option<Size>,
    pan: Vector,
    zoom: f32,
    tool: CanvasTool,
    selection: Option<Rectangle>,
    layers: Vec<CanvasLayer>,
    hit_layers: Vec<CanvasLayer>,
    transform_target: Option<CanvasLayer>,
    brush: BrushStyle,
    can_paint: bool,
    pending_preview: Option<StrokeTex>,
) -> iced::Element<'a, ImageCanvasEvent> {
    let program = ImageCanvas::new(doc_size, pan, zoom)
        .with_layers(layers)
        .with_hit_layers(hit_layers)
        .with_transform_target(transform_target)
        .with_tool(tool)
        .with_selection(selection)
        .with_brush(brush)
        .with_can_paint(can_paint)
        .with_pending_preview(pending_preview);
    iced::widget::canvas(program)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}

// ---------------------------------------------------------------------------
// Stroke preview rasterization (per 512×512 tile)
// ---------------------------------------------------------------------------

/// Rasterize segment `from -> to` (document coordinates) into preview.
/// Missing tiles are created on the fly; existing ones are never
/// moved (whole grid) → zero preview drift.
fn rasterize_segment(
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
        let first = &tex.tiles.iter().find(|t| t.tx == 0 && t.ty == 0).unwrap();
        assert!(alpha_at(first, 0, 0) > 0);
        let last = &tex.tiles.iter().find(|t| t.tx == 5 && t.ty == 5).unwrap();
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
