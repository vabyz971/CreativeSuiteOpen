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

//! Layer canvas — GPU compositing in render pass (zero CPU readback).
//!
//! Architecture ala Affinity/Photoshop:

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::unreadable_literal
)]
//! - each layer is a persistent GPU texture (uploaded ONCE per version)
//! - mask coverage is a SEPARATE persistent texture, sampled at draw time:
//!   painting a mask never regenerates the layer texture
//! - compositing happens in render passes on ping-pong textures, on the
//!   ICED wgpu device (via shader widget) — never transfers to CPU
//! - display is a blit with pan/zoom + procedural dotted background
//! - if stack doesn't change between two frames, blend passes are
//!   skipped: only blit runs
//!
//! Mouse events reuse [`image_canvas::ImageCanvasEvent`] to
//! stay compatible with existing app logic.

use std::collections::HashMap;
use std::sync::Arc;

use iced::wgpu;
use iced::widget::Shader;
use iced::widget::shader;
use iced::{Element, Length, Point, Rectangle, Size, Vector};

use crate::image_canvas::{
    BrushStyle, CanvasTool, ImageCanvasEvent, PICK_HOVER_STEP, StrokeTex, TransformHandle,
    point_in_quad, rasterize_segment,
};
use math_utils::Transform2D;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Displayed model
// ---------------------------------------------------------------------------

/// Une opération d'ajustement ACTIVE d'un calque de filtre, en unités
/// natives du moteur (mêmes `type_id` / plages que le registre d'effets :
/// `brightness_contrast`, `color_correct`, `blur`). Seuls les filtres à
/// effet réel sont représentés (`hue` est un no-op côté moteur).
#[derive(Clone, Debug)]
pub enum AdjustmentOp {
    /// Luminosité [-100 ; 100], contraste [-100 ; 100] (unités CPU).
    BrightnessContrast { brightness: f32, contrast: f32 },
    /// Saturation (1.0 = neutre).
    Saturation { value: f32 },
    /// Flou gaussien, sigma = rayon en px document (<= 0.1 = neutre).
    Blur { radius: f32 },
}

/// Contenu d'un [`DisplayLayer`] : pixel simple, sous-groupe, ou calque
/// d'ajustement (chaîne de filtres actifs). Un seul variant à la fois —
/// le CPU ne combine jamais groupe ET ajustement sur le même nœud.
#[derive(Clone, Debug)]
pub enum DisplayContent {
    /// Calque de pixels (texture `rgba`).
    Pixel,
    /// Groupe : les enfants composent d'abord dans une texture offscreen
    /// dédiée (mise en cache par signature), puis le résultat est fondu
    /// avec l'opacité / fusion / masque du groupe.
    Group(Vec<DisplayLayer>),
    /// Ajustement : la chaîne s'applique à l'accumulateur (tout ce qui est
    /// en dessous), pondérée par l'opacité de l'entrée.
    Adjustment(Vec<AdjustmentOp>),
}

/// A layer ready for GPU upload (preconverted shared RGBA8 pixels).
#[derive(Clone, Debug)]
pub struct DisplayLayer {
    /// Content identity (texture cache key, e.g. Arc pointer)
    pub key: u64,
    /// Pixels partagés RGBA8 (`None` pour les groupes et ajustements,
    /// qui n'ont pas de pixels propres).
    pub rgba: Option<Arc<[u8]>>,
    /// Dimensions LOGIQUES plein format (placement + normalisation UV).
    pub width: u32,
    /// Dimensions LOGIQUES plein format (placement + normalisation UV).
    pub height: u32,
    /// Dimensions RÉELLES du tampon `rgba` (≤ logiques : l'aperçu est
    /// réduit au-delà de 2048 px). L'upload utilise TOUJOURS ces dims —
    /// jamais les logiques — et l'échantillonnage [0,1] étire comme iced.
    pub tex_width: u32,
    /// Dimensions RÉELLES du tampon `rgba` (voir `tex_width`).
    pub tex_height: u32,
    /// Opacity 0..1
    pub opacity: f32,
    /// Blend mode (0 Normal ... 5 Lighten)
    pub blend: u32,
    /// Placement complet (offset, échelle, rotation, skew) — la matrice
    /// inverse est précalculée côté CPU (`affine_inverse`) et passée au
    /// shader, qui échantillonne en une fois.
    pub transform: Transform2D,
    /// Couverture de masque combinée (canal R), optionnelle.
    ///
    /// Échantillonnée AU DRAW dans le shader (`alpha × couverture`) : éditer
    /// un masque ne régénère jamais la texture du calque, seule la texture
    /// de masque est re-téléversée (clé = identité du contenu).
    ///
    /// Contrat d'espace : mêmes dimensions et même rect source que la
    /// texture du calque (le moteur fournit la couverture aux dims de
    /// l'image via `combined_mask_coverage` + rééchantillonnage).
    /// Pour les groupes : couverture en espace DOCUMENT.
    pub mask: Option<DisplayMask>,
    /// Contenu : pixel, sous-groupe ou ajustement.
    pub content: DisplayContent,
}

/// Couverture de masque prête pour upload GPU (pixels RGBA8 partagés).
#[derive(Clone, Debug)]
pub struct DisplayMask {
    /// Content identity (texture cache key, e.g. Arc pointer)
    pub key: u64,
    pub rgba: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
}

/// Calque cliquable (pick) : placement + dimensions logiques plein format.
/// Le visualiseur de transformation (boîte + poignées) n'est pas porté sur
/// le chemin GPU — le pick sert à sélectionner + déplacer au curseur.
#[derive(Clone, Debug)]
pub struct HitLayer {
    pub id: Option<Uuid>,
    pub transform: Transform2D,
    pub width: f32,
    pub height: f32,
}

/// Patch loupe pipette : carré `side`×`side` (pixels doc 1:1, RGBA8),
/// grossi ×4 à l'écran par le shader de présentation.
#[derive(Clone, Debug)]
pub struct LoupePatch {
    /// Content identity (dérivée du pointeur de l'Arc à la construction).
    pub key: u64,
    pub rgba: Arc<[u8]>,
    /// Côté du carré en pixels document.
    pub side: u32,
    /// Centre du patch en coordonnées document.
    pub center: (f32, f32),
}

/// Inverse affine pour l'échantillonnage shader : décompose
/// [`Transform2D::local_to_doc`] (scale → skew → rotation autour du centre,
/// puis offset) en matrice inverse 2×2 + origine + centre local.
///
/// Retourne `(xform, xform_off)` avec `xform = [n00, n01, n10, n11]` tel que
/// `local = N · (doc − origine) + (cx, cy)`.
/// `None` si dégénéré (échelle nulle…) : le calque ne contribue alors à rien.
fn affine_inverse(t: &Transform2D, w: f32, h: f32) -> Option<([f32; 4], [f32; 4])> {
    let kx = t.skew_x.to_radians().tan();
    let ky = t.skew_y.to_radians().tan();
    // M = cisaillement × échelle (même ordre que `local_to_doc`).
    let (m00, m01, m10, m11) = (t.scale_x, kx * t.scale_y, ky * t.scale_x, t.scale_y);
    // F = rotation × M.
    let rad = t.rotation_deg.to_radians();
    let (cos, sin) = (rad.cos(), rad.sin());
    let (f00, f01) = (cos * m00 - sin * m10, cos * m01 - sin * m11);
    let (f10, f11) = (sin * m00 + cos * m10, sin * m01 + cos * m11);
    let det = f00 * f11 - f01 * f10;
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let (cx, cy) = (w / 2.0, h / 2.0);
    Some((
        [f11 / det, -f01 / det, -f10 / det, f00 / det],
        [
            cx * t.scale_x + t.offset_x,
            cy * t.scale_y + t.offset_y,
            cx,
            cy,
        ],
    ))
}

/// Uniforms shader pour UNE opération d'ajustement (les autres canaux au
/// neutre) + rayon de flou éventuel. Conversions CPU → GPU identiques à
/// `GpuContext` (`engines/photo-engine/src/gpu.rs`) : luminosité ×0.01,
/// contraste 1+c/100 (négatif) ou 1+c/50 (positif).
fn adjust_uniforms(op: &AdjustmentOp) -> ([f32; 4], f32) {
    const NEUTRE: [f32; 4] = [0.0, 1.0, 1.0, 0.0];
    match *op {
        AdjustmentOp::BrightnessContrast {
            brightness,
            contrast,
        } => {
            let facteur = if contrast < 0.0 {
                1.0 + contrast / 100.0
            } else {
                1.0 + contrast / 50.0
            };
            ([brightness * 0.01, facteur, 1.0, 0.0], 0.0)
        }
        AdjustmentOp::Saturation { value } => ([0.0, 1.0, value, 0.0], 0.0),
        AdjustmentOp::Blur { radius } => {
            if radius <= 0.1 {
                (NEUTRE, 0.0)
            } else {
                (NEUTRE, radius)
            }
        }
    }
}

pub struct LayerCanvas<Message> {
    pub layers: Vec<DisplayLayer>,
    /// Calques cliquables (haut de pile en dernier) pour l'outil Select.
    pub hit_layers: Vec<HitLayer>,
    /// Document dimensions in pixels (None = no document)
    pub doc_size: Option<(f32, f32)>,
    pub pan: Vector,
    pub zoom: f32,
    pub tool: CanvasTool,
    pub selection: Option<Rectangle>,
    /// Style du pinceau / gomme (aperçu local, comme `image_canvas`).
    pub brush: BrushStyle,
    /// Faux si aucun calque peinturable (clic pinceau ignoré).
    pub can_paint: bool,
    /// Aperçu figé du commit en cours (texture, fournie par l'app).
    pub pending_preview: Option<StrokeTex>,
    /// Patch loupe pipette en cours (pixels fournis par l'app).
    pub loupe_patch: Option<LoupePatch>,
    /// Convert canvas events to app messages
    pub on_event: std::rc::Rc<dyn Fn(ImageCanvasEvent) -> Message>,
}

impl<Message> LayerCanvas<Message> {
    /// Écran (bounds) → coordonnées document (origine coin haut-gauche),
    /// même convention que `ImageCanvas::screen_to_doc`.
    fn screen_to_doc(&self, p: Point, bounds: Rectangle) -> (f32, f32) {
        let center = Point::new(
            bounds.width / 2.0 + self.pan.x,
            bounds.height / 2.0 + self.pan.y,
        );
        let (hw, hh) = self.doc_size.unwrap_or((0.0, 0.0));
        (
            (p.x - center.x) / self.zoom + hw / 2.0,
            (p.y - center.y) / self.zoom + hh / 2.0,
        )
    }

    pub fn new(
        doc_size: Option<(f32, f32)>,
        on_event: std::rc::Rc<dyn Fn(ImageCanvasEvent) -> Message>,
    ) -> Self {
        Self {
            layers: Vec::new(),
            hit_layers: Vec::new(),
            doc_size,
            pan: Vector::new(0.0, 0.0),
            zoom: 1.0,
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
            loupe_patch: None,
            on_event,
        }
    }

    #[must_use]
    pub fn with_layers(mut self, layers: Vec<DisplayLayer>) -> Self {
        self.layers = layers;
        self
    }

    /// Calques cliquables pour le pick (outil Select) — même ordre que
    /// `layers` (haut de pile en dernier).
    #[must_use]
    pub fn with_hit_layers(mut self, layers: Vec<HitLayer>) -> Self {
        self.hit_layers = layers;
        self
    }

    /// Pick : id du calque sous le point document (haut de pile d'abord).
    /// Même sémantique que `ImageCanvas::pick_layer` (sans la boîte de
    /// transformation, non portée sur le chemin GPU).
    fn pick(&self, doc: (f32, f32)) -> Option<Uuid> {
        let p = Point::new(doc.0, doc.1);
        self.hit_layers.iter().rev().find_map(|l| {
            let id = l.id?;
            let coins = l.transform.doc_corners(l.width, l.height);
            let quad = coins.map(|(x, y)| Point::new(x, y));
            point_in_quad(p, quad).then_some(id)
        })
    }

    #[must_use]
    pub fn with_view(mut self, pan: Vector, zoom: f32) -> Self {
        self.pan = pan;
        self.zoom = zoom.clamp(0.08, 6.0);
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

    /// Patch loupe pipette (pixels RGBA8 `side`×`side` fournis par l'app,
    /// échantillonnés sur la composite). Clé dérivée du contenu partagé.
    #[must_use]
    pub fn with_loupe_patch(mut self, rgba: Option<(Arc<[u8]>, u32)>) -> Self {
        self.loupe_patch = rgba.map(|(pixels, side)| LoupePatch {
            // Pointeur fin (même motif que `arc_addr` côté app) : le clone
            // conservé empêche la réutilisation de l'adresse (ABA).
            key: Arc::as_ptr(&pixels).cast::<u8>() as usize as u64,
            rgba: pixels,
            side: side.max(1),
            center: (0.0, 0.0),
        });
        self
    }
}

#[must_use]
pub fn view<'a, Message>(canvas: LayerCanvas<Message>) -> Element<'a, Message>
where
    Message: 'static + Clone,
{
    Shader::new(canvas)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ---------------------------------------------------------------------------
// Interaction state (same logic as image_canvas)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct State {
    dragging: Option<(Point, Vector)>,
    selecting: Option<(Point, Point)>,
    modifiers: iced::keyboard::Modifiers,
    prev_bounds: Option<Size>,
    /// Points du trait en cours (coordonnées document) — aperçu local
    /// sans aller-retour applicatif, comme `image_canvas`.
    stroke: Vec<(f32, f32)>,
    /// Version rastérisée du trait (tuiles espace document).
    stroke_tex: Option<StrokeTex>,
    /// Génération du trait : chaque segment rastérisé l'incrémente pour
    /// invalider les tuiles d'aperçu mises en cache côté GPU.
    stroke_gen: u64,
    /// Dernière position doc publiée pour la loupe pipette (quanta).
    last_pick_doc: Option<(f32, f32)>,
}

impl<Message> shader::Program<Message> for LayerCanvas<Message>
where
    Message: Clone + 'static,
{
    type State = State;
    type Primitive = CompositePrimitive;

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<shader::Action<Message>> {
        use iced::Event;
        use iced::mouse::{self, Button};

        // Publish viewport size on each change
        if let Event::Window(iced::window::Event::RedrawRequested(_)) = event {
            if state.prev_bounds != Some(bounds.size()) {
                state.prev_bounds = Some(bounds.size());
                return Some(
                    shader::Action::publish((self.on_event)(ImageCanvasEvent::Viewport(
                        bounds.size(),
                    )))
                    .and_capture(),
                );
            }
            return None;
        }

        if let Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) = event {
            state.modifiers = *m;
            return None;
        }

        // Release handled even outside bounds (mouse is captured during
        // drag) — otherwise state stays armed and events keep coming.
        if let Event::Mouse(mouse::Event::ButtonReleased(Button::Left)) = event {
            if let Some((start, end)) = state.selecting.take() {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return Some(shader::Action::publish((self.on_event)(
                        ImageCanvasEvent::SelectRect(None),
                    )));
                };
                let rect = Rectangle::new(start, Size::new(end.x - start.x, end.y - start.y));
                let norm = Rectangle::new(
                    Point::new(
                        rect.x.min(rect.x + rect.width),
                        rect.y.min(rect.y + rect.height),
                    ),
                    Size::new(rect.width.abs(), rect.height.abs()),
                );
                if norm.width > 5.0 && norm.height > 5.0 {
                    return Some(shader::Action::publish((self.on_event)(
                        ImageCanvasEvent::SelectRect(Some(norm)),
                    )));
                } else if self.tool == CanvasTool::Zoom {
                    let base_factor = 1.4_f32;
                    let factor = if state.modifiers.alt() {
                        1.0 / base_factor
                    } else {
                        base_factor
                    };
                    let new_zoom = (self.zoom * factor).clamp(0.08, 6.0);
                    let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
                    let ratio = new_zoom / self.zoom;
                    let new_pan = Vector::new(
                        cursor_pos.x - center.x - (cursor_pos.x - center.x - self.pan.x) * ratio,
                        cursor_pos.y - center.y - (cursor_pos.y - center.y - self.pan.y) * ratio,
                    );
                    return Some(shader::Action::publish((self.on_event)(
                        ImageCanvasEvent::ZoomAt {
                            zoom: new_zoom,
                            pan: new_pan,
                        },
                    )));
                }
                return Some(shader::Action::publish((self.on_event)(
                    ImageCanvasEvent::SelectRect(None),
                )));
            }
            if state.dragging.take().is_some() {
                match self.tool {
                    // Move, ou Select après un pick (marquee = selecting).
                    CanvasTool::Move | CanvasTool::Select => {
                        return Some(
                            shader::Action::publish((self.on_event)(
                                ImageCanvasEvent::TransformEnd,
                            ))
                            .and_capture(),
                        );
                    }
                    CanvasTool::Brush | CanvasTool::Eraser => {
                        let points = std::mem::take(&mut state.stroke);
                        let tex = state.stroke_tex.take();
                        let erase = self.tool == CanvasTool::Eraser;
                        return Some(
                            shader::Action::publish((self.on_event)(ImageCanvasEvent::BrushEnd {
                                points,
                                tex,
                                erase,
                            }))
                            .and_capture(),
                        );
                    }
                    _ => {}
                }
            }
            return Some(shader::Action::capture());
        }

        let cursor_pos = cursor.position_in(bounds)?;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(Button::Left)) => match self.tool {
                CanvasTool::Hand => {
                    state.dragging = Some((cursor_pos, self.pan));
                    Some(shader::Action::capture())
                }
                CanvasTool::Move => {
                    state.dragging = Some((cursor_pos, self.pan));
                    Some(
                        shader::Action::publish((self.on_event)(
                            ImageCanvasEvent::TransformStart {
                                id: None,
                                kind: TransformHandle::Move,
                                doc: self.screen_to_doc(cursor_pos, bounds),
                            },
                        ))
                        .and_capture(),
                    )
                }
                CanvasTool::Zoom => {
                    state.selecting = Some((cursor_pos, cursor_pos));
                    Some(shader::Action::capture())
                }
                CanvasTool::Select => {
                    // Pick : sur un calque → sélection + déplacement direct
                    // (même protocole que `image_canvas`, sans la boîte de
                    // transformation) ; dans le vide → marquee.
                    let doc = self.screen_to_doc(cursor_pos, bounds);
                    if let Some(id) = self.pick(doc) {
                        state.dragging = Some((cursor_pos, self.pan));
                        Some(
                            shader::Action::publish((self.on_event)(
                                ImageCanvasEvent::TransformStart {
                                    id: Some(id),
                                    kind: TransformHandle::Move,
                                    doc,
                                },
                            ))
                            .and_capture(),
                        )
                    } else {
                        state.selecting = Some((cursor_pos, cursor_pos));
                        Some(shader::Action::capture())
                    }
                }
                CanvasTool::Brush | CanvasTool::Eraser => {
                    if !self.can_paint {
                        return Some(shader::Action::capture());
                    }
                    // Même protocole que `image_canvas` : l'app commit les
                    // pixels, le canvas ne fait que l'aperçu local.
                    let (doc_x, doc_y) = self.screen_to_doc(cursor_pos, bounds);
                    state.stroke = vec![(doc_x, doc_y)];
                    state.stroke_tex = None;
                    state.dragging = Some((cursor_pos, self.pan));
                    let erase = self.tool == CanvasTool::Eraser;
                    Some(
                        shader::Action::publish((self.on_event)(ImageCanvasEvent::BrushStart {
                            x: doc_x,
                            y: doc_y,
                            erase,
                        }))
                        .and_capture(),
                    )
                }
                CanvasTool::Eyedropper => {
                    // Échantillonnage composite côté app (worker, voir
                    // `handle_pick_color`) : le canvas ne fait que relayer
                    // le point document, comme `image_canvas`.
                    let (doc_x, doc_y) = self.screen_to_doc(cursor_pos, bounds);
                    state.last_pick_doc = Some((doc_x, doc_y));
                    Some(
                        shader::Action::publish((self.on_event)(ImageCanvasEvent::ColorPick {
                            x: doc_x,
                            y: doc_y,
                        }))
                        .and_capture(),
                    )
                }
            },
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some((start, orig_pan)) = state.dragging {
                    if self.tool == CanvasTool::Hand {
                        let delta = Vector::new(cursor_pos.x - start.x, cursor_pos.y - start.y);
                        return Some(shader::Action::publish((self.on_event)(
                            ImageCanvasEvent::Pan(Vector::new(
                                orig_pan.x + delta.x,
                                orig_pan.y + delta.y,
                            )),
                        )));
                    } else if matches!(self.tool, CanvasTool::Move | CanvasTool::Select) {
                        // Curseur en coordonnées doc (pan/zoom pris en compte).
                        // Select + drag = suite d'un pick (marquee = selecting).
                        return Some(shader::Action::publish((self.on_event)(
                            ImageCanvasEvent::TransformCursor {
                                doc: self.screen_to_doc(cursor_pos, bounds),
                                uniform: state.modifiers.control(),
                                snap: state.modifiers.shift(),
                            },
                        )));
                    } else if matches!(self.tool, CanvasTool::Brush | CanvasTool::Eraser) {
                        // Aperçu purement local : rastérise le segment dans
                        // les tuiles (logique partagée `rasterize_segment`),
                        // sans aller-retour applicatif.
                        let (doc_x, doc_y) = self.screen_to_doc(cursor_pos, bounds);
                        let last = *state.stroke.last().unwrap_or(&(doc_x, doc_y));
                        let dist = ((doc_x - last.0).powi(2) + (doc_y - last.1).powi(2)).sqrt();
                        // Échantillonnage : un point tous les ~1/3 de rayon.
                        if dist >= (self.brush.radius * 0.35).max(1.0) {
                            state.stroke.push((doc_x, doc_y));
                            rasterize_segment(
                                &mut state.stroke_tex,
                                last,
                                (doc_x, doc_y),
                                &self.brush,
                            );
                            state.stroke_gen = state.stroke_gen.wrapping_add(1);
                        }
                        return Some(shader::Action::request_redraw().and_capture());
                    }
                }
                if let Some((start, _)) = state.selecting {
                    state.selecting = Some((start, cursor_pos));
                    return Some(shader::Action::request_redraw().and_capture());
                }
                // Survol pinceau/gomme : redessine pour déplacer l'anneau.
                if matches!(self.tool, CanvasTool::Brush | CanvasTool::Eraser) {
                    return Some(shader::Action::request_redraw().and_capture());
                }
                // Pipette + loupe : un patch par quantum de mouvement doc
                // (`PICK_HOVER_STEP`, même cadence que `image_canvas`) ;
                // l'app échantillonne dans un worker et renvoie une texture.
                if self.tool == CanvasTool::Eyedropper
                    && state.dragging.is_none()
                    && state.selecting.is_none()
                {
                    let (doc_x, doc_y) = self.screen_to_doc(cursor_pos, bounds);
                    let loin = state
                        .last_pick_doc
                        .map(|(ancien_x, ancien_y)| {
                            (doc_x - ancien_x).powi(2) + (doc_y - ancien_y).powi(2)
                                >= PICK_HOVER_STEP.powi(2)
                        })
                        .unwrap_or(true);
                    if loin {
                        state.last_pick_doc = Some((doc_x, doc_y));
                        return Some(
                            shader::Action::publish((self.on_event)(ImageCanvasEvent::PickHover {
                                x: doc_x,
                                y: doc_y,
                            }))
                            .and_capture(),
                        );
                    }
                    return Some(shader::Action::request_redraw().and_capture());
                }
                None
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let delta_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 20.0,
                };
                if delta_y.abs() < 0.01 {
                    return None;
                }
                let delta_y = if self.tool == CanvasTool::Zoom && state.modifiers.alt() {
                    -delta_y
                } else {
                    delta_y
                };
                let factor = 1.12_f32.powf(delta_y);
                let new_zoom = (self.zoom * factor).clamp(0.08, 6.0);
                let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
                let ratio = new_zoom / self.zoom;
                let new_pan = Vector::new(
                    cursor_pos.x - center.x - (cursor_pos.x - center.x - self.pan.x) * ratio,
                    cursor_pos.y - center.y - (cursor_pos.y - center.y - self.pan.y) * ratio,
                );
                Some(shader::Action::publish((self.on_event)(
                    ImageCanvasEvent::ZoomPan {
                        zoom: new_zoom,
                        pan: new_pan,
                    },
                )))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        cursor: iced::mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        // Tuiles d'aperçu du trait (geste en cours + commit figé) : elles
        // deviennent des calques overlay épinglés en haut de pile, avec des
        // clés de génération — le pipeline les téléverse et les fusionne
        // comme des pixels ordinaires, puis les évince avec la pile.
        const SEL_DIRECT: u64 = 0x9E37_79B9_7F4A_7C15;
        const SEL_FIGE: u64 = 0xBF58_476D_1CE4_E5B9;
        let mut layers = self.layers.clone();
        if let Some(tex) = state.stroke_tex.as_ref().or(self.pending_preview.as_ref()) {
            let en_cours = state.stroke_tex.is_some();
            for (i, (ox, oy, rgba)) in tex.tiles_cloned().into_iter().enumerate() {
                let key = if en_cours {
                    SEL_DIRECT
                        .wrapping_add(state.stroke_gen)
                        .wrapping_add(i as u64)
                        .wrapping_add((ox.to_bits() as u64) << 32 | oy.to_bits() as u64)
                } else {
                    // Aperçu figé : clé = contenu (même objet → même clé,
                    // pas de re-téléversement ; nouvel objet → nouvelle clé).
                    let mut h = SEL_FIGE ^ rgba.len() as u64;
                    for b in rgba.iter().step_by(997) {
                        h = h.wrapping_mul(0x100000001b3) ^ u64::from(*b);
                    }
                    h.wrapping_add(i as u64)
                        .wrapping_add((ox.to_bits() as u64) << 32 | oy.to_bits() as u64)
                };
                // Tuiles toujours pleines TILE×TILE (cf. `StrokeTex` :
                // allocation fixe, tampons rognés à l'intérieur).
                let w = 512u32;
                let h = 512u32;
                layers.push(DisplayLayer {
                    key,
                    rgba: Some(Arc::<[u8]>::from(rgba)),
                    width: w,
                    height: h,
                    // Tuiles toujours pleines : tampon = logique.
                    tex_width: w,
                    tex_height: h,
                    opacity: 1.0,
                    blend: 0,
                    transform: Transform2D {
                        offset_x: ox,
                        offset_y: oy,
                        ..Transform2D::default()
                    },
                    mask: None,
                    content: DisplayContent::Pixel,
                });
            }
        }
        // Anneau curseur pinceau/gomme (doc + rayon px doc).
        let curseur = if matches!(self.tool, CanvasTool::Brush | CanvasTool::Eraser) {
            cursor.position_in(bounds).map(|p| {
                let (doc_x, doc_y) = self.screen_to_doc(p, bounds);
                let sorte = if self.tool == CanvasTool::Eraser {
                    2
                } else {
                    1
                };
                (doc_x, doc_y, self.brush.radius.max(0.5), sorte)
            })
        } else {
            None
        };
        // Loupe pipette : patch fourni par l'app, centré sur le dernier
        // point de survol publié.
        let loupe = self.loupe_patch.as_ref().and_then(|patch| {
            state.last_pick_doc.map(|centre| LoupePatch {
                key: patch.key,
                rgba: Arc::clone(&patch.rgba),
                side: patch.side,
                center: centre,
            })
        });
        CompositePrimitive {
            layers,
            doc_size: self.doc_size.unwrap_or((800.0, 600.0)),
            has_doc: self.doc_size.is_some(),
            pan: self.pan,
            zoom: self.zoom,
            viewport: (bounds.width.max(1.0), bounds.height.max(1.0)),
            selection: self.selection,
            curseur,
            loupe,
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        use iced::mouse::Interaction;
        if state.dragging.is_some() {
            return Interaction::Grabbing;
        }
        if state.selecting.is_some() {
            return Interaction::Crosshair;
        }
        if cursor.is_over(bounds) {
            // Sur un calque cliquable (outil Select) : curseur Déplacement.
            if self.tool == CanvasTool::Select
                && let Some(pos) = cursor.position_in(bounds)
            {
                let doc = self.screen_to_doc(pos, bounds);
                if self.pick(doc).is_some() {
                    return Interaction::Move;
                }
            }
            if matches!(self.tool, CanvasTool::Brush | CanvasTool::Eraser) && !self.can_paint {
                return Interaction::NotAllowed;
            }
            return match self.tool {
                CanvasTool::Hand => Interaction::Grab,
                CanvasTool::Move => Interaction::Move,
                // Anneau dessiné par le shader : curseur OS masqué.
                CanvasTool::Brush | CanvasTool::Eraser => Interaction::Hidden,
                CanvasTool::Zoom => Interaction::ZoomIn,
                CanvasTool::Select => Interaction::Crosshair,
                CanvasTool::Eyedropper => Interaction::Crosshair,
            };
        }
        Interaction::default()
    }
}

// ---------------------------------------------------------------------------
// Primitive GPU
// ---------------------------------------------------------------------------

/// Config hash: if unchanged, blend passes are skipped.
/// Récursif : inclut transform, contenu (groupe / ajustement) et masques —
/// tout changement d'un seul nœud invalide le composite GPU.
fn config_hash(layers: &[DisplayLayer], doc: (f32, f32)) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    nourrir_couche(&mut h, layers, doc);
    h
}

fn nourrir(h: &mut u64, v: u64) {
    *h ^= v;
    *h = h.wrapping_mul(0x100000001b3);
}

fn nourrir_couche(h: &mut u64, layers: &[DisplayLayer], doc: (f32, f32)) {
    nourrir(h, u64::from(doc.0.to_bits()));
    nourrir(h, u64::from(doc.1.to_bits()));
    nourrir(h, layers.len() as u64);
    for l in layers {
        nourrir(h, l.key);
        nourrir(h, u64::from(l.opacity.to_bits()));
        nourrir(h, u64::from(l.blend));
        let t = &l.transform;
        for v in [
            t.offset_x,
            t.offset_y,
            t.rotation_deg,
            t.scale_x,
            t.scale_y,
            t.skew_x,
            t.skew_y,
        ] {
            nourrir(h, u64::from(v.to_bits()));
        }
        nourrir(h, u64::from(l.width));
        nourrir(h, u64::from(l.height));
        match &l.mask {
            Some(m) => {
                nourrir(h, m.key);
                nourrir(h, u64::from(m.width));
                nourrir(h, u64::from(m.height));
            }
            None => nourrir(h, 0),
        }
        match &l.content {
            DisplayContent::Pixel => nourrir(h, 0),
            DisplayContent::Group(enfants) => {
                nourrir(h, 1);
                nourrir_couche(h, enfants, doc);
            }
            DisplayContent::Adjustment(ops) => {
                nourrir(h, 2);
                nourrir(h, ops.len() as u64);
                for op in ops {
                    match *op {
                        AdjustmentOp::BrightnessContrast {
                            brightness,
                            contrast,
                        } => {
                            nourrir(h, 10);
                            nourrir(h, u64::from(brightness.to_bits()));
                            nourrir(h, u64::from(contrast.to_bits()));
                        }
                        AdjustmentOp::Saturation { value } => {
                            nourrir(h, 11);
                            nourrir(h, u64::from(value.to_bits()));
                        }
                        AdjustmentOp::Blur { radius } => {
                            nourrir(h, 12);
                            nourrir(h, u64::from(radius.to_bits()));
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompositePrimitive {
    pub layers: Vec<DisplayLayer>,
    pub doc_size: (f32, f32),
    pub has_doc: bool,
    pub pan: Vector,
    pub zoom: f32,
    pub viewport: (f32, f32),
    pub selection: Option<Rectangle>,
    /// Anneau curseur pinceau/gomme : (doc x, doc y, rayon px doc, 1/2).
    pub curseur: Option<(f32, f32, f32, u32)>,
    /// Patch loupe pipette (centré sur le survol).
    pub loupe: Option<LoupePatch>,
}

impl shader::Primitive for CompositePrimitive {
    type Pipeline = CompositePipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        pipeline.prepare(self, device, queue, viewport.scale_factor());
    }

    fn draw(&self, _pipeline: &Self::Pipeline, _render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        // Blending requires its own offscreen passes → render()
        false
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        pipeline.render(self, encoder, target, clip_bounds);
    }
}

// ---------------------------------------------------------------------------
// Shaders WGSL
// ---------------------------------------------------------------------------

const SHADER: &str = include_str!("shaders/layer_blend.wgsl");

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    /// xy = viewport widget px, zw = document px
    screen_doc: [f32; 4],
    /// xy = pan, z = zoom, w = layer opacity
    pan_zoom: [f32; 4],
    /// x = blend mode, y/z = top texture dims, w = image flag present
    mode_sizes: [u32; 4],
    /// xy = réservé (placement = xform), zw = selection position
    off_sel: [f32; 4],
    /// xy = selection size (x > 0 = active)
    sel_size: [f32; 4],
    /// x = 1 si masque, y/z = dims texture masque, w = 1 si masque doc
    mask_info: [u32; 4],
    /// Matrice inverse 2x2 (n00, n01, n10, n11) — voir `affine_inverse`.
    xform: [f32; 4],
    /// xy = origine doc, zw = centre local (cx, cy).
    xform_off: [f32; 4],
    /// x = luminosité, y = contraste, z = saturation, w = réservé.
    adjust: [f32; 4],
    /// x = rayon flou px doc, y = axe (0 = H, 1 = V), zw réservés.
    blur_px: [f32; 4],
    /// xy = curseur doc, z = rayon px doc, w : 0 inactif, 1/2 pinceau/gomme.
    curseur: [f32; 4],
    /// xy = centre doc patch, z = côté px doc, w = 1 si loupe active.
    loupe: [f32; 4],
}

impl Params {
    /// Uniforms neutres pour la passe de présentation (ni placement, ni
    /// ajustement, ni curseur) — seuls viewport, sélection, curseur et
    /// loupe varient.
    fn neutres(prim: &CompositePrimitive, echelle: f32) -> Self {
        let echelle = echelle.max(0.01);
        let (pos_sel, taille_sel) = match prim.selection {
            Some(r) => (
                [r.x * echelle, r.y * echelle, 0.0, 0.0],
                [r.width * echelle, r.height * echelle, 0.0, 0.0],
            ),
            None => ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0]),
        };
        // Tout l'espace ÉCRAN est en pixels physiques (cible = surface) :
        // viewport, pan et zoom (×échelle) ; le document reste inchangé.
        let zoom_ecran = prim.zoom * echelle;
        Self {
            screen_doc: [
                prim.viewport.0 * echelle,
                prim.viewport.1 * echelle,
                prim.doc_size.0,
                prim.doc_size.1,
            ],
            pan_zoom: [prim.pan.x * echelle, prim.pan.y * echelle, zoom_ecran, 1.0],
            mode_sizes: [0, 0, 0, u32::from(prim.has_doc)],
            off_sel: pos_sel,
            sel_size: taille_sel,
            mask_info: [0, 1, 1, 0],
            xform: [1.0, 0.0, 0.0, 1.0],
            xform_off: [0.0, 0.0, 0.0, 0.0],
            adjust: [0.0, 1.0, 1.0, 0.0],
            blur_px: [0.0, 0.0, 0.0, 0.0],
            curseur: prim
                .curseur
                .map(|(x, y, r, s)| [x, y, r, s as f32])
                .unwrap_or([0.0, 0.0, 0.0, 0.0]),
            loupe: prim
                .loupe
                .as_ref()
                .map(|p| (p.center.0, p.center.1, p.side as f32))
                .map(|(x, y, c)| [x, y, c, 1.0])
                .unwrap_or([0.0, 0.0, 0.0, 0.0]),
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

/// Ajustement neutre (identité) pour les passes couleur.
const AJUST_NEUTRE: [f32; 4] = [0.0, 1.0, 1.0, 0.0];

/// Le tampon couvre-t-il la texture déclarée ? Garde anti-panic de
/// l'upload (un aperçu réduit + des dims logiques plein format donnerait
/// une lecture hors limites — voir `televerser_portee`).
fn tampon_valide(longueur: usize, tex_l: u32, tex_h: u32) -> bool {
    longueur >= tex_l.max(1) as usize * tex_h.max(1) as usize * 4
}

/// Contexte d'une portée composite : dimensions document + cadrage écran.
#[derive(Clone, Copy)]
struct ScopeCtx {
    doc: (f32, f32),
    viewport: (f32, f32),
    pan: Vector,
    zoom: f32,
}

pub struct CompositePipeline {
    device: wgpu::Device,
    queue: wgpu::Queue,

    blend_pipeline: wgpu::RenderPipeline,
    /// Passe d'ajustement couleur (une opération : BC ou saturation).
    adjust_pipeline: wgpu::RenderPipeline,
    /// Flou séparable (H puis V) pour les calques d'ajustement.
    blur_pipeline: wgpu::RenderPipeline,
    present_pipeline: wgpu::RenderPipeline,
    bgl_all: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,

    /// Persistent layer textures, key = content identity
    layer_textures: HashMap<u64, LayerTex>,
    /// Persistent mask-coverage textures, key = content identity
    mask_textures: HashMap<u64, LayerTex>,
    /// 1×1 opaque fallback bound when a layer has no mask
    white_tex: LayerTex,
    /// Ping-pong accumulators (document space)
    accum: Option<Accum>,
    /// Scratch document-size (copie d'accumulateur pour le mix des
    /// ajustements). Créé dans `prepare`, jamais dans le thread de rendu.
    scratch: Option<RenderTex>,
    /// Composites de groupes (espace document), clé = identité du groupe,
    /// invalidés par la signature récursive du sous-arbre — même principe
    /// que `Renderer::appearance` côté moteur : seul un groupe modifié
    /// est recomposé. `Mutex` (et verrouillages courts) car `render()` ne
    /// prend que `&self` et le trait `Pipeline` exige `Sync`.
    group_cache: std::sync::Mutex<HashMap<u64, CachedGroup>>,
    /// Patch loupe pipette en cours (clé + texture).
    loupe_tex: Option<(u64, LayerTex)>,
    /// Facteur d'échelle écran (HiDPI), capturé dans `prepare()` depuis le
    /// viewport iced : les bornes du widget sont LOGIQUES mais la cible de
    /// la passe de présentation est PHYSIQUE. Sans cette mise à l'échelle,
    /// l'image est réduite/décalée dès que le facteur ≠ 1.
    /// Bits de f32 (atomique : le trait `Pipeline` exige `Sync`).
    echelle: std::sync::atomic::AtomicU32,
    /// Hash of last GPU recomposite (atomic: `render()` takes &self)
    last_hash: std::sync::atomic::AtomicU64,
}

struct LayerTex {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

/// Texture possédée lisible ET cible de rendu (groupes, scratch).
struct RenderTex {
    #[allow(dead_code)]
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

/// Composite d'un groupe mis en cache : paire ping-pong + signature du
/// sous-arbre. `current` en `Cell` (mutation sans emprunt pendant la
/// récursion de rendu).
struct CachedGroup {
    hash: u64,
    size: (u32, u32),
    views: [RenderTex; 2],
    current: std::cell::Cell<usize>,
}

struct Accum {
    views: [wgpu::TextureView; 2],
    /// Index of texture containing last composite
    current: std::sync::atomic::AtomicUsize,
    size: (u32, u32),
}

impl shader::Pipeline for CompositePipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self
    where
        Self: Sized,
    {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("layer-canvas"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        // Layout unique : base tex+sampler, top tex+sampler, uniform
        let tex_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let samp_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let bgl_all = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("layer-canvas-all"),
            entries: &[
                tex_entry(0),
                samp_entry(1),
                tex_entry(2),
                samp_entry(3),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(5),
                samp_entry(6),
            ],
        });

        let pll = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layer-canvas-blend-layout"),
            bind_group_layouts: &[&bgl_all],
            push_constant_ranges: &[],
        });
        let blend_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("layer-canvas-blend"),
            layout: Some(&pll),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_blend"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Passes d'ajustement : même layout et même cible que le blend
        // (Rgba8 doc) — seuls les entry points changent.
        let mk_pass = |label: &str, entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pll),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let adjust_pipeline = mk_pass("layer-canvas-adjust", "fs_adjust");
        let blur_pipeline = mk_pass("layer-canvas-blur", "fs_blur");

        let plp = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layer-canvas-present-layout"),
            bind_group_layouts: &[&bgl_all],
            push_constant_ranges: &[],
        });
        let present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("layer-canvas-present"),
            layout: Some(&plp),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_present"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("layer-canvas-linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let white_tex = Self::upload_texture(
            device,
            queue,
            "layer-canvas-white",
            &[255, 255, 255, 255],
            1,
            1,
        );
        Self {
            device: device.clone(),
            queue: queue.clone(),
            blend_pipeline,
            adjust_pipeline,
            blur_pipeline,
            present_pipeline,
            bgl_all,
            sampler,
            layer_textures: HashMap::new(),
            mask_textures: HashMap::new(),
            white_tex,
            accum: None,
            scratch: None,
            group_cache: std::sync::Mutex::new(HashMap::new()),
            loupe_tex: None,
            echelle: std::sync::atomic::AtomicU32::new(1.0_f32.to_bits()),
            last_hash: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn trim(&mut self) {}
}

impl CompositePipeline {
    /// Full bind group: base + top + mask textures, shared sampler, uniforms.
    /// `mask_view = None` → opaque 1×1 fallback (shader skips sampling via
    /// `mask_info.x`, but every binding must be bound).
    /// Le buffer d'uniforms est FRAIS par passe : les écritures `queue`
    /// s'exécutent avant la soumission de l'encodeur, donc UN SEUL buffer
    /// partagé verrait toutes les passes lire les params de la dernière.
    fn scene_bg(
        &self,
        view: &wgpu::TextureView,
        top_view: Option<&wgpu::TextureView>,
        mask_view: Option<&wgpu::TextureView>,
        params_buf: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        let fallback = view;
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("layer-canvas-scene-bg"),
            layout: &self.bgl_all,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(top_view.unwrap_or(fallback)),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        mask_view.unwrap_or(&self.white_tex.view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Upload RGBA8 pixels vers une texture GPU (lignes alignées sur 256 o).
    fn upload_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        rgba: &[u8],
        w: u32,
        h: u32,
    ) -> LayerTex {
        // Capture wgpu validation errors (otherwise silent)
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let w = w.max(1);
        let h = h.max(1);
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // bytes_per_row must be multiple of 256 (COPY_ALIGNMENT
        // wgpu). Otherwise silent validation error → texture
        // never uploaded → invisible image. Pad rows.
        const ALIGN: u32 = 256;
        let row_bytes = w * 4;
        let padded_row = row_bytes.div_ceil(ALIGN) * ALIGN;
        if padded_row == row_bytes {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        } else {
            // Copy line by line into padded buffer (once
            // per content version — amortized cost)
            let mut staged = vec![0u8; (padded_row * h) as usize];
            for r in 0..h as usize {
                let src = r * row_bytes as usize;
                let dst = r * padded_row as usize;
                staged[dst..dst + row_bytes as usize]
                    .copy_from_slice(&rgba[src..src + row_bytes as usize]);
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &staged[..],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        if let Some(err) = pollster::block_on(device.pop_error_scope()) {
            eprintln!("layer-canvas: échec upload texture {label} {err:#?}");
        }
        LayerTex {
            view,
            width: w,
            height: h,
        }
    }

    fn ensure_accum(&mut self, size: (u32, u32)) {
        let need = size != self.accum.as_ref().map_or((0, 0), |a| a.size);
        if need {
            let mk = || {
                let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("layer-canvas-accum"),
                    size: wgpu::Extent3d {
                        width: size.0.max(1),
                        height: size.1.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                tex.create_view(&wgpu::TextureViewDescriptor::default())
            };
            self.accum = Some(Accum {
                views: [mk(), mk()],
                current: std::sync::atomic::AtomicUsize::new(0),
                size,
            });
            // Force un recomposite GPU
            self.last_hash
                .store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Scratch document-size (copie d'accumulateur pour le mix final des
    /// calques d'ajustement). Recréé si la taille change (comme l'accum).
    fn ensure_scratch(&mut self, size: (u32, u32)) {
        let need = size != self.scratch.as_ref().map_or((0, 0), |s| s.size);
        if need {
            self.scratch = Some(Self::create_render_tex(
                &self.device,
                "layer-canvas-scratch",
                size.0,
                size.1,
            ));
            self.last_hash
                .store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Crée une texture possédée cible de rendu + lecture (groupes, scratch).
    fn create_render_tex(device: &wgpu::Device, label: &str, w: u32, h: u32) -> RenderTex {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        RenderTex {
            tex,
            view,
            size: (w.max(1), h.max(1)),
        }
    }

    fn prepare(
        &mut self,
        prim: &CompositePrimitive,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        echelle: f32,
    ) {
        self.echelle
            .store(echelle.to_bits(), std::sync::atomic::Ordering::Relaxed);
        let doc = (
            prim.doc_size.0.round().max(1.0) as u32,
            prim.doc_size.1.round().max(1.0) as u32,
        );
        self.ensure_accum(doc);
        self.ensure_scratch(doc);

        // Upload récursif des nouvelles versions + éviction des obsolètes
        // (groupes inclus : leurs enfants ont leurs propres textures).
        let mut vivants: Vec<u64> = Vec::new();
        let mut masques_vivants: Vec<u64> = Vec::new();
        let mut groupes_vivants: Vec<u64> = Vec::new();
        Self::collecter_cles(
            &prim.layers,
            &mut vivants,
            &mut masques_vivants,
            &mut groupes_vivants,
        );
        Self::televerser_portee(device, queue, &prim.layers, self);
        self.layer_textures.retain(|k, _| vivants.contains(k));
        self.mask_textures
            .retain(|k, _| masques_vivants.contains(k));
        if let Ok(cache) = self.group_cache.get_mut() {
            cache.retain(|k, _| groupes_vivants.contains(k));
        }

        // Patch loupe : re-téléversé seulement si le contenu change.
        match &prim.loupe {
            Some(patch) => {
                let change = self.loupe_tex.as_ref().map(|(cle, _)| *cle) != Some(patch.key);
                if change {
                    self.loupe_tex = Some((
                        patch.key,
                        Self::upload_texture(
                            device,
                            queue,
                            "layer-canvas-loupe",
                            &patch.rgba,
                            patch.side,
                            patch.side,
                        ),
                    ));
                }
            }
            None => self.loupe_tex = None,
        }
    }

    /// Clés vivantes d'une portée (pixels, masques, groupes), récursive.
    fn collecter_cles(
        layers: &[DisplayLayer],
        vivants: &mut Vec<u64>,
        masques: &mut Vec<u64>,
        groupes: &mut Vec<u64>,
    ) {
        for l in layers {
            if l.rgba.is_some() {
                vivants.push(l.key);
            }
            if let Some(m) = &l.mask {
                masques.push(m.key);
            }
            if let DisplayContent::Group(enfants) = &l.content {
                groupes.push(l.key);
                Self::collecter_cles(enfants, vivants, masques, groupes);
            }
        }
    }

    /// Téléverse pixels et masques d'une portée (récursif pour les groupes).
    /// Couverture de masque téléversée séparément, clé = identité du
    /// contenu — peindre un masque ne touche jamais la texture du calque.
    fn televerser_portee(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layers: &[DisplayLayer],
        pipe: &mut CompositePipeline,
    ) {
        for l in layers {
            if let Some(rgba) = l.rgba.as_ref() {
                // Garde anti-panic : le tampon doit couvrir la texture
                // déclarée (tampon réduit + dims logiques = combinaison
                // invalide, calque ignoré au lieu de planter le rendu).
                if !tampon_valide(rgba.len(), l.tex_width, l.tex_height) {
                    eprintln!(
                        "layer-canvas : tampon {} octets pour texture {}x{} — calque ignoré",
                        rgba.len(),
                        l.tex_width,
                        l.tex_height
                    );
                    continue;
                }
                pipe.layer_textures.entry(l.key).or_insert_with(|| {
                    Self::upload_texture(
                        device,
                        queue,
                        "layer-canvas-layer",
                        rgba,
                        l.tex_width,
                        l.tex_height,
                    )
                });
            }
            if let Some(mask) = &l.mask {
                pipe.mask_textures.entry(mask.key).or_insert_with(|| {
                    Self::upload_texture(
                        device,
                        queue,
                        "layer-canvas-mask",
                        &mask.rgba,
                        mask.width,
                        mask.height,
                    )
                });
            }
            if let DisplayContent::Group(enfants) = &l.content {
                Self::televerser_portee(device, queue, enfants, pipe);
            }
        }
    }

    /// Alloue un buffer d'uniforms FRAIS et l'initialise (voir `scene_bg`
    /// pour le pourquoi : jamais de buffer partagé entre passes).
    fn params_frais(&self, params: &Params) -> wgpu::Buffer {
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("layer-canvas-params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&buf, 0, bytemuck::bytes_of(params));
        buf
    }

    #[allow(clippy::too_many_arguments)]
    fn passe(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        etiquette: &str,
        pipeline: &wgpu::RenderPipeline,
        src: &wgpu::TextureView,
        dessus: Option<&wgpu::TextureView>,
        masque: Option<&wgpu::TextureView>,
        dst: &wgpu::TextureView,
        params: &Params,
    ) {
        let buf = self.params_frais(params);
        let groupe = self.scene_bg(src, dessus, masque, &buf);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(etiquette),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: dst,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &groupe, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Efface une vue (les textures wgpu naissent avec un contenu indéfini).
    fn effacer(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("layer-canvas-clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }

    /// Uniforms de fusion pour un calque de pixels (placement xform inclus).
    /// La vue de masque est résolue par l'appelant (voir `empiler`).
    fn params_pixel(&self, couche: &DisplayLayer, ctx: &ScopeCtx) -> Option<Params> {
        // Garde : texture téléversée (le rendu résout la vue lui-même).
        self.layer_textures.get(&couche.key)?;
        let (xform, xform_off) =
            affine_inverse(&couche.transform, couche.width as f32, couche.height as f32)?;
        let (masque_present, masque_l, masque_h) = match &couche.mask {
            Some(m) => (1, m.width, m.height),
            None => (0, 1, 1),
        };
        Some(Params {
            screen_doc: [ctx.viewport.0, ctx.viewport.1, ctx.doc.0, ctx.doc.1],
            pan_zoom: [
                ctx.pan.x,
                ctx.pan.y,
                ctx.zoom,
                couche.opacity.clamp(0.0, 1.0),
            ],
            // Dimensions LOGIQUES (plein format) : la texture téléversée
            // peut être réduite (aperçu > 2048 px), l'échantillonnage
            // [0,1] étire comme iced — jamais les dims du buffer.
            mode_sizes: [couche.blend, couche.width.max(1), couche.height.max(1), 0],
            off_sel: [0.0, 0.0, 0.0, 0.0],
            sel_size: [0.0, 0.0, 0.0, 0.0],
            mask_info: [masque_present, masque_l, masque_h, 0],
            xform,
            xform_off,
            adjust: AJUST_NEUTRE,
            blur_px: [0.0, 0.0, 0.0, 0.0],
            curseur: [0.0, 0.0, 0.0, 0.0],
            loupe: [0.0, 0.0, 0.0, 0.0],
        })
    }

    /// Fusionne `dessus` sur `src` vers `dst` (passe blend générique).
    #[allow(clippy::too_many_arguments)]
    fn fusionner(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::TextureView,
        dessus: &wgpu::TextureView,
        dessus_l: u32,
        dessus_h: u32,
        masque: Option<(&wgpu::TextureView, u32, u32, bool)>,
        dst: &wgpu::TextureView,
        opacite: f32,
        fusion: u32,
        xform: [f32; 4],
        xform_off: [f32; 4],
        ctx: &ScopeCtx,
    ) {
        let (present, ml, mh, en_doc, vue) = match masque {
            Some((v, l, h, doc)) => (1, l, h, u32::from(doc), Some(v)),
            None => (0, 1, 1, 0, None),
        };
        let params = Params {
            screen_doc: [ctx.viewport.0, ctx.viewport.1, ctx.doc.0, ctx.doc.1],
            pan_zoom: [ctx.pan.x, ctx.pan.y, ctx.zoom, opacite.clamp(0.0, 1.0)],
            mode_sizes: [fusion, dessus_l, dessus_h, 0],
            off_sel: [0.0, 0.0, 0.0, 0.0],
            sel_size: [0.0, 0.0, 0.0, 0.0],
            mask_info: [present, ml, mh, en_doc],
            xform,
            xform_off,
            adjust: AJUST_NEUTRE,
            blur_px: [0.0, 0.0, 0.0, 0.0],
            curseur: [0.0, 0.0, 0.0, 0.0],
            loupe: [0.0, 0.0, 0.0, 0.0],
        };
        self.passe(
            encoder,
            "layer-canvas-blend-pass",
            &self.blend_pipeline,
            src,
            Some(dessus),
            vue,
            dst,
            &params,
        );
    }

    /// Composite une portée en ping-pong entre `vues` (espace document).
    /// Retourne l'index de la vue contenant le résultat. Groupes et
    /// ajustements sont traités récursivement (voir `groupe_composite`).
    fn empiler(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        couches: &[DisplayLayer],
        vues: [&wgpu::TextureView; 2],
        ctx: &ScopeCtx,
    ) -> usize {
        self.effacer(encoder, vues[0]);
        let mut cur = 0;
        for couche in couches {
            match &couche.content {
                DisplayContent::Pixel => {
                    if couche.rgba.is_none() {
                        continue;
                    }
                    let Some(params) = self.params_pixel(couche, ctx) else {
                        continue;
                    };
                    let Some(tex) = self.layer_textures.get(&couche.key) else {
                        continue;
                    };
                    if tex.width == 0 || tex.height == 0 {
                        continue;
                    }
                    let masque = couche.mask.as_ref().and_then(|m| {
                        self.mask_textures
                            .get(&m.key)
                            .map(|t| (&t.view, m.width, m.height, false))
                    });
                    let dessus = tex.view.clone();
                    let src = vues[cur].clone();
                    let dst = vues[cur ^ 1].clone();
                    // `params_pixel` a déjà tout calculé ; on rejoue la
                    // passe via `fusionner`-léger : on réutilise `passe`.
                    let (present, ml, mh, en_doc, vue) = match masque {
                        Some((v, l, h, doc)) => (1, l, h, u32::from(doc), Some(v)),
                        None => (0, 1, 1, 0, None),
                    };
                    let mut p = params;
                    p.mask_info = [present, ml, mh, en_doc];
                    self.passe(
                        encoder,
                        "layer-canvas-blend-pass",
                        &self.blend_pipeline,
                        &src,
                        Some(&dessus),
                        vue,
                        &dst,
                        &p,
                    );
                    cur ^= 1;
                }
                DisplayContent::Group(enfants) => {
                    if enfants.is_empty() {
                        continue;
                    }
                    let Some(vue_groupe) = self.groupe_composite(encoder, couche, enfants, ctx)
                    else {
                        continue;
                    };
                    // Le cache est déjà en espace document : xform identité,
                    // masque du groupe échantillonné en espace document.
                    let masque = couche.mask.as_ref().and_then(|m| {
                        self.mask_textures
                            .get(&m.key)
                            .map(|t| (&t.view, m.width, m.height, true))
                    });
                    let doc_l = ctx.doc.0.round().max(1.0) as u32;
                    let doc_h = ctx.doc.1.round().max(1.0) as u32;
                    let src = vues[cur].clone();
                    let dst = vues[cur ^ 1].clone();
                    self.fusionner(
                        encoder,
                        &src,
                        &vue_groupe,
                        doc_l,
                        doc_h,
                        masque,
                        &dst,
                        couche.opacity,
                        couche.blend,
                        [1.0, 0.0, 0.0, 1.0],
                        [0.0, 0.0, 0.0, 0.0],
                        ctx,
                    );
                    cur ^= 1;
                }
                DisplayContent::Adjustment(ops) => {
                    if ops.is_empty() {
                        continue;
                    }
                    let Some(scratch) = self.scratch.as_ref() else {
                        continue;
                    };
                    // 1. Copie de l'original (passe neutre = identité).
                    let neutres = Params {
                        screen_doc: [ctx.viewport.0, ctx.viewport.1, ctx.doc.0, ctx.doc.1],
                        pan_zoom: [ctx.pan.x, ctx.pan.y, ctx.zoom, 1.0],
                        mode_sizes: [0, 0, 0, 0],
                        off_sel: [0.0, 0.0, 0.0, 0.0],
                        sel_size: [0.0, 0.0, 0.0, 0.0],
                        mask_info: [0, 1, 1, 0],
                        xform: [1.0, 0.0, 0.0, 1.0],
                        xform_off: [0.0, 0.0, 0.0, 0.0],
                        adjust: AJUST_NEUTRE,
                        blur_px: [0.0, 0.0, 0.0, 0.0],
                        curseur: [0.0, 0.0, 0.0, 0.0],
                        loupe: [0.0, 0.0, 0.0, 0.0],
                    };
                    let original = vues[cur].clone();
                    let copie = scratch.view.clone();
                    self.passe(
                        encoder,
                        "layer-canvas-adjust-copy",
                        &self.adjust_pipeline,
                        &original,
                        None,
                        None,
                        &copie,
                        &neutres,
                    );
                    // 2. Chaîne d'opérations en ping-pong sur la paire.
                    for op in ops {
                        let (ajust, flou) = adjust_uniforms(op);
                        if flou > 0.0 {
                            for axe in [0.0, 1.0] {
                                let mut p = neutres;
                                p.blur_px = [flou, axe, 0.0, 0.0];
                                let src = vues[cur].clone();
                                let dst = vues[cur ^ 1].clone();
                                self.passe(
                                    encoder,
                                    "layer-canvas-blur-pass",
                                    &self.blur_pipeline,
                                    &src,
                                    None,
                                    None,
                                    &dst,
                                    &p,
                                );
                                cur ^= 1;
                            }
                        } else if ajust != AJUST_NEUTRE {
                            let mut p = neutres;
                            p.adjust = ajust;
                            let src = vues[cur].clone();
                            let dst = vues[cur ^ 1].clone();
                            self.passe(
                                encoder,
                                "layer-canvas-adjust-pass",
                                &self.adjust_pipeline,
                                &src,
                                None,
                                None,
                                &dst,
                                &p,
                            );
                            cur ^= 1;
                        }
                    }
                    // 3. Mix : base = original (scratch), dessus = filtré,
                    // poids = opacité (sémantique `apply_adjustment` CPU).
                    let doc_l = ctx.doc.0.round().max(1.0) as u32;
                    let doc_h = ctx.doc.1.round().max(1.0) as u32;
                    let filtre = vues[cur].clone();
                    let dst = vues[cur ^ 1].clone();
                    self.fusionner(
                        encoder,
                        &copie,
                        &filtre,
                        doc_l,
                        doc_h,
                        None,
                        &dst,
                        couche.opacity,
                        0,
                        [1.0, 0.0, 0.0, 1.0],
                        [0.0, 0.0, 0.0, 0.0],
                        ctx,
                    );
                    cur ^= 1;
                }
            }
        }
        cur
    }

    /// Composite d'un groupe dans sa texture dédiée (mise en cache par
    /// signature récursive du sous-arbre). Seul un groupe MODIFIÉ est
    /// recomposé — même principe que `Renderer::appearance` côté moteur.
    /// Retourne la vue contenant le composite du groupe.
    fn groupe_composite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        groupe: &DisplayLayer,
        enfants: &[DisplayLayer],
        ctx: &ScopeCtx,
    ) -> Option<wgpu::TextureView> {
        let taille = (
            ctx.doc.0.round().max(1.0) as u32,
            ctx.doc.1.round().max(1.0) as u32,
        );
        let mut h: u64 = 0xcbf29ce484222325;
        nourrir_couche(&mut h, enfants, ctx.doc);
        let signature = h;
        // Cache hit : même clé, même signature, même taille.
        if let Ok(cache) = self.group_cache.try_lock()
            && let Some(hit) = cache.get(&groupe.key)
            && hit.hash == signature
            && hit.size == taille
        {
            return Some(hit.views[hit.current.get()].view.clone());
        }
        // Miss : paire dédiée (créée ou redimensionnée hors verrou tenu),
        // puis composition récursive des enfants dedans.
        let (v0, v1) = {
            let mut cache = self.group_cache.try_lock().ok()?;
            let entree = cache.entry(groupe.key).or_insert_with(|| CachedGroup {
                hash: u64::MAX,
                size: taille,
                views: [
                    Self::create_render_tex(&self.device, "layer-canvas-group", taille.0, taille.1),
                    Self::create_render_tex(&self.device, "layer-canvas-group", taille.0, taille.1),
                ],
                current: std::cell::Cell::new(0),
            });
            if entree.size != taille {
                *entree = CachedGroup {
                    hash: u64::MAX,
                    size: taille,
                    views: [
                        Self::create_render_tex(
                            &self.device,
                            "layer-canvas-group",
                            taille.0,
                            taille.1,
                        ),
                        Self::create_render_tex(
                            &self.device,
                            "layer-canvas-group",
                            taille.0,
                            taille.1,
                        ),
                    ],
                    current: std::cell::Cell::new(0),
                };
            }
            (entree.views[0].view.clone(), entree.views[1].view.clone())
        };
        let vues = [&v0, &v1];
        let final_idx = self.empiler(encoder, enfants, vues, ctx);
        if let Ok(mut cache) = self.group_cache.try_lock()
            && let Some(entree) = cache.get_mut(&groupe.key)
        {
            entree.hash = signature;
            entree.current.set(final_idx);
            return Some(entree.views[final_idx].view.clone());
        }
        None
    }

    fn render(
        &self,
        prim: &CompositePrimitive,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        use std::sync::atomic::Ordering;
        let Some(accum) = self.accum.as_ref() else {
            return;
        };

        // --- BLEND PASSES (offscreen, ping-pong) ---
        // Recomposite only if stack changed since last frame
        let hash = config_hash(&prim.layers, prim.doc_size);
        if hash != self.last_hash.load(Ordering::Relaxed) {
            // Diagnostic temporaire (bogues d'affichage) : table de la pile
            // vue par le GPU — borné aux recomposites, silencieux sinon.
            eprintln!(
                "layer-canvas : recomposite doc={}x{} vue={}x{} couches={}",
                prim.doc_size.0,
                prim.doc_size.1,
                prim.viewport.0,
                prim.viewport.1,
                prim.layers.len(),
            );
            for (i, couche) in prim.layers.iter().enumerate() {
                let sorte = match &couche.content {
                    DisplayContent::Pixel => "pixel",
                    DisplayContent::Group(e) => {
                        eprintln!("  couche {i} : groupe {} enfants", e.len());
                        "groupe"
                    }
                    DisplayContent::Adjustment(o) => {
                        eprintln!("  couche {i} : ajustement {} ops", o.len());
                        "ajustement"
                    }
                };
                if matches!(couche.content, DisplayContent::Pixel) {
                    let t = &couche.transform;
                    eprintln!(
                        "  couche {i} : {sorte} cle={} log={}x{} tex={}x{} octets={} op={} blend={} off=({},{}) echelle=({},{}) rot={} skew=({},{}) masque={}",
                        couche.key,
                        couche.width,
                        couche.height,
                        couche.tex_width,
                        couche.tex_height,
                        couche.rgba.as_ref().map(|r| r.len()).unwrap_or(0),
                        couche.opacity,
                        couche.blend,
                        t.offset_x,
                        t.offset_y,
                        t.scale_x,
                        t.scale_y,
                        t.rotation_deg,
                        t.skew_x,
                        t.skew_y,
                        couche.mask.is_some(),
                    );
                }
            }
            let ctx = ScopeCtx {
                doc: prim.doc_size,
                viewport: prim.viewport,
                pan: prim.pan,
                zoom: prim.zoom,
            };
            let vues = [&accum.views[0], &accum.views[1]];
            let final_idx = self.empiler(encoder, &prim.layers, vues, &ctx);
            accum.current.store(final_idx, Ordering::Relaxed);
            self.last_hash.store(hash, Ordering::Relaxed);
        }

        // --- PRESENTATION PASS (screen) ---
        let Some(acc) = self.accum.as_ref() else {
            return;
        };
        let final_view = acc.views[acc.current.load(Ordering::Relaxed)].clone();
        let vue_loupe = prim.loupe.as_ref().and_then(|patch| {
            self.loupe_tex
                .as_ref()
                .filter(|(cle, _)| *cle == patch.key)
                .map(|(_, tex)| &tex.view)
        });
        let params = Params::neutres(
            prim,
            f32::from_bits(self.echelle.load(std::sync::atomic::Ordering::Relaxed)),
        );
        let fond_params = self.params_frais(&params);
        let fond = self.scene_bg(&final_view, vue_loupe, None, &fond_params);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("layer-canvas-present-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.055,
                        g: 0.055,
                        b: 0.055,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.present_pipeline);
        pass.set_bind_group(0, &fond, &[]);
        // Ciseaux écran : le triangle couvre tout le widget en clip-space,
        // mais la cible est la surface ENTIÈRE — sans ciseaux le canvas
        // repeindrait par-dessus les panneaux voisins.
        pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width.max(1),
            clip_bounds.height.max(1),
        );
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn layer(key: u64, mask_key: Option<u64>) -> DisplayLayer {
        DisplayLayer {
            key,
            rgba: Some(Arc::<[u8]>::from(vec![0u8; 16])),
            width: 2,
            height: 2,
            tex_width: 2,
            tex_height: 2,
            opacity: 1.0,
            blend: 0,
            transform: Transform2D::default(),
            mask: mask_key.map(|key| DisplayMask {
                key,
                rgba: Arc::<[u8]>::from(vec![255u8; 16]),
                width: 2,
                height: 2,
            }),
            content: DisplayContent::Pixel,
        }
    }

    fn groupe(key: u64, enfants: Vec<DisplayLayer>) -> DisplayLayer {
        DisplayLayer {
            key,
            rgba: None,
            width: 0,
            height: 0,
            tex_width: 0,
            tex_height: 0,
            opacity: 1.0,
            blend: 0,
            transform: Transform2D::default(),
            mask: None,
            content: DisplayContent::Group(enfants),
        }
    }

    fn ajustement(key: u64, ops: Vec<AdjustmentOp>) -> DisplayLayer {
        DisplayLayer {
            key,
            rgba: None,
            width: 0,
            height: 0,
            tex_width: 0,
            tex_height: 0,
            opacity: 0.8,
            blend: 0,
            transform: Transform2D::default(),
            mask: None,
            content: DisplayContent::Adjustment(ops),
        }
    }

    #[test]
    fn hash_change_avec_masque() {
        // Éditer un masque doit invalider le composite GPU, sans toucher à
        // la texture du calque : seule la clé de couverture change.
        let sans = config_hash(&[layer(1, None)], (8.0, 8.0));
        let avec = config_hash(&[layer(1, Some(2))], (8.0, 8.0));
        assert_ne!(sans, avec);
        assert_eq!(avec, config_hash(&[layer(1, Some(2))], (8.0, 8.0)));
    }

    #[test]
    fn hash_change_avec_skew() {
        // Inclinaison ⇒ placement différent ⇒ recomposite (ancien angle
        // mort du chemin GPU, qui ignorait le skew).
        let droit = layer(1, None);
        let mut incline = layer(1, None);
        incline.transform.skew_x = 15.0;
        assert_ne!(
            config_hash(&[droit], (8.0, 8.0)),
            config_hash(&[incline], (8.0, 8.0)),
        );
    }

    #[test]
    fn hash_change_avec_groupe_et_ajustement() {
        // Groupe : toucher un enfant invalide ; ajustement : toucher la
        // chaîne invalide. Stabilité sinon (cache par signature).
        let g1 = groupe(7, vec![layer(1, None)]);
        let g2 = groupe(7, vec![layer(2, None)]);
        assert_ne!(
            config_hash(std::slice::from_ref(&g1), (8.0, 8.0)),
            config_hash(&[g2], (8.0, 8.0)),
        );
        assert_eq!(
            config_hash(std::slice::from_ref(&g1), (8.0, 8.0)),
            config_hash(&[g1], (8.0, 8.0)),
        );
        let a1 = ajustement(
            9,
            vec![AdjustmentOp::BrightnessContrast {
                brightness: 10.0,
                contrast: 5.0,
            }],
        );
        let a2 = ajustement(
            9,
            vec![AdjustmentOp::BrightnessContrast {
                brightness: 11.0,
                contrast: 5.0,
            }],
        );
        assert_ne!(
            config_hash(std::slice::from_ref(&a1), (8.0, 8.0)),
            config_hash(&[a2], (8.0, 8.0)),
        );
        assert_eq!(
            config_hash(std::slice::from_ref(&a1), (8.0, 8.0)),
            config_hash(&[a1], (8.0, 8.0)),
        );
    }

    #[test]
    fn shader_wgsl_valide() {
        // Garde-fou permanent : le fichier fut un temps corrompu (code
        // Rust autour du WGSL) sans qu'aucun test ne le voie — le parseur
        // est exactement celui du wgpu d'iced (naga épinglé en dev-dep).
        let module = naga::front::wgsl::parse_str(SHADER).expect("layer_blend.wgsl doit parser");
        let mut validateur = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        validateur.validate(&module).expect("module WGSL invalide");
        for attendu in ["vs_main", "fs_blend", "fs_adjust", "fs_blur", "fs_present"] {
            assert!(
                module.entry_points.iter().any(|e| e.name == attendu),
                "entry point manquant : {attendu}",
            );
        }
    }

    #[test]
    fn neutres_echelle_hidpi() {
        // HiDPI ×2 : tout l'espace ÉCRAN double (viewport, pan, zoom,
        // sélection), le document reste inchangé.
        let prim = CompositePrimitive {
            layers: Vec::new(),
            doc_size: (100.0, 100.0),
            has_doc: true,
            pan: Vector::new(10.0, 20.0),
            zoom: 1.5,
            viewport: (200.0, 100.0),
            selection: Some(Rectangle::new(Point::new(1.0, 2.0), Size::new(3.0, 4.0))),
            curseur: None,
            loupe: None,
        };
        let p1 = Params::neutres(&prim, 1.0);
        let p2 = Params::neutres(&prim, 2.0);
        assert_eq!(p1.screen_doc, [200.0, 100.0, 100.0, 100.0]);
        assert_eq!(p2.screen_doc, [400.0, 200.0, 100.0, 100.0]);
        assert_eq!(p1.pan_zoom[..3], [10.0, 20.0, 1.5]);
        assert_eq!(p2.pan_zoom[..3], [20.0, 40.0, 3.0]);
        assert_eq!(p2.off_sel, [2.0, 4.0, 0.0, 0.0]);
        assert_eq!(p2.sel_size, [6.0, 8.0, 0.0, 0.0]);
    }

    #[test]
    fn tampon_valide_garde() {
        // Cas nominal : tampon exact.
        assert!(tampon_valide(2 * 2 * 4, 2, 2));
        // Aperçu réduit + dims logiques plein format : INVALIDE (c'est ce
        // qui plantait l'upload des photos > 2048 px).
        assert!(!tampon_valide(8 * 8 * 4, 100, 100));
        // Tampon vide ou dims nulles : invalide, jamais de panic.
        assert!(!tampon_valide(0, 2, 2));
        assert!(!tampon_valide(0, 0, 0));
    }

    fn toile_picking(cibles: Vec<HitLayer>) -> LayerCanvas<String> {
        LayerCanvas::new(
            Some((100.0, 100.0)),
            std::rc::Rc::new(|e: ImageCanvasEvent| format!("{e:?}")),
        )
        .with_hit_layers(cibles)
    }

    fn cible(id: u128, ox: f32, oy: f32) -> HitLayer {
        HitLayer {
            id: Some(Uuid::from_u128(id)),
            transform: Transform2D {
                offset_x: ox,
                offset_y: oy,
                ..Transform2D::default()
            },
            width: 10.0,
            height: 10.0,
        }
    }

    #[test]
    fn pick_dessus_dabord() {
        // Haut de pile en dernier : le point (6,6) touche les deux, le
        // second gagne ; (2,2) ne touche que le premier ; (50,50) aucun.
        let toile = toile_picking(vec![cible(1, 0.0, 0.0), cible(2, 5.0, 5.0)]);
        assert_eq!(toile.pick((6.0, 6.0)), Some(Uuid::from_u128(2)));
        assert_eq!(toile.pick((2.0, 2.0)), Some(Uuid::from_u128(1)));
        assert_eq!(toile.pick((50.0, 50.0)), None);
    }

    #[test]
    fn pick_incline_suit_les_coins() {
        // Calque incliné : le quad suit le skew (le point hors axe mais
        // dans le parallélogramme est touché).
        let toile = toile_picking(vec![HitLayer {
            id: Some(Uuid::from_u128(7)),
            transform: Transform2D {
                skew_x: 45.0,
                ..Transform2D::default()
            },
            width: 10.0,
            height: 10.0,
        }]);
        assert_eq!(toile.pick((12.0, 8.0)), Some(Uuid::from_u128(7)));
        assert_eq!(toile.pick((0.0, 9.0)), None);
    }

    #[test]
    fn affine_inverse_aller_retour() {
        // L'inverse CPU doit annuler EXACTEMENT `local_to_doc` (même ordre
        // scale → skew → rotation, mêmes `tan()`), y compris avec skew.
        let cas = [
            Transform2D::default(),
            Transform2D {
                offset_x: 12.0,
                offset_y: -7.0,
                ..Transform2D::default()
            },
            Transform2D {
                skew_x: 15.0,
                skew_y: -10.0,
                ..Transform2D::default()
            },
            Transform2D {
                scale_x: 1.5,
                scale_y: 0.75,
                rotation_deg: 30.0,
                skew_x: 12.0,
                skew_y: 5.0,
                offset_x: -3.0,
                offset_y: 9.0,
            },
        ];
        for t in cas {
            let (w, h) = (100.0, 60.0);
            let Some((n, o)) = affine_inverse(&t, w, h) else {
                panic!("cas inversible déclaré dégénéré");
            };
            for (x, y) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h), (33.0, 21.0)] {
                let (dx, dy) = t.local_to_doc(w, h, x, y);
                let ex = dx - o[0];
                let ey = dy - o[1];
                let rx = n[0] * ex + n[1] * ey + o[2];
                let ry = n[2] * ex + n[3] * ey + o[3];
                assert!(
                    (rx - x).abs() < 1e-3 && (ry - y).abs() < 1e-3,
                    "aller-retour hors tolérance pour {t:?} en ({x}, {y}) : ({rx}, {ry})",
                );
            }
        }
    }

    #[test]
    fn affine_inverse_degenere() {
        // Échelle nulle ⇒ pas d'inverse : le calque est ignoré au lieu
        // d'échantillonner n'importe quoi.
        let t = Transform2D {
            scale_x: 0.0,
            ..Transform2D::default()
        };
        assert!(affine_inverse(&t, 10.0, 10.0).is_none());
    }

    #[test]
    fn uniforms_ajustement_conversions() {
        // Mêmes conversions CPU → GPU que `GpuContext` : luminosité ×0.01,
        // contraste 1+c/100 (négatif) ou 1+c/50 (positif).
        let (u, flou) = adjust_uniforms(&AdjustmentOp::BrightnessContrast {
            brightness: 20.0,
            contrast: -50.0,
        });
        assert!((u[0] - 0.2).abs() < 1e-6);
        assert!((u[1] - 0.5).abs() < 1e-6);
        assert_eq!(u[2], 1.0);
        assert_eq!(flou, 0.0);
        let (u, _) = adjust_uniforms(&AdjustmentOp::BrightnessContrast {
            brightness: 0.0,
            contrast: 50.0,
        });
        assert!((u[1] - 2.0).abs() < 1e-6);
        let (u, _) = adjust_uniforms(&AdjustmentOp::Saturation { value: 1.5 });
        assert_eq!(u, [0.0, 1.0, 1.5, 0.0]);
        let (u, flou) = adjust_uniforms(&AdjustmentOp::Blur { radius: 5.0 });
        assert_eq!(u, AJUST_NEUTRE);
        assert_eq!(flou, 5.0);
        // Flou négligeable ⇒ neutre (même garde que le CPU, radius <= 0.1).
        let (u, flou) = adjust_uniforms(&AdjustmentOp::Blur { radius: 0.05 });
        assert_eq!(u, AJUST_NEUTRE);
        assert_eq!(flou, 0.0);
    }
}
