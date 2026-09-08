const SHADER: &str = r"
struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct Params {
    screen_doc: vec4<f32>,   // xy = viewport widget px, zw = document px
    pan_zoom: vec4<f32>,     // xy = pan, z = zoom, w = opacite layer
    mode_sizes: vec4<u32>,   // x = blend mode, y/z = top texture dims, w = image flag
    off_sel: vec4<f32>,      // xy = decalage layer, zw = position selection
    sel_size: vec4<f32>,     // xy = size selection (x > 0 = active)
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

// Single bind group (max_bind_groups = 2 limit on iced device)
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
    // Echantillonne le layer a (doc_px - offset), normalise par SES dimensions
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

const GRID_BG = vec3<f32>(0.0549, 0.0549, 0.0549);   // theme::SURFACE_CONTAINER_LOWEST #0E0E0E
const GRID_DOT = vec3<f32>(0.2078, 0.2078, 0.2039);  // theme::SURFACE_CONTAINER_HIGHEST #353534

@fragment
fn fs_present(in: VOut) -> @location(0) vec4<f32> {
    let screen_px = in.uv * pp.screen_doc.xy;
    let pan = pp.pan_zoom.xy;
    let zoom = pp.pan_zoom.z;

    // Solid background — checker removed for performance and resize stability
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
";

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
