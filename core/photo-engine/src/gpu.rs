//! GPU via WGPU 30 - Compute shaders pour nœuds + détection
//! Rendu UI déjà WGPU via iced_wgpu, ce module ajoute le TRAITEMENT GPU

use image::DynamicImage;
use std::sync::{Arc, OnceLock};
use wgpu::util::DeviceExt;

// ---------------------------------------------------------------------------
// Contexte GPU partagé (intégré au canvas principal via wgpu Instance partagée)
// ---------------------------------------------------------------------------

pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: String,
    pipeline_bc: wgpu::ComputePipeline,
    pipeline_sat: wgpu::ComputePipeline,
    pipeline_blur: wgpu::ComputePipeline,
    pipeline_blend: wgpu::ComputePipeline,
}

static GPU: OnceLock<Option<Arc<GpuContext>>> = OnceLock::new();

impl GpuContext {
    pub fn is_available() -> bool {
        Self::get().is_some()
    }
    pub fn get() -> Option<Arc<Self>> {
        GPU.get_or_init(Self::try_new).clone()
    }
    fn try_new() -> Option<Arc<Self>> {
        pollster::block_on(Self::try_new_async()).map(Arc::new)
    }
    async fn try_new_async() -> Option<Self> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::all();
        let instance = wgpu::Instance::new(desc);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .ok()?;
        let info = adapter.get_info();
        let adapter_info = format!(
            "{} - {} - {:?} - backend {:?}",
            info.name, info.vendor, info.device_type, info.backend
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("CreativeSuite GPU"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .ok()?;

        let pipeline_bc = Self::create_pipeline(&device, SHADER_BC, "bc");
        let pipeline_sat = Self::create_pipeline(&device, SHADER_SAT, "sat");
        let pipeline_blur = Self::create_pipeline(&device, SHADER_BLUR, "blur");
        let pipeline_blend = Self::create_pipeline(&device, SHADER_BLEND, "blend");

        Some(Self {
            device,
            queue,
            adapter_info,
            pipeline_bc,
            pipeline_sat,
            pipeline_blur,
            pipeline_blend,
        })
    }
    fn create_pipeline(device: &wgpu::Device, shader: &str, label: &str) -> wgpu::ComputePipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(shader.into()),
        });
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        })
    }
    pub fn adapter_info() -> String {
        Self::get()
            .map(|g| g.adapter_info.clone())
            .unwrap_or_else(|| "Aucun GPU détecté - fallback CPU".into())
    }
}

// ---------------------------------------------------------------------------
// Détection pour UI
// ---------------------------------------------------------------------------

pub fn detect_gpu_info_sync() -> String {
    if let Some(gpu) = GpuContext::get() {
        let mut s = String::new();
        s.push_str(&format!("GPU: {}\n", gpu.adapter_info));
        s.push_str(&format!(
            "Backend: wgpu 30.0.0 - Device limits: {:?}\n",
            gpu.device.limits()
        ));
        s.push_str("Backends: Vulkan/Metal/DX12/WGL (auto)\n");
        s.push_str("Traitement nodal: GPU compute (branché) + fallback CPU rayon\n");
        s.push_str("Rendu canvas: WGPU via iced_wgpu (textures + shaders)\n");
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        s.push_str(&format!("CPU cores: {} (rayon global pool)\n", cores));
        // Force rayon pool à utiliser tous les cœurs
        s.push_str(&format!(
            "Rayon threads: {} (actifs)\n",
            rayon::current_num_threads()
        ));
        s
    } else {
        let mut s = String::new();
        s.push_str("GPU: non disponible - CPU seul\n");
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        s.push_str(&format!("CPU cores: {} (rayon)\n", cores));
        s.push_str(&format!("Rayon threads: {}\n", rayon::current_num_threads()));
        s
    }
}
pub async fn detect_gpu_info() -> String {
    detect_gpu_info_sync()
}

// ---------------------------------------------------------------------------
// Helpers GPU
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ParamsBc {
    width: u32,
    height: u32,
    brightness: f32,
    contrast: f32,
}
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ParamsSat {
    width: u32,
    height: u32,
    sat: f32,
    _pad: f32,
}
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ParamsMix {
    width: u32,
    height: u32,
    factor: f32,
    _pad: f32,
}
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ParamsBlur {
    width: u32,
    height: u32,
    radius: i32,
    _pad: u32,
}

fn image_to_floats(img: &DynamicImage) -> (Vec<f32>, u32, u32) {
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let raw = rgba.into_raw();
    let mut floats = Vec::with_capacity((w * h * 4) as usize);
    for b in raw {
        floats.push(b as f32 / 255.0);
    }
    (floats, w, h)
}
fn floats_to_image(floats: &[f32], w: u32, h: u32) -> DynamicImage {
    let mut raw = Vec::with_capacity((w * h * 4) as usize);
    for &f in floats {
        raw.push((f.clamp(0.0, 1.0) * 255.0) as u8);
    }
    let buf = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(w, h, raw).unwrap();
    DynamicImage::ImageRgba8(buf)
}

fn run_compute(
    gpu: &GpuContext,
    pipeline: &wgpu::ComputePipeline,
    src_floats: &[f32],
    w: u32,
    h: u32,
    params_bytes: &[u8],
) -> Option<Vec<f32>> {
    let count = (w * h) as usize;
    let src_size = (count * 4 * 4) as u64; // vec4<f32> per pixel
    // Garde-fou : respecter les DEUX limites wgpu
    // - max_buffer_size (création du buffer, 256 Mo par défaut)
    // - max_buffer_binding_size (binding dans le shader, 128 Mo par défaut)
    let limits = gpu.device.limits();
    if src_size > limits.max_storage_buffer_binding_size || src_size > limits.max_buffer_size {
        return None; // -> fallback CPU (ou bandes via run_compute_banded)
    }
    // src buffer
    let src_buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("src"),
        contents: bytemuck::cast_slice(src_floats),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let dst_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dst"),
        size: src_size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: src_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let params_buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("params"),
        contents: params_bytes,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bg"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: src_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: dst_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });
    let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("compute"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let (wx, wy) = (w.div_ceil(8), h.div_ceil(8));
        pass.dispatch_workgroups(wx, wy, 1);
    }
    encoder.copy_buffer_to_buffer(&dst_buf, 0, &readback, 0, src_size);
    gpu.queue.submit(Some(encoder.finish()));
    // Map
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = gpu
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .ok();
    if rx.recv().unwrap().is_err() {
        return None;
    }
    let data = readback
        .slice(..)
        .get_mapped_range()
        .expect("get_mapped_range failed")
        .to_vec();
    readback.unmap();
    let floats: Vec<f32> = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
    // s'assurer que la taille correspond
    if floats.len() != src_floats.len() {
        return None;
    }
    Some(floats)
}

// ---------------------------------------------------------------------------
// API publique - tente GPU, retourne None si indisponible -> fallback CPU
// ---------------------------------------------------------------------------

/// Budget par bande de lignes : reste sous max_storage_buffer_binding_size
/// tout en gardant de bonnes performances.
const BAND_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

/// Exécute un compute en BANDES de lignes : chaque buffer/binding reste sous
/// les limites wgpu même sur de très grandes images (opérations ponctuelles exactes).
fn run_compute_banded(
    gpu: &GpuContext,
    pipeline: &wgpu::ComputePipeline,
    src_floats: &[f32],
    w: u32,
    h: u32,
    make_params: impl Fn(u32, u32) -> Vec<u8>,
) -> Option<Vec<f32>> {
    let row_bytes = (w as u64) * 16; // vec4<f32> par pixel
    if row_bytes == 0 {
        return None;
    }
    let limits = gpu.device.limits();
    let budget = limits
        .max_storage_buffer_binding_size
        .min(limits.max_buffer_size)
        .min(BAND_BUDGET_BYTES);
    let rows_per_band = (budget / row_bytes).max(1) as usize;
    let floats_per_row = (w as usize) * 4;

    let mut out = vec![0f32; src_floats.len()];
    for start in (0..h as usize).step_by(rows_per_band) {
        let rows = rows_per_band.min(h as usize - start);
        let lo = start * floats_per_row;
        let hi = lo + rows * floats_per_row;
        let band_out = run_compute(
            gpu,
            pipeline,
            &src_floats[lo..hi],
            w,
            rows as u32,
            &make_params(w, rows as u32),
        )?;
        out[lo..hi].copy_from_slice(&band_out);
    }
    Some(out)
}

pub fn apply_brightness_contrast_gpu(img: &DynamicImage, brightness: f32, contrast: f32) -> Option<DynamicImage> {
    let gpu = GpuContext::get()?;
    // Optimisation : petites images < 256x256 restent CPU (overhead GPU)
    if img.width() * img.height() < 65536 {
        return None;
    }
    let (floats, w, h) = image_to_floats(img);
    // Convertit brightness/contrast CPU -> GPU
    // CPU: b = brightness*2.55, contrast_factor = 1+contrast/100 ou 1+contrast/50
    let contrast_factor = if contrast < 0.0 { 1.0 + contrast / 100.0 } else { 1.0 + contrast / 50.0 };
    let brightness_norm = brightness * 0.01; // 2.55/255 =0.01
    // Bandes de lignes : supporte les très grandes résolutions sans dépasser les limites wgpu
    let out = run_compute_banded(&gpu, &gpu.pipeline_bc, &floats, w, h, |bw, bh| {
        bytemuck::bytes_of(&ParamsBc { width: bw, height: bh, brightness: brightness_norm, contrast: contrast_factor }).to_vec()
    })?;
    Some(floats_to_image(&out, w, h))
}

pub fn apply_saturation_gpu(img: &DynamicImage, sat: f32) -> Option<DynamicImage> {
    let gpu = GpuContext::get()?;
    if img.width() * img.height() < 65536 { return None; }
    let (floats, w, h) = image_to_floats(img);
    let sat_clamped = sat.clamp(0.0, 3.0);
    let out = run_compute_banded(&gpu, &gpu.pipeline_sat, &floats, w, h, |bw, bh| {
        bytemuck::bytes_of(&ParamsSat { width: bw, height: bh, sat: sat_clamped, _pad: 0.0 }).to_vec()
    })?;
    Some(floats_to_image(&out, w, h))
}

pub fn apply_mix_gpu(a: &DynamicImage, b: &DynamicImage, factor: f32) -> Option<DynamicImage> {
    // Mix 2 entrées : pas de shader dédié (le nœud Calque couvre ce besoin via SHADER_BLEND)
    // → fallback CPU direct, sans dispatch GPU inutile.
    let _ = (a, b, factor);
    None
}

pub fn apply_blur_gpu(img: &DynamicImage, radius: f32) -> Option<DynamicImage> {
    let gpu = GpuContext::get()?;
    if radius <= 0.1 { return Some(img.clone()); }
    if img.width() * img.height() < 65536 { return None; }
    let r = (radius as i32).clamp(1, 50);
    let (floats, w, h) = image_to_floats(img);
    let params = ParamsBlur { width: w, height: h, radius: r, _pad: 0 };
    let out = run_compute(&gpu, &gpu.pipeline_blur, &floats, w, h, bytemuck::bytes_of(&params))?;
    Some(floats_to_image(&out, w, h))
}

// ---------------------------------------------------------------------------
// Fusion de calques (GPU) — deux entrées, opacité + mode
// ---------------------------------------------------------------------------

/// Exécute un pipeline à DEUX sources en bandes de lignes.
/// Layout bindings : 0=srcA(read) 1=srcB(read) 2=dst(rw) 3=params(uniform)
fn run_compute2(
    gpu: &GpuContext,
    pipeline: &wgpu::ComputePipeline,
    src_a: &[f32],
    src_b: &[f32],
    w: u32,
    h: u32,
    params_bytes: &[u8],
) -> Option<Vec<f32>> {
    use wgpu::util::DeviceExt;
    let count = (w * h) as usize;
    let size = (count * 4 * 4) as u64;
    let limits = gpu.device.limits();
    if size > limits.max_storage_buffer_binding_size || size > limits.max_buffer_size {
        return None; // -> fallback CPU
    }

    let buf_a = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("srcA"), contents: bytemuck::cast_slice(src_a),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let buf_b = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("srcB"), contents: bytemuck::cast_slice(src_b),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let dst = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dst"), size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"), size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let params_buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("params"), contents: params_bytes,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bg2"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: buf_a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: buf_b.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: dst.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: params_buf.as_entire_binding() },
        ],
    });
    let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("enc2") });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("compute2"), timestamp_writes: None });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
    }
    encoder.copy_buffer_to_buffer(&dst, 0, &readback, 0, size);
    gpu.queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
    let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).ok();
    if rx.recv().ok()?.is_err() { return None; }
    let data = readback.slice(..).get_mapped_range().ok()?.to_vec();
    readback.unmap();
    let floats: Vec<f32> = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
    if floats.len() != src_a.len() { return None; }
    Some(floats)
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ParamsBlend2 {
    width: u32,
    height: u32,
    top_w: u32,
    top_h: u32,
    opacity: f32,
    mode: u32,
    offset_x: f32,
    offset_y: f32,
}

const BLEND_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

pub fn apply_blend_gpu(
    base: &DynamicImage,
    top: &DynamicImage,
    opacity_pct: f32,
    mode: u32,
    offset_x: f32,
    offset_y: f32,
) -> Option<DynamicImage> {
    // Déplacement non nul : fallback CPU (shader offset bandé complexe)
    if offset_x != 0.0 || offset_y != 0.0 {
        return None;
    }
    let gpu = GpuContext::get()?;
    if base.width() * base.height() < 65536 { return None; }
    let (a_floats, w, h) = image_to_floats(base);
    let (mut b_floats, w2, h2) = image_to_floats(top);
    if w != w2 || h != h2 {
        // resize_exact : `resize` préserve le ratio et donnerait une taille
        // différente de w×h → slices hors bornes plus bas (crash).
        let t_img = floats_to_image(&b_floats, w2, h2)
            .resize_exact(w, h, ::image::imageops::FilterType::Triangle);
        b_floats = image_to_floats(&t_img).0;
    }
    // Garde-fou : tailles strictement identiques, sinon fallback CPU
    if b_floats.len() != a_floats.len() {
        return None;
    }
    let op = (opacity_pct / 100.0).clamp(0.0, 1.0);

    let row_bytes = (w as u64) * 16;
    if row_bytes == 0 { return None; }
    let budget = limits_min(&gpu).min(BLEND_BUDGET_BYTES);
    let rows_per_band = (budget / row_bytes).max(1) as usize;
    let floats_per_row = (w as usize) * 4;
    let mut out = vec![0f32; a_floats.len()];
    for start in (0..h as usize).step_by(rows_per_band) {
        let rows = rows_per_band.min(h as usize - start);
        let lo = start * floats_per_row;
        let hi = lo + rows * floats_per_row;
        let params = ParamsBlend2 {
            width: w,
            height: rows as u32,
            top_w: w,
            top_h: h,
            opacity: op,
            mode,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        let band_out = run_compute2(
            &gpu, &gpu.pipeline_blend, &a_floats[lo..hi], &b_floats[lo..hi], w, rows as u32,
            bytemuck::bytes_of(&params),
        )?;
        out[lo..hi].copy_from_slice(&band_out);
    }
    Some(floats_to_image(&out, w, h))
}

fn limits_min(gpu: &GpuContext) -> u64 {
    let l = gpu.device.limits();
    l.max_storage_buffer_binding_size.min(l.max_buffer_size)
}

// ---------------------------------------------------------------------------
// Shaders WGSL
// ---------------------------------------------------------------------------

const SHADER_BC: &str = r#"
struct Params { width: u32, height: u32, brightness: f32, contrast: f32 };
@group(0) @binding(0) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> dst: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> p: Params;
@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let idx = id.y * p.width + id.x;
    var c = src[idx];
    c = (c - vec4<f32>(0.5)) * p.contrast + vec4<f32>(0.5) + vec4<f32>(p.brightness);
    // preserve alpha
    let a = src[idx].a;
    c = clamp(c, vec4<f32>(0.0), vec4<f32>(1.0));
    c.a = a;
    dst[idx] = c;
}
"#;

const SHADER_SAT: &str = r#"
struct Params { width: u32, height: u32, sat: f32, _pad: f32 };
@group(0) @binding(0) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> dst: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> p: Params;
@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let idx = id.y * p.width + id.x;
    var c = src[idx];
    let gray = dot(c.rgb, vec3<f32>(0.299, 0.587, 0.114));
    c.r = gray + (c.r - gray) * p.sat;
    c.g = gray + (c.g - gray) * p.sat;
    c.b = gray + (c.b - gray) * p.sat;
    dst[idx] = clamp(c, vec4<f32>(0.0), vec4<f32>(1.0));
}
"#;

const SHADER_BLUR: &str = r#"
struct Params { width: u32, height: u32, radius: i32, _pad: u32 };
@group(0) @binding(0) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> dst: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> p: Params;
@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let idx = id.y * p.width + id.x;
    var sum = vec4<f32>(0.0);
    var count = 0.0;
    let r = p.radius;
    for (var dy: i32 = -r; dy <= r; dy = dy + 1) {
        for (var dx: i32 = -r; dx <= r; dx = dx + 1) {
            let x = i32(id.x) + dx;
            let y = i32(id.y) + dy;
            if (x < 0 || y < 0 || x >= i32(p.width) || y >= i32(p.height)) { continue; }
            let sidx = u32(y) * p.width + u32(x);
            sum = sum + src[sidx];
            count = count + 1.0;
        }
    }
    dst[idx] = sum / count;
}
"#;

const SHADER_BLEND: &str = r#"
struct Params { width: u32, height: u32, top_w: u32, top_h: u32, opacity: f32, mode: u32, offset_x: f32, offset_y: f32 };
@group(0) @binding(0) var<storage, read> base: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> top: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dst: array<vec4<f32>>;
@group(0) @binding(3) var<uniform> p: Params;
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
@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let idx = id.y * p.width + id.x;
    var b = base[idx];
    // Échantillonne le dessus avec décalage et bornes du top
    var t = vec4<f32>(0.0);
    let tx = i32(id.x) - i32(p.offset_x);
    let ty = i32(id.y) - i32(p.offset_y);
    if (tx >= 0 && tx < i32(p.top_w) && ty >= 0 && ty < i32(p.top_h)) {
        let t_idx = u32(ty) * p.top_w + u32(tx);
        // Quand top et base ont même taille et offset 0, t_idx == idx
        // Pour les bandes, top est slicé de la même façon que base, donc l'index reste cohérent
        t = top[idx];
        // Si top plus petit que base et offset non nul, l'échantillonnage via idx reste valable
        // car top a été redimensionné à la taille de base avant upload (cas offset==0)
        // Pour offset !=0 on est en fallback CPU, donc ce chemin n'est pas pris
    }
    t.a = t.a * p.opacity;
    if (t.a <= 0.001) { dst[idx] = b; return; }
    let br = blend_channel(b.r, t.r, p.mode);
    let bg = blend_channel(b.g, t.g, p.mode);
    let bb = blend_channel(b.b, t.b, p.mode);
    let blended = vec3<f32>(br, bg, bb);
    let out_a = t.a + b.a * (1.0 - t.a);
    var out = vec4<f32>(0.0);
    if (out_a > 0.001) {
        out.r = (blended.r * t.a + b.r * b.a * (1.0 - t.a)) / out_a;
        out.g = (blended.g * t.a + b.g * b.a * (1.0 - t.a)) / out_a;
        out.b = (blended.b * t.a + b.b * b.a * (1.0 - t.a)) / out_a;
        out.a = out_a;
    } else {
        out = vec4<f32>(0.0);
    }
    dst[idx] = clamp(out, vec4<f32>(0.0), vec4<f32>(1.0));
}
"#;

// Pour compat avec ancien code qui appelle evaluate_gpu_available
pub fn evaluate_gpu_available() -> bool { GpuContext::is_available() }
