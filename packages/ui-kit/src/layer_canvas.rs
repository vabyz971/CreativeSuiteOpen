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

use crate::image_canvas::{CanvasTool, ImageCanvasEvent};

// ---------------------------------------------------------------------------
// Displayed model
// ---------------------------------------------------------------------------

/// A layer ready for GPU upload (preconverted shared RGBA8 pixels).
#[derive(Clone, Debug)]
pub struct DisplayLayer {
    /// Content identity (texture cache key, e.g. Arc pointer)
    pub key: u64,
    pub rgba: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    /// Opacity 0..1
    pub opacity: f32,
    /// Blend mode (0 Normal ... 5 Lighten)
    pub blend: u32,
    pub offset_x: f32,
    pub offset_y: f32,
}

pub struct LayerCanvas<Message> {
    pub layers: Vec<DisplayLayer>,
    /// Document dimensions in pixels (None = no document)
    pub doc_size: Option<(f32, f32)>,
    pub pan: Vector,
    pub zoom: f32,
    pub tool: CanvasTool,
    pub selection: Option<Rectangle>,
    /// Convert canvas events to app messages
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

    #[must_use]
    pub fn with_layers(mut self, layers: Vec<DisplayLayer>) -> Self {
        self.layers = layers;
        self
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
            if state.dragging.take().is_some() && self.tool == CanvasTool::Move {
                return Some(
                    shader::Action::publish((self.on_event)(ImageCanvasEvent::MoveLayerEnd))
                        .and_capture(),
                );
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
                // Brush/eraser not supported by experimental GPU path
                CanvasTool::Brush | CanvasTool::Eraser => Some(shader::Action::capture()),
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
                    } else if self.tool == CanvasTool::Move {
                        // Raw screen delta; image pixel conversion happens
                        // on app side with current zoom.
                        return Some(shader::Action::publish((self.on_event)(
                            ImageCanvasEvent::MoveLayer {
                                dx: cursor_pos.x - start.x,
                                dy: cursor_pos.y - start.y,
                            },
                        )));
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
                CanvasTool::Brush | CanvasTool::Eraser => Interaction::Crosshair,
            };
        }
        Interaction::default()
    }
}

// ---------------------------------------------------------------------------
// Primitive GPU
// ---------------------------------------------------------------------------

/// Config hash: if unchanged, blend passes are skipped.
fn config_hash(layers: &[DisplayLayer], doc: (f32, f32)) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let feed = |v: u64, h: &mut u64| {
        *h ^= v;
        *h = h.wrapping_mul(0x100000001b3);
    };
    feed(u64::from(doc.0.to_bits()), &mut h);
    feed(u64::from(doc.1.to_bits()), &mut h);
    feed(layers.len() as u64, &mut h);
    for l in layers {
        feed(l.key, &mut h);
        feed(u64::from(l.opacity.to_bits()), &mut h);
        feed(u64::from(l.blend), &mut h);
        feed(u64::from(l.offset_x.to_bits()), &mut h);
        feed(u64::from(l.offset_y.to_bits()), &mut h);
        feed(u64::from(l.width), &mut h);
        feed(u64::from(l.height), &mut h);
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
        // Blending requires its own offscreen passes → render()
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
    /// xy = layer offset (px document), zw = selection position
    off_sel: [f32; 4],
    /// xy = selection size (x > 0 = active)
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

    /// Persistent layer textures, key = content identity
    layer_textures: HashMap<u64, LayerTex>,
    /// Ping-pong accumulators (document space)
    accum: Option<Accum>,
    /// Hash of last GPU recomposite (atomic: `render()` takes &self)
    last_hash: std::sync::atomic::AtomicU64,
}

struct LayerTex {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
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
    /// Full bind group: tex0 + sampler in base slots, `top` reused
    /// in top slots (unused by present shader).
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

    fn prepare(&mut self, prim: &CompositePrimitive, device: &wgpu::Device, queue: &wgpu::Queue) {
        let doc = (
            prim.doc_size.0.round().max(1.0) as u32,
            prim.doc_size.1.round().max(1.0) as u32,
        );
        self.ensure_accum(doc);

        // Upload new layer versions + eviction of obsolete ones
        let live: Vec<u64> = prim.layers.iter().map(|l| l.key).collect();
        for l in &prim.layers {
            self.layer_textures.entry(l.key).or_insert_with(|| {
                // Capture wgpu validation errors (otherwise silent)
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
                        &l.rgba[..],
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
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                if let Some(err) = pollster::block_on(device.pop_error_scope()) {
                    eprintln!("layer-canvas: échec upload calque {err:#?}");
                }
                LayerTex {
                    view,
                    width: l.width,
                    height: l.height,
                }
            });
        }
        self.layer_textures.retain(|k, _| live.contains(k));

        // Recomposite decided in render(): compare current hash
        let _ = config_hash(&prim.layers, prim.doc_size);
    }

    fn write_params(&self, params: &Params) {
        self.queue
            .write_buffer(&self.params_buf, 0, bytemuck::bytes_of(params));
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

        // --- BLEND PASSES (offscreen, ping-pong) ---
        // Recomposite only if stack changed since last frame
        let hash = config_hash(&prim.layers, prim.doc_size);
        if hash != self.last_hash.load(Ordering::Relaxed) {
            let accum = self.accum.as_ref().expect("accum initialized in prepare");
            let mut cur = accum.current.load(Ordering::Relaxed);

            // Wgpu textures start with undefined content.
            // Clear initial base to transparent before first blend,
            // otherwise first layer would be mixed with random pixels.
            {
                let base_init = self.accum.as_ref().expect("accum initialized").views[cur].clone();
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
                let Some(tex) = self.layer_textures.get(&layer.key) else {
                    continue;
                };

                // Ping-pong: src = previous result, dst = target for this layer
                let accum_ref = self.accum.as_ref().expect("accum initialized");
                let src_view = accum_ref.views[cur].clone();
                let dst_view = accum_ref.views[cur ^ 1].clone();

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

        // --- PRESENTATION PASS (screen) ---
        let acc = self.accum.as_ref().expect("accum initialized for present");
        let final_view = acc.views[acc.current.load(Ordering::Relaxed)].clone();
        let acc_bg = self.scene_bg(&final_view, None);

        let (sel_pos, sel_size) = match prim.selection {
            Some(r) => ([r.x, r.y, 0.0, 0.0], [r.width, r.height, 0.0, 0.0]),
            None => ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0]),
        };
        let params = Params {
            screen_doc: [
                prim.viewport.0,
                prim.viewport.1,
                prim.doc_size.0,
                prim.doc_size.1,
            ],
            pan_zoom: [prim.pan.x, prim.pan.y, prim.zoom, 1.0],
            mode_sizes: [0, 0, 0, u32::from(prim.has_doc)],
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
