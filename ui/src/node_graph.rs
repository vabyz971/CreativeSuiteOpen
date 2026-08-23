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

//! Graphe nodal interactif - Canvas infini façon Blender + cache Bézier
//! Optimisé : culling, LOD, cache persistant, grille constante

use suite_core::Graph;
use datatypes::{NodeId, SocketType, Vec2};

use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Text};
use iced::widget::image;
use iced::{Color, Point, Rectangle, Size, Theme, Vector};
use std::collections::HashMap;

use crate::theme::{colors, metrics};

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum NodeGraphEvent {
    NodeSelected(NodeId),
    NodeMoved { id: NodeId, position: Vec2 },
    PanePan(Vector),
    GraphZoom { zoom: f32, pan: Vector },
    Connect { from: NodeId, from_socket: String, to: NodeId, to_socket: String },
    Disconnect { node: NodeId, socket: String },
    TogglePreview(NodeId),
    BackgroundClicked,
    /// Position monde (placement du nœud) + position LOCALE au canvas (ancrage exact du menu)
    RequestContextMenu(Vec2, iced::Point),
    /// Drag d'une sortie vers le vide : ouvre le menu de création et auto-connecte
    RequestConnectMenu {
        from: NodeId,
        from_socket: String,
        from_type: SocketType,
        is_output: bool,
        world: Vec2,
        local: iced::Point,
    },
}

// ---------------------------------------------------------------------------
// Snapshot d'un node
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct NodeView {
    id: NodeId,
    name: String,
    type_id: String,
    pos: Vec2,
    header_color: Color,
    inputs: Vec<(String, SocketType)>,
    outputs: Vec<(String, SocketType)>,
    selected: bool,
    preview_enabled: bool,
    /// Nœud désactivé = effet bypassé (affiché grisé/translucide)
    enabled: bool,
}

fn node_view_from_graph(graph: &Graph, id: NodeId, selected: Option<NodeId>) -> Option<NodeView> {
    let node = graph.get(id)?;
    let (header_color, inputs, outputs) = node_palette(&node.type_id, &node.params);
    Some(NodeView {
        id,
        name: node.name.clone(),
        type_id: node.type_id.clone(),
        pos: node.position,
        header_color,
        inputs,
        outputs,
        selected: selected == Some(id),
        preview_enabled: node.preview_enabled,
        enabled: node.enabled,
    })
}

fn node_palette(type_id: &str, params: &std::collections::HashMap<String, datatypes::ParamValue>) -> (Color, Vec<(String, SocketType)>, Vec<(String, SocketType)>) {
    match type_id {
        "input_image" => (
            Color::from_rgb(0.25, 0.45, 0.75),
            vec![],
            vec![("image".into(), SocketType::Image)],
        ),
        "output" => (
            Color::from_rgb(0.65, 0.20, 0.20),
            vec![("image".into(), SocketType::Image)],
            vec![],
        ),
        "brightness_contrast" => (
            Color::from_rgb(0.75, 0.55, 0.15),
            vec![("image".into(), SocketType::Image)],
            vec![("image".into(), SocketType::Image)],
        ),
        "layer" => (
            Color::from_rgb(0.45, 0.55, 0.85),
            vec![
                ("base".into(), SocketType::Image),
                ("top".into(), SocketType::Image),
            ],
            vec![("image".into(), SocketType::Image)],
        ),
        "blur" => (
            Color::from_rgb(0.20, 0.55, 0.75),
            vec![("image".into(), SocketType::Image)],
            vec![("image".into(), SocketType::Image)],
        ),
        "mix" | "blend" => {
            // Entrées dynamiques : n sockets visibles selon le paramètre count
            let count = params
                .get("count")
                .and_then(|v| match v {
                    datatypes::ParamValue::Int(i) => Some(*i as usize),
                    _ => None,
                })
                .unwrap_or(2)
                .clamp(2, 6);
            let inputs: Vec<(String, SocketType)> = (1..=count)
                .map(|i| (format!("image_{i}"), SocketType::Image))
                .collect();
            (
                Color::from_rgb(0.45, 0.35, 0.65),
                inputs,
                vec![("image".into(), SocketType::Image)],
            )
        }
        "color_correct" => (
            Color::from_rgb(0.85, 0.55, 0.10),
            vec![("image".into(), SocketType::Image)],
            vec![("image".into(), SocketType::Image)],
        ),
        _ => (
            Color::from_rgb(0.35, 0.35, 0.35),
            vec![("in".into(), SocketType::Float)],
            vec![("out".into(), SocketType::Float)],
        ),
    }
}

fn node_size(view: &NodeView) -> Size {
    let rows = view.inputs.len().max(view.outputs.len()).max(1);
    let mut h = metrics::NODE_HEADER_HEIGHT + rows as f32 * metrics::NODE_ROW_HEIGHT + 8.0;
    if view.preview_enabled {
        h += 84.0;
    }
    Size::new(metrics::NODE_WIDTH, h)
}

fn preview_button_rect(view: &NodeView) -> Rectangle {
    let b = node_bounds(view);
    Rectangle::new(
        Point::new(b.x + b.width - 22.0, b.y + 6.0),
        Size::new(16.0, 16.0),
    )
}

fn preview_rect(view: &NodeView) -> Rectangle {
    let b = node_bounds(view);
    Rectangle::new(
        Point::new(b.x + 4.0, b.y + b.height - 84.0 + 4.0),
        Size::new(b.width - 8.0, 76.0),
    )
}

fn node_bounds(view: &NodeView) -> Rectangle {
    Rectangle::new(Point::new(view.pos.x, view.pos.y), node_size(view))
}

fn socket_position(view: &NodeView, index: usize, is_input: bool) -> Point {
    let bounds = node_bounds(view);
    let y = bounds.y + metrics::NODE_HEADER_HEIGHT + 8.0 + index as f32 * metrics::NODE_ROW_HEIGHT + metrics::NODE_ROW_HEIGHT / 2.0;
    let x = if is_input { bounds.x } else { bounds.x + bounds.width };
    Point::new(x, y)
}

// ---------------------------------------------------------------------------
// Helpers pan/zoom
// ---------------------------------------------------------------------------

fn screen_to_world(screen: Point, pan: Vector, zoom: f32) -> Point {
    Point::new((screen.x - pan.x) / zoom, (screen.y - pan.y) / zoom)
}
fn world_to_screen(world: Point, pan: Vector, zoom: f32) -> Point {
    Point::new(world.x * zoom + pan.x, world.y * zoom + pan.y)
}

fn find_socket_at(views: &[NodeView], cursor_screen: Point, pan: Vector, zoom: f32) -> Option<(NodeId, String, SocketType, Point, bool)> {
    for view in views.iter().rev() {
        for (idx, (name, ty)) in view.inputs.iter().enumerate() {
            let p_world = socket_position(view, idx, true);
            let sp = world_to_screen(p_world, pan, zoom);
            if (cursor_screen.x - sp.x).abs() < metrics::NODE_SOCKET_HIT_RADIUS
                && (cursor_screen.y - sp.y).abs() < metrics::NODE_SOCKET_HIT_RADIUS
            {
                return Some((view.id, name.clone(), *ty, p_world, true));
            }
        }
        for (idx, (name, ty)) in view.outputs.iter().enumerate() {
            let p_world = socket_position(view, idx, false);
            let sp = world_to_screen(p_world, pan, zoom);
            if (cursor_screen.x - sp.x).abs() < metrics::NODE_SOCKET_HIT_RADIUS
                && (cursor_screen.y - sp.y).abs() < metrics::NODE_SOCKET_HIT_RADIUS
            {
                return Some((view.id, name.clone(), *ty, p_world, false));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// State persistant avec cache
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Connecting {
    from_node: NodeId,
    from_socket: String,
    from_type: SocketType,
    from_pos: Point,
    current: Point,
    is_output: bool,
}

pub struct InteractionState {
    dragging: Option<(NodeId, Vector)>,
    panning: Option<(Point, Vector)>,
    connecting: Option<Connecting>,
    cache: Cache,
    last_len: usize,
    last_conn_len: usize,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            dragging: None,
            panning: None,
            connecting: None,
            cache: Cache::new(),
            last_len: 0,
            last_conn_len: 0,
        }
    }
}

pub struct NodeGraph {
    pub graph: Graph,
    pub selected: Option<NodeId>,
    pub pan: Vector,
    pub zoom: f32,
    pub previews: HashMap<NodeId, image::Handle>,
    /// Nœuds actuellement traités par le worker de rendu (indicateur animé)
    pub busy: std::collections::HashSet<NodeId>,
}

impl NodeGraph {
    pub fn new(graph: Graph, selected: Option<NodeId>) -> Self {
        Self {
            graph,
            selected,
            pan: Vector::new(0.0, 0.0),
            zoom: 1.0,
            previews: HashMap::new(),
            busy: std::collections::HashSet::new(),
        }
    }

    fn views(&self) -> Vec<NodeView> {
        self.graph
            .nodes
            .keys()
            .filter_map(|id| node_view_from_graph(&self.graph, *id, self.selected))
            .collect()
    }
}

impl canvas::Program<NodeGraphEvent> for NodeGraph {
    type State = InteractionState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<NodeGraphEvent>> {
        // Invalidation du cache quand la structure du graphe change
        // (ouverture d'image, ajout/suppression de calque) — sinon le canvas
        // garde l'ancienne géométrie jusqu'à la prochaine interaction.
        if self.graph.len() != state.last_len
            || self.graph.connections.len() != state.last_conn_len
        {
            state.cache.clear();
            state.last_len = self.graph.len();
            state.last_conn_len = self.graph.connections.len();
        }

        let cursor_pos = cursor.position_in(bounds)?;

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let views = self.views();
                if let Some((node, socket, _ty, _pos, is_input)) = find_socket_at(&views, cursor_pos, self.pan, self.zoom)
                    && is_input {
                        let has_conn = self.graph.connections.iter().any(|c| c.to_node == node && c.to_socket == socket);
                        if has_conn {
                            state.cache.clear();
                            return Some(canvas::Action::publish(NodeGraphEvent::Disconnect { node, socket }).and_capture());
                        }
                    }
                let world = screen_to_world(cursor_pos, self.pan, self.zoom);
                state.cache.clear();
                return Some(canvas::Action::publish(NodeGraphEvent::RequestContextMenu(
                    Vec2::new(world.x, world.y),
                    cursor_pos,
                )).and_capture());
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let views = self.views();
                for view in views.iter().rev() {
                    let btn_world = preview_button_rect(view);
                    let sp = world_to_screen(btn_world.position(), self.pan, self.zoom);
                    let screen_btn = Rectangle::new(sp, Size::new(btn_world.width * self.zoom, btn_world.height * self.zoom));
                    if screen_btn.contains(cursor_pos) {
                        state.cache.clear();
                        return Some(canvas::Action::publish(NodeGraphEvent::TogglePreview(view.id)).and_capture());
                    }
                }
                if let Some((node, socket, ty, pos_world, is_input)) = find_socket_at(&views, cursor_pos, self.pan, self.zoom) {
                    let cursor_world = screen_to_world(cursor_pos, self.pan, self.zoom);
                    let is_output = !is_input;
                    state.connecting = Some(Connecting {
                        from_node: node,
                        from_socket: socket,
                        from_type: ty,
                        from_pos: pos_world,
                        current: cursor_world,
                        is_output,
                    });
                    return Some(canvas::Action::publish(NodeGraphEvent::NodeSelected(node)).and_capture());
                }
                for view in views.iter().rev() {
                    let b = node_bounds(view);
                    let cursor_world = screen_to_world(cursor_pos, self.pan, self.zoom);
                    if b.contains(cursor_world) {
                        let offset = Vector::new(b.x - cursor_world.x, b.y - cursor_world.y);
                        state.dragging = Some((view.id, offset));
                        return Some(canvas::Action::publish(NodeGraphEvent::NodeSelected(view.id)).and_capture());
                    }
                }
                state.panning = Some((cursor_pos, self.pan));
                return Some(canvas::Action::publish(NodeGraphEvent::BackgroundClicked).and_capture());
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(conn) = state.connecting.take() {
                    state.cache.clear();
                    let views = self.views();
                    if let Some((target_node, target_socket, _target_ty, _pos, is_input)) = find_socket_at(&views, cursor_pos, self.pan, self.zoom) {
                        let valid = if conn.is_output { is_input } else { !is_input };
                        if valid {
                            let (from, from_sock, to, to_sock) = if conn.is_output {
                                (conn.from_node, conn.from_socket.clone(), target_node, target_socket.clone())
                            } else {
                                (target_node, target_socket.clone(), conn.from_node, conn.from_socket.clone())
                            };
                            if from != to {
                                return Some(canvas::Action::publish(NodeGraphEvent::Connect { from, from_socket: from_sock, to, to_socket: to_sock }).and_capture());
                            }
                        }
                    }
                    // Relâché dans le vide : ouvre le menu de création pour auto-connecter
                    let world = screen_to_world(cursor_pos, self.pan, self.zoom);
                    return Some(
                        canvas::Action::publish(NodeGraphEvent::RequestConnectMenu {
                            from: conn.from_node,
                            from_socket: conn.from_socket,
                            from_type: conn.from_type,
                            is_output: conn.is_output,
                            world: Vec2::new(world.x, world.y),
                            local: cursor_pos,
                        })
                        .and_capture(),
                    );
                }
                if state.dragging.is_some() {
                    state.dragging = None;
                    state.cache.clear();
                    return Some(canvas::Action::capture());
                }
                if state.panning.is_some() {
                    state.panning = None;
                    return Some(canvas::Action::capture());
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(conn) = state.connecting.as_mut() {
                    conn.current = screen_to_world(cursor_pos, self.pan, self.zoom);
                    return Some(canvas::Action::request_redraw());
                }
                if let Some((id, offset)) = state.dragging {
                    let cursor_world = screen_to_world(cursor_pos, self.pan, self.zoom);
                    let pos = Vec2::new(cursor_world.x + offset.x, cursor_world.y + offset.y);
                    state.cache.clear();
                    return Some(canvas::Action::publish(NodeGraphEvent::NodeMoved { id, position: pos }).and_capture());
                }
                if let Some((start, orig_pan)) = state.panning
                    && state.dragging.is_none() && state.connecting.is_none() {
                        let delta = Vector::new(cursor_pos.x - start.x, cursor_pos.y - start.y);
                        let new_pan = Vector::new(orig_pan.x + delta.x, orig_pan.y + delta.y);
                        state.cache.clear();
                        return Some(canvas::Action::publish(NodeGraphEvent::PanePan(new_pan)).and_capture());
                    }
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let delta_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 20.0,
                };
                if delta_y.abs() < 0.01 {
                    return None;
                }
                let factor = (1.12_f32).powf(delta_y);
                let new_zoom = (self.zoom * factor).clamp(0.25, 3.0);
                let factor_ratio = new_zoom / self.zoom;
                let new_pan = Vector::new(
                    cursor_pos.x - (cursor_pos.x - self.pan.x) * factor_ratio,
                    cursor_pos.y - (cursor_pos.y - self.pan.y) * factor_ratio,
                );
                state.cache.clear();
                return Some(canvas::Action::publish(NodeGraphEvent::GraphZoom { zoom: new_zoom, pan: new_pan }).and_capture());
            }
            _ => {}
        }
        None
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        // Cache persistant pour le fond + nœuds statiques
        let static_geom = state.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), colors::BG_GRAPH_GRID);

            // Grille : taille écran constante 24px, pas 24*zoom (évite explosion à dézoom)
            let grid = 24.0;
            let offset_x = self.pan.x.rem_euclid(grid);
            let offset_y = self.pan.y.rem_euclid(grid);
            let cols = (bounds.width / grid).ceil() as usize + 1;
            let rows = (bounds.height / grid).ceil() as usize + 1;
            // LOD : si zoom très petit, on allège la grille (1 point sur 2)
            let step = if self.zoom < 0.5 { 2 } else { 1 };
            for c in (0..cols).step_by(step) {
                for r in (0..rows).step_by(step) {
                    let p = Point::new(offset_x + c as f32 * grid, offset_y + r as f32 * grid);
                    // Culling écran déjà par cols/rows, pas besoin de test
                    let dot = Path::circle(p, 1.2);
                    frame.fill(&dot, colors::BG_GRAPH_DOT);
                }
            }

            frame.translate(self.pan);
            frame.scale(self.zoom);

            // Visible world pour culling
            let visible = Rectangle::new(
                Point::new(-self.pan.x / self.zoom, -self.pan.y / self.zoom),
                Size::new(bounds.width / self.zoom, bounds.height / self.zoom),
            );
            let visible_expanded = Rectangle::new(
                Point::new(visible.x - 200.0, visible.y - 200.0),
                Size::new(visible.width + 400.0, visible.height + 400.0),
            );

            // Connections avec culling
            for conn in &self.graph.connections {
                let from_view = match node_view_from_graph(&self.graph, conn.from_node, self.selected) {
                    Some(v) => v,
                    None => continue,
                };
                let to_view = match node_view_from_graph(&self.graph, conn.to_node, self.selected) {
                    Some(v) => v,
                    None => continue,
                };
                // On vérifie si l'un des deux nœuds est visible
                let fb = node_bounds(&from_view);
                let tb = node_bounds(&to_view);
                if !visible_expanded.intersects(&fb) && !visible_expanded.intersects(&tb) {
                    continue;
                }
                let from_idx = from_view.outputs.iter().position(|(n, _)| n == &conn.from_socket).unwrap_or(0);
                let to_idx = to_view.inputs.iter().position(|(n, _)| n == &conn.to_socket).unwrap_or(0);
                let p0 = socket_position(&from_view, from_idx, false);
                let p1 = socket_position(&to_view, to_idx, true);
                let dx = (p1.x - p0.x).abs().max(60.0);
                let c0 = Point::new(p0.x + dx * 0.5, p0.y);
                let c1 = Point::new(p1.x - dx * 0.5, p1.y);
                let cable = Path::new(|b| {
                    b.move_to(p0);
                    b.bezier_curve_to(c0, c1, p1);
                });
                let ty_color = {
                    let [r, g, b] = conn.socket_type.color();
                    Color::from_rgb(r, g, b)
                };
                frame.stroke(&cable, canvas::Stroke { style: canvas::Style::Solid(colors::CABLE_SHADOW), width: (metrics::CABLE_WIDTH + 2.0) / self.zoom, line_cap: canvas::LineCap::Round, ..Default::default() });
                frame.stroke(&cable, canvas::Stroke { style: canvas::Style::Solid(ty_color), width: metrics::CABLE_WIDTH / self.zoom, line_cap: canvas::LineCap::Round, ..Default::default() });
                let _ = p0;
            }

            // Nodes avec culling + LOD
            let show_details = self.zoom >= 0.45;
            let show_labels = self.zoom >= 0.6;
            let show_previews = self.zoom >= 0.7;
            let views = self.views();
            for view in &views {
                let bounds_n = node_bounds(view);
                if !visible_expanded.intersects(&bounds_n) {
                    continue;
                }
                let rect = Path::rounded_rectangle(Point::new(bounds_n.x, bounds_n.y), bounds_n.size(), metrics::RADIUS_NODE.into());
                // Nœud désactivé : rendu grisé/translucide (l'effet est bypassé)
                let dim: f32 = if view.enabled { 1.0 } else { 0.35 };
                let bg_base = if view.selected { colors::BG_NODE_SELECTED } else { colors::BG_NODE };
                let bg = Color { a: bg_base.a * dim.max(0.45), ..bg_base };
                frame.fill(&rect, bg);
                let border_color = if !view.enabled {
                    colors::TEXT_MUTED
                } else if view.selected {
                    colors::BORDER_NODE_SELECTED
                } else {
                    colors::BORDER_NODE
                };
                let bw = if view.selected { metrics::BORDER_WIDTH_NODE_SELECTED } else { metrics::BORDER_WIDTH_NODE };
                frame.stroke(&rect, canvas::Stroke { style: canvas::Style::Solid(border_color), width: bw / self.zoom, ..Default::default() });
                let header_rect = Path::new(|b| {
                    let r = metrics::RADIUS_NODE;
                    let x = bounds_n.x;
                    let y = bounds_n.y;
                    let w = bounds_n.width;
                    let h = metrics::NODE_HEADER_HEIGHT;
                    b.move_to(Point::new(x + r, y));
                    b.line_to(Point::new(x + w - r, y));
                    b.arc_to(Point::new(x + w, y), Point::new(x + w, y + r), r);
                    b.line_to(Point::new(x + w, y + h));
                    b.line_to(Point::new(x, y + h));
                    b.line_to(Point::new(x, y + r));
                    b.arc_to(Point::new(x, y), Point::new(x + r, y), r);
                    b.close();
                });
                frame.fill(&header_rect, Color { a: if view.enabled { 1.0 } else { 0.35 }, ..view.header_color });
                frame.fill_text(Text { content: view.name.clone(), position: Point::new(bounds_n.x + 10.0, bounds_n.y + metrics::NODE_HEADER_HEIGHT / 2.0), color: if view.enabled { Color::WHITE } else { colors::TEXT_MUTED }, size: iced::Pixels(12.0), font: iced::Font::default(), align_x: iced::alignment::Horizontal::Left.into(), align_y: iced::alignment::Vertical::Center, line_height: iced::widget::text::LineHeight::default(), shaping: iced::widget::text::Shaping::Basic, max_width: f32::INFINITY, ..Default::default() });
                if show_details {
                    let btn = preview_button_rect(view);
                    let eye = if view.preview_enabled { "\u{e8f4}" } else { "\u{e8f5}" };
                    let btn_bg = Path::circle(btn.center(), 8.0);
                    frame.fill(&btn_bg, if view.preview_enabled { Color::from_rgb(0.20, 0.45, 0.85) } else { Color::from_rgba(0.0, 0.0, 0.0, 0.25) });
                    frame.fill_text(Text { content: eye.to_string(), position: btn.center(), color: Color::WHITE, size: iced::Pixels(10.0), font: iced::Font::with_name("Material Icons"), align_x: iced::alignment::Horizontal::Center.into(), align_y: iced::alignment::Vertical::Center, line_height: iced::widget::text::LineHeight::default(), shaping: iced::widget::text::Shaping::Basic, max_width: f32::INFINITY, ..Default::default() });
                }
                // Sockets : toujours dessinés mais labels selon LOD
                for (idx, (name, ty)) in view.inputs.iter().enumerate() {
                    let p = socket_position(view, idx, true);
                    let c = Path::circle(p, metrics::NODE_SOCKET_RADIUS);
                    let col = { let [r, g, b] = ty.color(); Color::from_rgb(r, g, b) };
                    frame.fill(&c, col);
                    frame.stroke(&c, canvas::Stroke { style: canvas::Style::Solid(Color::from_rgb(0.05, 0.05, 0.05)), width: 1.2 / self.zoom, ..Default::default() });
                    if show_labels {
                        frame.fill_text(Text { content: name.clone(), position: Point::new(p.x + 12.0, p.y), color: colors::TEXT_SECONDARY, size: iced::Pixels(10.0), align_x: iced::alignment::Horizontal::Left.into(), align_y: iced::alignment::Vertical::Center, line_height: iced::widget::text::LineHeight::default(), shaping: iced::widget::text::Shaping::Basic, max_width: 120.0, ..Default::default() });
                    }
                }
                for (idx, (name, ty)) in view.outputs.iter().enumerate() {
                    let p = socket_position(view, idx, false);
                    let c = Path::circle(p, metrics::NODE_SOCKET_RADIUS);
                    let col = { let [r, g, b] = ty.color(); Color::from_rgb(r, g, b) };
                    frame.fill(&c, col);
                    frame.stroke(&c, canvas::Stroke { style: canvas::Style::Solid(Color::from_rgb(0.05, 0.05, 0.05)), width: 1.2 / self.zoom, ..Default::default() });
                    if show_labels {
                        frame.fill_text(Text { content: name.clone(), position: Point::new(p.x - 12.0, p.y), color: colors::TEXT_SECONDARY, size: iced::Pixels(10.0), align_x: iced::alignment::Horizontal::Right.into(), align_y: iced::alignment::Vertical::Center, line_height: iced::widget::text::LineHeight::default(), shaping: iced::widget::text::Shaping::Basic, max_width: 120.0, ..Default::default() });
                    }
                }
                if view.preview_enabled && show_previews {
                    let pr = preview_rect(view);
                    let checker: f32 = 6.0;
                    let cols = (pr.width / checker).ceil() as usize;
                    let rows = (pr.height / checker).ceil() as usize;
                    // Limite checker à zoom faible
                    if self.zoom > 0.6 {
                        frame.with_clip(pr, |frame| {
                            for c in 0..cols { for r in 0..rows { let col = if (c + r) % 2 == 0 { Color::from_rgb(0.22, 0.22, 0.22) } else { Color::from_rgb(0.28, 0.28, 0.28) }; frame.fill_rectangle(Point::new(pr.x + c as f32 * checker, pr.y + r as f32 * checker), Size::new(checker, checker), col); } }
                        });
                    }
                    let pr_path = Path::rectangle(pr.position(), pr.size());
                    frame.stroke(&pr_path, canvas::Stroke::default().with_width(1.0 / self.zoom).with_color(Color::from_rgb(0.30, 0.30, 0.30)));
                    if let Some(handle) = self.previews.get(&view.id) { frame.draw_image(pr, iced_core::Image::new(handle.clone())); } else { frame.fill_text(Text { content: "Aperçu...".into(), position: pr.center(), color: colors::TEXT_MUTED, size: iced::Pixels(9.0), align_x: iced::alignment::Horizontal::Center.into(), align_y: iced::alignment::Vertical::Center, ..Default::default() }); }
                }
            }
        });

        let mut layers = vec![static_geom];

        // Couche DYNAMIQUE (non cachée, reconstruite à chaque frame) :
        // - clignotement d'opacité des nœuds en traitement (worker de rendu)
        // - câble en cours de connexion
        if !self.busy.is_empty() || state.connecting.is_some() {
            let mut frame = Frame::new(renderer, bounds.size());

            if !self.busy.is_empty() {
                frame.translate(self.pan);
                frame.scale(self.zoom);
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as f32)
                    .unwrap_or(0.0);
                // Cycle de 1,4 s : ping-pong linéaire + smoothstep (ease-in-out
                // cubic) sur chaque montée/descente → clignotement doux aux deux extrémités
                let t = (now_ms % 1400.0) / 1400.0;
                let p = if t < 0.5 { t * 2.0 } else { 2.0 - t * 2.0 };
                let eased = p * p * (3.0 - 2.0 * p); // smoothstep ease-in-out
                let veil_alpha = 0.30 * eased;
                for view in self.views() {
                    if !self.busy.contains(&view.id) {
                        continue;
                    }
                    let b = node_bounds(&view);
                    let veil = Path::rounded_rectangle(
                        Point::new(b.x, b.y),
                        b.size(),
                        metrics::RADIUS_NODE.into(),
                    );
                    frame.fill(&veil, Color::from_rgba(0.02, 0.03, 0.05, veil_alpha));
                }
            }

            if let Some(conn) = &state.connecting {
                frame.translate(self.pan);
                frame.scale(self.zoom);
                let p0 = conn.from_pos;
                let p1 = conn.current;
                let dx = (p1.x - p0.x).abs().max(60.0);
                let (c0, c1) = if conn.is_output { (Point::new(p0.x + dx * 0.5, p0.y), Point::new(p1.x - dx * 0.5, p1.y)) } else { (Point::new(p0.x - dx * 0.5, p0.y), Point::new(p1.x + dx * 0.5, p1.y)) };
                let cable = Path::new(|b| { b.move_to(p0); b.bezier_curve_to(c0, c1, p1); });
                let col = { let [r,g,b] = conn.from_type.color(); Color::from_rgb(r,g,b) };
                frame.stroke(&cable, canvas::Stroke { style: canvas::Style::Solid(Color::from_rgba(col.r, col.g, col.b, 0.9)), width: metrics::CABLE_WIDTH / self.zoom, line_cap: canvas::LineCap::Round, ..Default::default() });
                let end = Path::circle(p1, 4.0 / self.zoom);
                frame.fill(&end, col);
                if let Some(pos) = cursor.position_in(bounds) {
                    let views = self.views();
                    if let Some((_n,_s,_ty,p,_inp)) = find_socket_at(&views, pos, self.pan, self.zoom) {
                        let hl = Path::circle(p, 8.0);
                        frame.stroke(&hl, canvas::Stroke { style: canvas::Style::Solid(Color::WHITE), width: 1.5 / self.zoom, ..Default::default() });
                    }
                }
            }

            layers.push(frame.into_geometry());
        }
        layers
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging.is_some() { return mouse::Interaction::Grabbing; }
        if state.connecting.is_some() { return mouse::Interaction::Crosshair; }
        if let Some(pos) = cursor.position_in(bounds) {
            for view in self.views() {
                let btn = preview_button_rect(&view);
                let sp = world_to_screen(btn.position(), self.pan, self.zoom);
                let screen_btn = Rectangle::new(sp, Size::new(btn.width * self.zoom, btn.height * self.zoom));
                if screen_btn.contains(pos) { return mouse::Interaction::Pointer; }
            }
            let views = self.views();
            if find_socket_at(&views, pos, self.pan, self.zoom).is_some() { return mouse::Interaction::Crosshair; }
            let world = screen_to_world(pos, self.pan, self.zoom);
            for view in self.views() { if node_bounds(&view).contains(world) { return mouse::Interaction::Grab; } }
            if state.panning.is_some() { return mouse::Interaction::Grabbing; }
        }
        mouse::Interaction::default()
    }
}

pub fn view<'a>(
    graph: Graph,
    selected: Option<NodeId>,
    pan: Vector,
    zoom: f32,
    previews: HashMap<NodeId, image::Handle>,
    busy: &std::collections::HashSet<NodeId>,
) -> iced::Element<'a, NodeGraphEvent> {
    let program = NodeGraph {
        graph,
        selected,
        pan,
        zoom: zoom.clamp(0.25, 3.0),
        previews,
        busy: busy.clone(),
    };
    iced::widget::canvas(program)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}
