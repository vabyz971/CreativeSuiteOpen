// CreativeSuiteOpen — Suite créative professionnelle open source
// Copyright (C) 2025 vabyz971
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
use iced::{Color, Point, Rectangle, Size, Theme, Vector};

use crate::theme::colors;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasTool {
    Hand,
    Zoom,
    Select,
    Move,
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
    MoveLayer { dx: f32, dy: f32 },
    /// Fin du déplacement — valide le décalage
    MoveLayerEnd,
}

/// Un calque affichable sur le canvas — dessiné à SA position monde.
/// Déplacer = changer offset → simple redraw, zéro recomposite.
/// L'opacité est appliquée AU DRAW (GPU) — zéro régénération de texture.
pub struct CanvasLayer {
    pub handle: image::Handle,
    pub width: f32,
    pub height: f32,
    /// Position monde (0,0) = coin haut-gauche du document
    pub offset_x: f32,
    pub offset_y: f32,
    /// Opacité 0..1 appliquée au draw (sans toucher aux pixels)
    pub opacity: f32,
}

pub struct ImageCanvas {
    pub layers: Vec<CanvasLayer>,
    /// Dimensions du document (repère au sol, dessiné dans l'espace monde)
    pub doc_size: Option<Size>,
    pub pan: Vector,
    pub zoom: f32,
    pub tool: CanvasTool,
    pub selection: Option<Rectangle>,
}

impl ImageCanvas {
    pub fn new(
        doc_size: Option<Size>,
        pan: Vector,
        zoom: f32,
    ) -> Self {
        Self {
            layers: Vec::new(),
            doc_size,
            pan,
            zoom: zoom.clamp(0.08, 6.0),
            tool: CanvasTool::Hand,
            selection: None,
        }
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
    pub selecting: Option<(Point, Point)>, // start, current
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
            if let Some((start, end)) = state.selecting.take() {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
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
                } else if self.tool == CanvasTool::Zoom {
                    // Clic sans drag = zoom sur point (Alt = zoom arrière)
                    let base_factor = 1.4_f32;
                    let factor = if state.modifiers.alt() {
                        1.0 / base_factor
                    } else {
                        base_factor
                    };
                    let new_zoom = (self.zoom * factor).clamp(0.08, 6.0);
                    let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
                    let factor_ratio = new_zoom / self.zoom;
                    let new_pan = Vector::new(
                        cursor_pos.x
                            - center.x
                            - (cursor_pos.x - center.x - self.pan.x) * factor_ratio,
                        cursor_pos.y
                            - center.y
                            - (cursor_pos.y - center.y - self.pan.y) * factor_ratio,
                    );
                    return Some(canvas::Action::publish(ImageCanvasEvent::ZoomAt {
                        zoom: new_zoom,
                        pan: new_pan,
                    }));
                } else {
                    return Some(canvas::Action::publish(ImageCanvasEvent::SelectRect(None)));
                }
            }
            if let Some((_start, _orig_pan)) = state.dragging.take() {
                if self.tool == CanvasTool::Move {
                    return Some(
                        canvas::Action::publish(ImageCanvasEvent::MoveLayerEnd).and_capture(),
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
                            canvas::Action::publish(ImageCanvasEvent::MoveLayerStart)
                                .and_capture(),
                        )
                    }
                    CanvasTool::Zoom | CanvasTool::Select => {
                        // Clic simple zoom ou début sélection rect
                        state.selecting = Some((cursor_pos, cursor_pos));
                        Some(canvas::Action::capture())
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
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
                        return Some(canvas::Action::publish(ImageCanvasEvent::MoveLayer { dx, dy }));
                    }
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
            let w = l.width * self.zoom;
            let h = l.height * self.zoom;
            let top_left = Point::new(
                center.x + (l.offset_x - doc_half_w) * self.zoom,
                center.y + (l.offset_y - doc_half_h) * self.zoom,
            );
            frame.draw_image(
                Rectangle::new(top_left, Size::new(w, h)),
                iced_core::Image::new(l.handle.clone()).opacity(l.opacity),
            );
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
            frame.fill_rectangle(
                norm.position(),
                norm.size(),
                Color::from_rgba(0.2, 0.5, 0.9, 0.15),
            );
            let path = Path::rectangle(norm.position(), norm.size());
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(1.0)
                    .with_color(Color::from_rgb(0.2, 0.5, 0.9)),
            );
        } else if let Some(sel) = self.selection {
            frame.fill_rectangle(
                sel.position(),
                sel.size(),
                Color::from_rgba(0.2, 0.5, 0.9, 0.15),
            );
            let path = Path::rectangle(sel.position(), sel.size());
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(1.0)
                    .with_color(Color::from_rgb(0.2, 0.5, 0.9)),
            );
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
                CanvasTool::Zoom => mouse::Interaction::ZoomIn,
                CanvasTool::Select => mouse::Interaction::Crosshair,
            };
        }
        mouse::Interaction::default()
    }
}

pub fn view_with_tool<'a>(
    doc_size: Option<Size>,
    pan: Vector,
    zoom: f32,
    tool: CanvasTool,
    selection: Option<Rectangle>,
    layers: Vec<CanvasLayer>,
) -> iced::Element<'a, ImageCanvasEvent> {
    let program = ImageCanvas::new(doc_size, pan, zoom)
        .with_layers(layers)
        .with_tool(tool)
        .with_selection(selection);
    iced::widget::canvas(program)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}
