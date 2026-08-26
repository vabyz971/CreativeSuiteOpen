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

//! Canvas image interactif avec pan/zoom - utilise iced::widget::canvas native
//! Inspiré de examples/bezier_tool et game_of_life (pan/zoom infini)

use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::widget::image;
use iced::{Point, Rectangle, Size, Theme, Vector};

use crate::theme::colors;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasTool {
    Hand,
    Zoom,
    Select,
    Move,
    /// Pinceau : peint sur le calque sélectionné
    Brush,
    /// Gomme : efface (réduit l'alpha) sur le calque sélectionné
    Eraser,
}

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
    /// Taille du viewport du canvas (pour calculer "ajuster à l'image")
    Viewport(Size),
    /// Début du déplacement du calque sélectionné
    MoveLayerStart,
    /// Déplacement du calque sélectionné (dx/dy en pixels image depuis le début du drag)
    MoveLayer {
        dx: f32,
        dy: f32,
    },
    /// Fin du déplacement — valide le décalage
    MoveLayerEnd,
    /// Début d'un trait (coordonnées document) — pinceau ou gomme
    BrushStart {
        x: f32,
        y: f32,
        /// true = gomme (destination-out), false = pinceau
        erase: bool,
    },
    /// Fin du trait — polyligne (commit pixels) + texture d'aperçu figée
    /// jusqu'à l'application effective des pixels.
    BrushEnd {
        points: Vec<(f32, f32)>,
        tex: Option<StrokeTex>,
        /// true = gomme (destination-out), false = pinceau
        erase: bool,
    },
}

/// Style du pinceau/gomme pour l'aperçu live (espace document).
#[derive(Clone, Copy, Debug)]
pub struct BrushStyle {
    /// Couleur RGB 0-255 (ignorée pour la gomme : aperçu en anneau)
    pub color: [u8; 3],
    /// Rayon en pixels DOCUMENT (= taille / 2)
    pub radius: f32,
    pub opacity: f32,
    /// true = gomme → l'aperçu est un ANNEAU (empreinte) au lieu du disque
    pub erase: bool,
}

/// Aperçu d'un trait — TUILES 512×512 en coordonnées document.
///
/// Pourquoi une texture et pas des cercles vectoriels ? Le moteur iced
/// impose par couche l'ordre de rendu figé quads -> meshes -> images :
/// la géométrie vectorielle passerait SOUS les textures des calques.
/// Une image, elle, est dessinée après les images de calques.
///
/// Pourquoi des tuiles ? L'atlas de textures iced_wgpu limite une image à
/// 2048×2048 (`atlas::MAX_SIZE`) : un grand tracé dans une texture unique
/// dépassait la limite et DISPARAISSAIT de l'aperçu. Chaque tuile reste
/// loin sous la limite, quelle que soit l'étendue du trait. Les tuiles
/// étant alignées sur une grille entière, aucun ré-échantillonnage n'a
/// lieu lors de l'extension du trait — l'aperçu ne « marche » plus.
#[derive(Clone, Debug, Default)]
pub struct StrokeTex {
    tiles: Vec<Tile>,
}

/// Côté d'une tuile d'aperçu (pixels document).
const TILE: u32 = 512;

#[derive(Clone, Debug)]
struct Tile {
    /// Coordonnées tuile sur la grille (× TILE = origine document)
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

    /// Estampe un disque (couleur) ou un anneau (gomme) centré en (cx, cy)
    /// document — écrit dans toutes les tuiles chevauchées.
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

    /// Estampe des disques/anneaux le long du segment (pas ~ rayon/3).
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

/// Un calque affichable sur le canvas — dessiné à SA position monde.
/// Déplacer = changer offset → simple redraw, zéro recomposite.
/// L'opacité/rotation/scale sont appliqués AU DRAW (GPU) — zéro
/// régénération de pixels.
pub struct CanvasLayer {
    pub handle: image::Handle,
    pub width: f32,
    pub height: f32,
    /// Position monde (0,0) = coin haut-gauche du document
    pub offset_x: f32,
    pub offset_y: f32,
    /// Opacité 0..1 appliquée au draw (sans toucher aux pixels)
    pub opacity: f32,
    /// Rotation en degrés (autour du centre du calque)
    pub rotation_deg: f32,
    /// Échelle uniforme (1.0 = 100 %)
    pub scale: f32,
}

pub struct ImageCanvas {
    pub layers: Vec<CanvasLayer>,
    /// Dimensions du document (repère au sol, dessiné dans l'espace monde)
    pub doc_size: Option<Size>,
    pub pan: Vector,
    pub zoom: f32,
    pub tool: CanvasTool,
    pub selection: Option<Rectangle>,
    /// Style du pinceau (rastérisation de l'aperçu live)
    pub brush: BrushStyle,
    /// Aperçu figé pendant le commit asynchrone (après relâchement)
    pub pending_preview: Option<StrokeTex>,
}

impl ImageCanvas {
    pub fn new(doc_size: Option<Size>, pan: Vector, zoom: f32) -> Self {
        Self {
            layers: Vec::new(),
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
            pending_preview: None,
        }
    }
    pub fn with_brush(mut self, brush: BrushStyle) -> Self {
        self.brush = brush;
        self
    }
    pub fn with_pending_preview(mut self, preview: Option<StrokeTex>) -> Self {
        self.pending_preview = preview;
        self
    }

    /// Convertit une position écran canvas en coordonnées DOCUMENT
    /// (inverse exact du transform de draw : centre + pan + zoom).
    fn screen_to_doc(&self, p: Point, bounds: Rectangle) -> Point {
        let center = Point::new(
            bounds.width / 2.0 + self.pan.x,
            bounds.height / 2.0 + self.pan.y,
        );
        let (hw, hh) = self
            .doc_size
            .map(|s| (s.width / 2.0, s.height / 2.0))
            .unwrap_or((0.0, 0.0));
        Point::new(
            (p.x - center.x) / self.zoom + hw,
            (p.y - center.y) / self.zoom + hh,
        )
    }
    pub fn with_layers(mut self, layers: Vec<CanvasLayer>) -> Self {
        self.layers = layers;
        self
    }
    pub fn with_tool(mut self, tool: CanvasTool) -> Self {
        self.tool = tool;
        self
    }
    pub fn with_selection(mut self, sel: Option<Rectangle>) -> Self {
        self.selection = sel;
        self
    }
}

#[derive(Default)]
pub struct State {
    pub dragging: Option<(Point, Vector)>,
    /// Points du trait en cours (coords document) — stockés dans le canvas
    /// pour un aperçu sans aller-retour vers l'application (zéro latence,
    /// redraw explicite à chaque déplacement).
    pub stroke: Vec<(f32, f32)>,
    /// Version rastérisée du trait (texture doc-space, au-dessus des calques)
    pub stroke_tex: Option<StrokeTex>,
    pub selecting: Option<(Point, Point)>, // start, current
    /// Drag scrubby zoom — ancre écran + zoom/pan de départ
    pub zoom_dragging: Option<(Point, f32, Vector)>,
    /// Position écran du curseur pour aperçu taille d'outil
    pub cursor_pos: Option<Point>,
    /// Modificateurs clavier courants (Alt = zoom inversé avec l'outil loupe)
    pub modifiers: iced::keyboard::Modifiers,
    /// Dernière taille de viewport publiée (évite spam d'événements)
    pub prev_bounds: Option<Size>,
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
        // Redraw demandé à chaque frame : on en profite pour publier le viewport
        if let canvas::Event::Window(iced::window::Event::RedrawRequested(_)) = event {
            if state.prev_bounds != Some(bounds.size()) {
                state.prev_bounds = Some(bounds.size());
                return Some(canvas::Action::publish(ImageCanvasEvent::Viewport(
                    bounds.size(),
                )));
            }
            return None;
        }

        // Suivi des modificateurs clavier (avant le early-return sur le curseur)
        if let canvas::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) = event {
            state.modifiers = *m;
            return None;
        }

        // Relâchement : DOIT être traité même hors des bornes du widget
        // (la souris est capturée pendant un drag) — sinon l'état de drag
        // reste armé et les événements de déplacement continuent d'arriver.
        if let canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
            if let Some((anchor, start_zoom, start_pan)) = state.zoom_dragging.take() {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return Some(canvas::Action::capture());
                };
                let dx = cursor_pos.x - anchor.x;
                let dy = cursor_pos.y - anchor.y;
                let drag_dist = (dx * dx + dy * dy).sqrt();
                // Clic sans déplacement significatif = zoom ponctuel sur l'ancre
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
                    // Relâché hors du canvas : annule la sélection
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
                if self.tool == CanvasTool::Move {
                    return Some(
                        canvas::Action::publish(ImageCanvasEvent::MoveLayerEnd).and_capture(),
                    );
                }
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
                // Outils
                match self.tool {
                    CanvasTool::Hand => {
                        state.dragging = Some((cursor_pos, self.pan));
                        Some(canvas::Action::capture())
                    }
                    CanvasTool::Move => {
                        state.dragging = Some((cursor_pos, self.pan));
                        Some(
                            canvas::Action::publish(ImageCanvasEvent::MoveLayerStart).and_capture(),
                        )
                    }
                    CanvasTool::Brush | CanvasTool::Eraser => {
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
                        // Début scrubby zoom — ancre = point de clic
                        state.zoom_dragging = Some((cursor_pos, self.zoom, self.pan));
                        Some(canvas::Action::capture())
                    }
                    CanvasTool::Select => {
                        state.selecting = Some((cursor_pos, cursor_pos));
                        Some(canvas::Action::capture())
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // Met à jour aperçu taille outil et scrubby zoom
                state.cursor_pos = Some(cursor_pos);
                if let Some((anchor, start_zoom, start_pan)) = state.zoom_dragging {
                    // Scrubby zoom : vertical = avance/recule, ancré sur le point de clic
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
                    if self.tool == CanvasTool::Hand {
                        let delta = Vector::new(cursor_pos.x - start.x, cursor_pos.y - start.y);
                        let new_pan = Vector::new(orig_pan.x + delta.x, orig_pan.y + delta.y);
                        return Some(canvas::Action::publish(ImageCanvasEvent::Pan(new_pan)));
                    } else if self.tool == CanvasTool::Move {
                        // Delta écran BRUT : l'aperçu suit le curseur 1:1,
                        // la conversion en pixels image se fait une seule fois au commit.
                        let dx = cursor_pos.x - start.x;
                        let dy = cursor_pos.y - start.y;
                        return Some(canvas::Action::publish(ImageCanvasEvent::MoveLayer {
                            dx,
                            dy,
                        }));
                    } else if self.tool == CanvasTool::Brush || self.tool == CanvasTool::Eraser {
                        let doc = self.screen_to_doc(cursor_pos, bounds);
                        let last = *state.stroke.last().unwrap_or(&(doc.x, doc.y));
                        let dist = ((doc.x - last.0).powi(2) + (doc.y - last.1).powi(2)).sqrt();
                        // Échantillonnage : un point tous les ~1/3 de rayon
                        if dist >= (self.brush.radius * 0.35).max(1.0) {
                            state.stroke.push((doc.x, doc.y));
                            rasterize_segment(
                                &mut state.stroke_tex,
                                last,
                                (doc.x, doc.y),
                                &self.brush,
                            );
                        }
                        // Aperçu purement local : redraw sans aller-retour app
                        return Some(canvas::Action::request_redraw().and_capture());
                    }
                }
                // Survol pinceau/gomme : redessine pour déplacer le cercle d'aperçu
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
                // Alt enfoncé + outil loupe : inverse le sens du zoom
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

        // Fond uni — damier retiré (causait ralentissements au dézoom et bugs au resize)
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), colors::BG_APP);

        let center = Point::new(
            bounds.width / 2.0 + self.pan.x,
            bounds.height / 2.0 + self.pan.y,
        );

        // Chaque calque dessiné à SA position monde — le drag ne change
        // qu'un offset, aucun recomposite (modèle Affinity).
        // Convention : offset (0,0) = coin haut-gauche DU DOCUMENT
        // (même sémantique que le composite CPU et le panneau Propriétés).
        let (doc_half_w, doc_half_h) = self
            .doc_size
            .map(|s| (s.width / 2.0, s.height / 2.0))
            .unwrap_or((0.0, 0.0));
        for l in &self.layers {
            // Rotation appliquée autour du centre du rect par iced —
            // le rect garde la taille d'origine (w_s×h_s), les coins
            // tournés dépassent naturellement sans être rognés.
            let w = l.width * l.scale * self.zoom;
            let h = l.height * l.scale * self.zoom;
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

        // Aperçu pinceau : TEXTURES (une par tuile 512×512) dessinées après
        // les images de calques. Ordre figé du moteur iced par couche :
        // quads -> meshes -> images ; une géométrie vectorielle passerait
        // SOUS les calques. Priorité au trait live (drag) ; sinon l'aperçu
        // figé du commit.
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

        // Repère document dessiné DANS l'espace monde → insensible au zoom,
        // parfaitement synchrone avec pan/zoom (l'overlay widget se déformait).
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
            // Pas d'image : texte centré
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

        // Grille supprimée — fond uni uniquement

        // Sélection rect (outil Select/Zoom) - comme dans l'exemple Bézier Pending
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

        // Aperçu taille d'outil — IMAGE au-dessus des calques (vectoriel passerait dessous)
        if matches!(self.tool, CanvasTool::Brush | CanvasTool::Eraser)
            && let Some(pos) = state.cursor_pos
            && bounds.contains(pos)
        {
            let r = (self.brush.radius * self.zoom).max(2.0);
            // Génère une petite texture RGBA avec cercle gris — dessinée en IMAGE
            // pour passer au-dessus des calques (ordre iced: quads->meshes->images)
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
                    // Liseré blanc/noir adouci via alpha déjà — le gris reste lisible
                }
            }
            // Point central pour pinceau
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
            // Fond semi-transparent derrière label pour lisibilité sur blanc/noir
            let label_bg = Rectangle::new(
                Point::new(label_pos.x - 22.0, label_pos.y - 7.0),
                Size::new(44.0, 14.0),
            );
            frame.fill_rectangle(
                label_bg.position(),
                label_bg.size(),
                iced::Color::from_rgba(0.0, 0.0, 0.0, 0.55),
            );
            frame.fill_text(iced::widget::canvas::Text {
                content: label,
                position: label_pos,
                color: iced::Color::from_rgba(0.95, 0.95, 0.95, 1.0),
                size: iced::Pixels(10.0),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..Default::default()
            });
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging.is_some() {
            return mouse::Interaction::Grabbing;
        }
        if state.selecting.is_some() {
            return mouse::Interaction::Crosshair;
        }
        if cursor.is_over(bounds) {
            return match self.tool {
                CanvasTool::Hand => mouse::Interaction::Grab,
                CanvasTool::Move => mouse::Interaction::Move,
                CanvasTool::Brush | CanvasTool::Eraser => mouse::Interaction::Crosshair,
                CanvasTool::Zoom => mouse::Interaction::ZoomIn,
                CanvasTool::Select => mouse::Interaction::Crosshair,
            };
        }
        mouse::Interaction::default()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn view_with_tool<'a>(
    doc_size: Option<Size>,
    pan: Vector,
    zoom: f32,
    tool: CanvasTool,
    selection: Option<Rectangle>,
    layers: Vec<CanvasLayer>,
    brush: BrushStyle,
    pending_preview: Option<StrokeTex>,
) -> iced::Element<'a, ImageCanvasEvent> {
    let program = ImageCanvas::new(doc_size, pan, zoom)
        .with_layers(layers)
        .with_tool(tool)
        .with_selection(selection)
        .with_brush(brush)
        .with_pending_preview(pending_preview);
    iced::widget::canvas(program)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}

// ---------------------------------------------------------------------------
// Rastérisation de l'aperçu du trait (par tuile 512×512)
// ---------------------------------------------------------------------------

/// Rastérise le segment `from -> to` (coordonnées document) dans l'aperçu.
/// Les tuiles manquantes sont créées à la volée ; les existantes ne sont
/// JAMAIS déplacées (grille entière) → zéro dérive de l'aperçu.
fn rasterize_segment(
    tex: &mut Option<StrokeTex>,
    from: (f32, f32),
    to: (f32, f32),
    brush: &BrushStyle,
) {
    let t = tex.get_or_insert_with(StrokeTex::default);
    t.stamp_segment(from, to, brush);
}

/// Disque avec bord adouci sur 1 px ; alpha final = couverture x opacité
fn stamp_circle(t: &mut Tile, cx: f32, cy: f32, r: f32, col: [u8; 3], opacity: f32) {
    let w = TILE as i64;
    let h = TILE as i64;
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
            // Fond transparent : source-over == MAX ; évite l'assombrissement
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

/// Anneau blanc semi-transparent : empreinte visuelle de la gomme.
/// L'intérieur reste TRANSPARENT — on montre OÙ l'effacement aura lieu,
/// pas une couleur de peinture. Blanc pour rester lisible sur tout fond.
fn stamp_ring(t: &mut Tile, cx: f32, cy: f32, r: f32, thickness: f32, col: [u8; 3], opacity: f32) {
    const RING_ALPHA: f32 = 0.85;
    let inner = (r - thickness).max(0.0);
    let outer = r + 1.0; // bord adouci externe 1 px
    let w = TILE as i64;
    let h = TILE as i64;
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
            // Bandeau [inner, r] plein, adouci sur 1 px de chaque côté ;
            // l'INTÉRIEUR (d < inner-1) reste transparent — l'anneau montre
            // l'empreinte de la gomme, pas une peinture.
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
        // Disque centré sur la frontière x = 512
        tex.stamp_disc(512.0, 100.0, 20.0, false, [255, 0, 0], 1.0);
        assert_eq!(tex.tiles.len(), 2, "tuiles (0,0) et (1,0) touchées");

        let right = &tex.tiles.iter().find(|t| t.tx == 1).expect("tuile droite");
        // Centre du disque : local (0, 100) dans la tuile de droite
        assert_eq!(alpha_at(right, 0, 100), 255);
        let left = &tex.tiles.iter().find(|t| t.tx == 0).expect("tuile gauche");
        // Bord gauche du disque : local (511, 100) dans la tuile de gauche
        assert!(alpha_at(left, 511, 100) > 0);
    }

    #[test]
    fn trait_transfrontalier_sans_limite_de_taille() {
        // Tracé de (0,0) à (3000, 3000) : dépasse largement l'ancienne
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
        // La diagonale traverse les tuiles (0,0)…(5,5) + voisines touchées
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
        // Point de départ et d'arrivée bien estampés
        let first = &tex.tiles.iter().find(|t| t.tx == 0 && t.ty == 0).unwrap();
        assert!(alpha_at(first, 0, 0) > 0);
        let last = &tex.tiles.iter().find(|t| t.tx == 5 && t.ty == 5).unwrap();
        // 3000 - 5*512 = 440 : le centre du disque final est en (440,440) local
        assert!(alpha_at(last, 440, 440) > 0);
    }

    #[test]
    fn extension_du_trait_ne_deplace_pas_les_tuiles_existantes() {
        // Régression du bug de dérive : estampe proche, puis segment loin —
        // les pixels de la première estampe restent EXACTEMENT en place.
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

        // Segment loin de la première estampe : aucune retouche possible
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
        // Centre : intérieur de l'anneau → transparent
        assert_eq!(alpha_at(t, 100, 100), 0);
        // Bande de l'anneau (d ≤ r) : opaque à 85 % (RING_ALPHA)
        assert_eq!(alpha_at(t, 119, 100), 217);
    }
}
