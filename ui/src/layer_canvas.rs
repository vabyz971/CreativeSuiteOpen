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

//! Canvas de calques — compositing GPU en render pass (zéro readback CPU).
//!
//! Architecture façon Affinity/Photoshop :
//! - chaque calque est une texture GPU persistante (upload UNE fois par version)
//! - la fusion se fait dans des render passes sur textures ping-pong, sur le
//!   device wgpu D'ICED (via le widget shader) — jamais de transfert vers le CPU
//! - l'affichage est un blit avec pan/zoom + fond pointillé procédural
//! - si la pile ne change pas entre deux frames, les passes de fusion sont
//!   sautées : seul le blit tourne
//!
//! Les événements souris réutilisent [`image_canvas::ImageCanvasEvent`] pour
//! rester compatible avec la logique applicative existante.

use std::collections::HashMap;
use std::sync::Arc;

use iced::widget::shader;
use iced::widget::Shader;
use iced::wgpu;
use iced::{Element, Length, Point, Rectangle, Size, Vector};

use crate::image_canvas::{CanvasTool, ImageCanvasEvent};

// ---------------------------------------------------------------------------
// Modèle affiché
// ---------------------------------------------------------------------------

/// Un calque prêt pour l'upload GPU (pixels RGBA8 préconvertis, partagés).
#[derive(Clone, Debug)]
pub struct DisplayLayer {
    /// Identité du contenu (clé de cache texture, ex : pointeur Arc)
    pub key: u64,
    pub rgba: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    /// Opacité 0..1
    pub opacity: f32,
    /// Mode de fusion (0 Normal … 5 Lighten)
    pub blend: u32,
    pub offset_x: f32,
    pub offset_y: f32,
}

pub struct LayerCanvas<Message> {
    pub layers: Vec<DisplayLayer>,
    /// Dimensions du document en pixels (None = pas de document)
    pub doc_size: Option<(f32, f32)>,
    pub pan: Vector,
    pub zoom: f32,
    pub tool: CanvasTool,
    pub selection: Option<Rectangle>,
    /// Convertit les événements canvas en messages applicatifs
    pub on_event: std::rc::Rc<dyn Fn(ImageCanvasEvent) -> Message>,
}

impl<Message> LayerCanvas<Message> {
    pub fn new(
        doc_size: Option<(f32, f32)>,
        on_event: std::rc::Rc<dyn Fn(ImageCanvasEvent) -> Message>,
    ) -> Self {
        Self {
            layers: Vec::new(),
            doc_size,
            pan: Vector::new(0.0, 0.0),
            zoom: 1.0,
            tool: CanvasTool::Hand,
            selection: None,
            on_event,
        }
    }

    pub fn with_layers(mut self, layers: Vec<DisplayLayer>) -> Self {
        self.layers = layers;
        self
    }

    pub fn with_view(mut self, pan: Vector, zoom: f32) -> Self {
        self.pan = pan;
        self.zoom = zoom.clamp(0.08, 6.0);
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
// État d'interaction (même logique que image_canvas)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct State {
    dragging: Option<(Point, Vector)>,
    selecting: Option<(Point, Point)>,
    modifiers: iced::keyboard::Modifiers,
    prev_bounds: Option<Size>,
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

        // Publie la taille du viewport à chaque changement
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

        // Relâchement traité même hors bornes (la souris est capturée pendant
        // un drag) — sinon l'état reste armé et les événements continuent.
        if let Event::Mouse(mouse::Event::ButtonReleased(Button::Left)) = event {
            if let Some((start, end)) = state.selecting.take() {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return Some(shader::Action::publish((self.on_event)(ImageCanvasEvent::SelectRect(None))));
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
                    let factor =
                        if state.modifiers.alt() { 1.0 / base_factor } else { base_factor };
                    let new_zoom = (self.zoom * factor).clamp(0.08, 6.0);
                    let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
                    let ratio = new_zoom / self.zoom;
                    let new_pan = Vector::new(
                        cursor_pos.x - center.x - (cursor_pos.x - center.x - self.pan.x) * ratio,
                        cursor_pos.y - center.y - (cursor_pos.y - center.y - self.pan.y) * ratio,
                    );
                    return Some(shader::Action::publish((self.on_event)(ImageCanvasEvent::ZoomAt {
                        zoom: new_zoom,
                        pan: new_pan,
                    })));
                } else {
                    return Some(shader::Action::publish((self.on_event)(ImageCanvasEvent::SelectRect(None))));
                }
            }
            if state.dragging.take().is_some() && self.tool == CanvasTool::Move {
                return Some(shader::Action::publish((self.on_event)(ImageCanvasEvent::MoveLayerEnd)).and_capture());
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
                        shader::Action::publish((self.on_event)(ImageCanvasEvent::MoveLayerStart))
                            .and_capture(),
                    )
                }
                CanvasTool::Zoom | CanvasTool::Select => {
                    state.selecting = Some((cursor_pos, cursor_pos));
                    Some(shader::Action::capture())
                }
            },
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some((start, orig_pan)) = state.dragging {
                    if self.tool == CanvasTool::Hand {
                        let delta = Vector::new(cursor_pos.x - start.x, cursor_pos.y - start.y);
                        return Some(shader::Action::publish((self.on_event)(ImageCanvasEvent::Pan(
                            Vector::new(orig_pan.x + delta.x, orig_pan.y + delta.y),
                        ))));
                    } else if self.tool == CanvasTool::Move {
                        // Delta écran brut ; la conversion pixels image se fait
                        // côté app avec le zoom courant.
                        return Some(shader::Action::publish((self.on_event)(ImageCanvasEvent::MoveLayer {
                            dx: cursor_pos.x - start.x,
                            dy: cursor_pos.y - start.y,
                        })));
                    }
                }
                if let Some((start, _)) = state.selecting {
                    state.selecting = Some((start, cursor_pos));
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
                Some(shader::Action::publish((self.on_event)(ImageCanvasEvent::ZoomPan {
                    zoom: new_zoom,
                    pan: new_pan,
                })))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: iced::mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        CompositePrimitive {
            layers: self.layers.clone(),
            doc_size: self.doc_size.unwrap_or((800.0, 600.0)),
            has_doc: self.doc_size.is_some(),
            pan: self.pan,
            zoom: self.zoom,
            viewport: (bounds.width.max(1.0), bounds.height.max(1.0)),
            selection: self.selection,
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
            return match self.tool {
                CanvasTool::Hand => Interaction::Grab,
                CanvasTool::Move => Interaction::Move,
                CanvasTool::Zoom => Interaction::ZoomIn,
                CanvasTool::Select => Interaction::Crosshair,
            };
        }
        Interaction::default()
    }
}

// ---------------------------------------------------------------------------
// Primitive GPU
// ---------------------------------------------------------------------------

/// Hash de configuration : si inchangé, les passes de fusion sont sautées.
fn config_hash(layers: &[DisplayLayer], doc: (f32, f32)) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let feed = |v: u64, h: &mut u64| {
        *h ^= v;
        *h = h.wrapping_mul(0x100000001b3);
    };
    feed(doc.0.to_bits() as u64, &mut h);
    feed(doc.1.to_bits() as u64, &mut h);
    feed(layers.len() as u64, &mut h);
    for l in layers {
        feed(l.key, &mut h);
        feed(l.opacity.to_bits() as u64, &mut h);
        feed(l.blend as u64, &mut h);
        feed(l.offset_x.to_bits() as u64, &mut h);
        feed(l.offset_y.to_bits() as u64, &mut h);
        feed(l.width as u64, &mut h);
        feed(l.height as u64, &mut h);
    }
    h
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
}

impl shader::Primitive for CompositePrimitive {
    type Pipeline = CompositePipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &shader::Viewport,
    ) {
        pipeline.prepare(self, device, queue);
    }

    fn draw(&self, _pipeline: &Self::Pipeline, _render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        // La fusion nécessite ses propres passes hors écran → render()
        false
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        _clip_bounds: &Rectangle<u32>,
    ) {
        pipeline.render(self, encoder, target);
    }
}

// ---------------------------------------------------------------------------
// Shaders WGSL
// ---------------------------------------------------------------------------

const SHADER: &str = r#"
struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct Params {
    screen_doc: vec4<f32>,   // xy = viewport widget px, zw = document px
    pan_zoom: vec4<f32>,     // xy = pan, z = zoom, w = opacite calque
    mode_sizes: vec4<u32>,   // x = mode fusion, y/z = dims texture top, w = flag image
    off_sel: vec4<f32>,      // xy = decalage calque, zw = position selection
    sel_size: vec4<f32>,     // xy = taille selection (x > 0 = active)
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VOut {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var out: VOut;
    out.pos = vec4<f32>(p[idx], 0.0, 1.0);
    out.uv = vec2<f32>((p[idx].x + 1.0) * 0.5, 1.0 - (p[idx].y + 1.0) * 0.5);
    return out;
}

fn blend_channel(b: f32, t: f32, mode: u32) -> f32 {
    if (mode == 1u) { return b * t; }
    if (mode == 2u) { return 1.0 - (1.0 - b) * (1.0 - t); }
    if (mode == 3u) {
        if (b < 0.5) { return 2.0 * b * t; } else { return 1.0 - 2.0 * (1.0 - b) * (1.0 - t); }
    }
    if (mode == 4u) { return min(b, t); }
    if (mode == 5u) { return max(b, t); }
    return t;
}

// Un seul bind group (limite max_bind_groups = 2 sur le device iced)
@group(0) @binding(0) var base_tex: texture_2d<f32>;
@group(0) @binding(1) var base_samp: sampler;
@group(0) @binding(2) var top_tex: texture_2d<f32>;
@group(0) @binding(3) var top_samp: sampler;
@group(0) @binding(4) var<uniform> bp: Params;

@fragment
fn fs_blend(in: VOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let b = textureSampleLevel(base_tex, base_samp, uv, 0.0);
    // Coordonnees pixel document de ce fragment
    let doc_px = uv * bp.screen_doc.zw;
    // Echantillonne le calque a (doc_px - offset), normalise par SES dimensions
    let t_px = doc_px - bp.off_sel.xy;
    let t_uv = t_px / vec2<f32>(f32(bp.mode_sizes.y), f32(bp.mode_sizes.z));
    var t = vec4<f32>(0.0);
    if (t_uv.x >= 0.0 && t_uv.x <= 1.0 && t_uv.y >= 0.0 && t_uv.y <= 1.0) {
        t = textureSampleLevel(top_tex, top_samp, t_uv, 0.0);
    }
    let ta = t.a * bp.pan_zoom.w;
    if (ta <= 0.001) { return b; }
    let br = blend_channel(b.r, t.r, bp.mode_sizes.x);
    let bg_ = blend_channel(b.g, t.g, bp.mode_sizes.x);
    let bb = blend_channel(b.b, t.b, bp.mode_sizes.x);
    let out_a = ta + b.a * (1.0 - ta);
    var out = vec4<f32>(0.0);
    if (out_a > 0.001) {
        out.r = (br * ta + b.r * b.a * (1.0 - ta)) / out_a;
        out.g = (bg_ * ta + b.g * b.a * (1.0 - ta)) / out_a;
        out.b = (bb * ta + b.b * b.a * (1.0 - ta)) / out_a;
        out.a = out_a;
    }
    return clamp(out, vec4<f32>(0.0), vec4<f32>(1.0));
}

@group(0) @binding(0) var acc_tex: texture_2d<f32>;
@group(0) @binding(1) var acc_samp: sampler;
@group(0) @binding(4) var<uniform> pp: Params;

const GRID_BG = vec3<f32>(0.055, 0.055, 0.055);   // #0E0E0E
const GRID_DOT = vec3<f32>(0.208, 0.208, 0.204);  // #353534

@fragment
fn fs_present(in: VOut) -> @location(0) vec4<f32> {
    let screen_px = in.uv * pp.screen_doc.xy;
    let pan = pp.pan_zoom.xy;
    let zoom = pp.pan_zoom.z;

    // Fond uni — damier retiré pour performance et stabilité au resize
    var col = GRID_BG;

    // Image composite (espace document), si document present
    if (pp.mode_sizes.w == 1u) {
        let doc_px = (screen_px - pp.screen_doc.xy / 2.0 - pan) / zoom + pp.screen_doc.zw / 2.0;
        if (doc_px.x >= 0.0 && doc_px.y >= 0.0 && doc_px.x <= pp.screen_doc.z && doc_px.y <= pp.screen_doc.w) {
            let auv = doc_px / pp.screen_doc.zw;
            let img = textureSampleLevel(acc_tex, acc_samp, auv, 0.0);
            col = mix(col, img.rgb, img.a);
        }
    }

    // Rectangle de selection (coords ecran)
    if (pp.sel_size.x > 0.0) {
        let smin = min(pp.off_sel.zw, pp.off_sel.zw + pp.sel_size.xy);
        let smax = max(pp.off_sel.zw, pp.off_sel.zw + pp.sel_size.xy);
        if (all(screen_px >= smin) && all(screen_px <= smax)) {
            col = mix(col, vec3<f32>(0.2, 0.5, 0.9), 0.15);
        }
        let near_min = (screen_px >= smin - 1.0) & (screen_px <= smin + 1.0);
        let near_max = (screen_px >= smax - 1.0) & (screen_px <= smax + 1.0);
        let near_edge = any(near_min) || any(near_max);
        if (all(screen_px >= smin - 1.0) && all(screen_px <= smax + 1.0) && near_edge) {
            col = vec3<f32>(0.2, 0.5, 0.9);
        }
    }
    return vec4<f32>(col, 1.0);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    /// xy = viewport widget px, zw = document px
    screen_doc: [f32; 4],
    /// xy = pan, z = zoom, w = opacité calque
    pan_zoom: [f32; 4],
    /// x = mode fusion, y/z = dims texture top, w = flag image présente
    mode_sizes: [u32; 4],
    /// xy = décalage calque (px document), zw = position sélection
    off_sel: [f32; 4],
    /// xy = taille sélection (x > 0 = active)
    sel_size: [f32; 4],
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

pub struct CompositePipeline {
    device: wgpu::Device,
    queue: wgpu::Queue,

    blend_pipeline: wgpu::RenderPipeline,
    present_pipeline: wgpu::RenderPipeline,
    params_buf: wgpu::Buffer,
    bgl_all: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,

    /// Textures de calques persistantes, clé = identité du contenu
    layer_textures: HashMap<u64, LayerTex>,
    /// Accumulateurs ping-pong (espace document)
    accum: Option<Accum>,
    /// Hash du dernier recomposite GPU (atomique : render() prend &self)
    last_hash: std::sync::atomic::AtomicU64,
}

struct LayerTex {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

struct Accum {
    views: [wgpu::TextureView; 2],
    /// Index de la texture contenant le dernier composite
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

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("layer-canvas-params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            device: device.clone(),
            queue: queue.clone(),
            blend_pipeline,
            present_pipeline,
            params_buf,
            bgl_all,
            sampler,
            layer_textures: HashMap::new(),
            accum: None,
            last_hash: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn trim(&mut self) {}
}

impl CompositePipeline {
    /// Bind group complet : tex0 + sampler en slots base, `top` réutilisé
    /// dans les slots top (inutilisés par le shader present).
    fn scene_bg(
        &self,
        view: &wgpu::TextureView,
        top_view: Option<&wgpu::TextureView>,
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
                    resource: self.params_buf.as_entire_binding(),
                },
            ],
        })
    }

    fn ensure_accum(&mut self, size: (u32, u32)) {
        let need = size != self.accum.as_ref().map(|a| a.size).unwrap_or((0, 0));
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

    fn prepare(&mut self, prim: &CompositePrimitive, device: &wgpu::Device, queue: &wgpu::Queue) {
        let doc = (
            prim.doc_size.0.round().max(1.0) as u32,
            prim.doc_size.1.round().max(1.0) as u32,
        );
        self.ensure_accum(doc);

        // Upload des nouvelles versions de calques + éviction des obsolètes
        let live: Vec<u64> = prim.layers.iter().map(|l| l.key).collect();
        for l in &prim.layers {
            self.layer_textures.entry(l.key).or_insert_with(|| {
                // Capture les erreurs de validation wgpu (sinon silencieuses)
                device.push_error_scope(wgpu::ErrorFilter::Validation);
                let w = l.width.max(1);
                let h = l.height.max(1);
                let tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("layer-canvas-layer"),
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
                // bytes_per_row doit être un multiple de 256 (COPY_ALIGNMENT
                // wgpu). Sinon erreur de validation silencieuse → texture
                // jamais téléversée → image invisible. On padde les lignes.
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
                        &l.rgba[..],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(row_bytes),
                            rows_per_image: None,
                        },
                        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    );
                } else {
                    // Copie ligne à ligne dans un buffer paddé (une seule fois
                    // par version de contenu — coût amorti)
                    let mut staged = vec![0u8; (padded_row * h) as usize];
                    for r in 0..h as usize {
                        let src = r * row_bytes as usize;
                        let dst = r * padded_row as usize;
                        staged[dst..dst + row_bytes as usize]
                            .copy_from_slice(&l.rgba[src..src + row_bytes as usize]);
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
                        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    );
                }
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                if let Some(err) = pollster::block_on(device.pop_error_scope()) {
                    eprintln!("layer-canvas: échec upload calque {:#?}", err);
                }
                LayerTex { view, width: l.width, height: l.height }
            });
        }
        self.layer_textures.retain(|k, _| live.contains(k));

        // Recomposite décidé dans render() : comparaison du hash courant
        let _ = config_hash(&prim.layers, prim.doc_size);
    }

    fn write_params(&self, params: &Params) {
        self.queue.write_buffer(
            &self.params_buf,
            0,
            bytemuck::bytes_of(params),
        );
    }

    fn render(
        &self,
        prim: &CompositePrimitive,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) {
        use std::sync::atomic::Ordering;
        if self.accum.is_none() {
            return;
        }

        // --- PASSES DE FUSION (hors écran, ping-pong) ---
        // Recomposite seulement si la pile a changé depuis la dernière frame
        let hash = config_hash(&prim.layers, prim.doc_size);
        if hash != self.last_hash.load(Ordering::Relaxed) {
            let mut cur = self.accum.as_ref().unwrap().current.load(Ordering::Relaxed);

            // Les textures wgpu démarrent avec un contenu indéfini.
            // On efface la base initiale à transparent avant la première fusion,
            // sinon le premier calque serait mélangé avec des pixels aléatoires.
            {
                let base_init = self.accum.as_ref().unwrap().views[cur].clone();
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("layer-canvas-clear-base"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view: &base_init,
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

            for layer in &prim.layers {
                let Some(tex) = self.layer_textures.get(&layer.key) else { continue };

                // Ping-pong : src = résultat précédent, dst = cible de ce calque
                let src_view = self.accum.as_ref().unwrap().views[cur].clone();
                let dst_view = self.accum.as_ref().unwrap().views[cur ^ 1].clone();

                let scene_bg = self.scene_bg(&src_view, Some(&tex.view));

                let params = Params {
                    screen_doc: [
                        prim.viewport.0,
                        prim.viewport.1,
                        prim.doc_size.0,
                        prim.doc_size.1,
                    ],
                    pan_zoom: [
                        prim.pan.x,
                        prim.pan.y,
                        prim.zoom,
                        layer.opacity.clamp(0.0, 1.0),
                    ],
                    mode_sizes: [layer.blend, tex.width, tex.height, 0],
                    off_sel: [layer.offset_x, layer.offset_y, 0.0, 0.0],
                    sel_size: [0.0, 0.0, 0.0, 0.0],
                };
                self.write_params(&params);

                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("layer-canvas-blend-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            depth_slice: None,
                            view: &dst_view,
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
                    pass.set_pipeline(&self.blend_pipeline);
                    pass.set_bind_group(0, &scene_bg, &[]);
                    pass.draw(0..3, 0..1);
                }

                cur ^= 1;
            }
            if let Some(a) = self.accum.as_ref() {
                a.current.store(cur, Ordering::Relaxed);
            }
            self.last_hash.store(hash, Ordering::Relaxed);
        }

        // --- PASS DE PRÉSENTATION (écran) ---
        let acc = self.accum.as_ref().unwrap();
        let final_view = acc.views[acc.current.load(Ordering::Relaxed)].clone();
        let acc_bg = self.scene_bg(&final_view, None);

        let (sel_pos, sel_size) = match prim.selection {
            Some(r) => ([r.x, r.y, 0.0, 0.0], [r.width, r.height, 0.0, 0.0]),
            None => ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0]),
        };
        let params = Params {
            screen_doc: [prim.viewport.0, prim.viewport.1, prim.doc_size.0, prim.doc_size.1],
            pan_zoom: [prim.pan.x, prim.pan.y, prim.zoom, 1.0],
            mode_sizes: [0, 0, 0, if prim.has_doc { 1 } else { 0 }],
            off_sel: sel_pos,
            sel_size,
        };
        self.write_params(&params);

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
        pass.set_bind_group(0, &acc_bg, &[]);
        pass.draw(0..3, 0..1);
    }
}
