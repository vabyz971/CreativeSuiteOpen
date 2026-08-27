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
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::widget::image;
use iced::{Point, Rectangle, Size, Theme, Vector};

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
    /// Start moving selected layer
    MoveLayerStart,
    /// Move selected layer (dx/dy in image pixels since drag start)
    MoveLayer {
        dx: f32,
        dy: f32,
    },
    /// End of move — commit offset
    MoveLayerEnd,
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
pub struct CanvasLayer {
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
    /// Uniform scale (1.0 = 100%)
    pub scale: f32,
}

/// Interactive image canvas program.
pub struct ImageCanvas {
    pub layers: Vec<CanvasLayer>,
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
    pub fn with_tool(mut self, tool: CanvasTool) -> Self {
        self.tool = tool;
        self
    }
    #[must_use]
    pub fn with_selection(mut self, sel: Option<Rectangle>) -> Self {
        self.selection = sel;
        self
    }
}

#[derive(Default)]
pub struct State {
    pub dragging: Option<(Point, Vector)>,
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
                // Espace maintenu → pan temporaire quel que soit l'outil
                if state.space_held {
                    state.dragging = Some((cursor_pos, self.pan));
                    return Some(canvas::Action::capture());
                }
                // Tools
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
                    CanvasTool::Select => {
                        state.selecting = Some((cursor_pos, cursor_pos));
                        Some(canvas::Action::capture())
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // Update tool size preview and scrubby zoom
                state.cursor_pos = Some(cursor_pos);
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
                    } else if self.tool == CanvasTool::Move {
                        // Raw screen delta: preview follows cursor 1:1,
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
            // naturally overflow without clipping.
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

        // Rect selection (Select/Zoom tool) — as in Bezier Pending example
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
    brush: BrushStyle,
    can_paint: bool,
    pending_preview: Option<StrokeTex>,
) -> iced::Element<'a, ImageCanvasEvent> {
    let program = ImageCanvas::new(doc_size, pan, zoom)
        .with_layers(layers)
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
