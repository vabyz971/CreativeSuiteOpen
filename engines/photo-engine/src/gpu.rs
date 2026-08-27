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

//! GPU via WGPU 30 - Compute shaders pour nœuds + détection
//! Rendu UI déjà WGPU via iced_wgpu, ce module ajoute le TRAITEMENT GPU

use image::DynamicImage;
use std::mem::{align_of, size_of};
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
    pipeline_bc_tex: wgpu::ComputePipeline,
    uniforms_bc_tex: wgpu::Buffer,
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

        // Chemin texture -> texture (zéro readback) : module + pipeline + uniform persistant
        let module_bc_tex = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bc_tex"),
            source: wgpu::ShaderSource::Wgsl(SHADER_BC_TEX.into()),
        });
        let pipeline_bc_tex = create_pipeline(&device, &module_bc_tex);
        let uniforms_bc_tex = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bc_tex_uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Some(Self {
            device,
            queue,
            adapter_info,
            pipeline_bc,
            pipeline_sat,
            pipeline_blur,
            pipeline_blend,
            pipeline_bc_tex,
            uniforms_bc_tex,
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
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
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
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        s.push_str(&format!("CPU cores: {} (rayon)\n", cores));
        s.push_str(&format!(
            "Rayon threads: {}\n",
            rayon::current_num_threads()
        ));
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
    // BUG: panic only if caller provides inconsistent w/h vs floats length — indicates logic error upstream
    let buf = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(w, h, raw)
        .expect("floats_to_image: dimensions invalides");
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
    let src_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
    let params_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
    let Ok(map_res) = rx.recv() else {
        return None;
    };
    if map_res.is_err() {
        return None;
    }
    let Ok(mapped) = readback.slice(..).get_mapped_range() else {
        return None;
    };
    let data = mapped.to_vec();
    drop(mapped);
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

pub fn apply_brightness_contrast_gpu(
    img: &DynamicImage,
    brightness: f32,
    contrast: f32,
) -> Option<DynamicImage> {
    let gpu = GpuContext::get()?;
    // Optimisation : petites images < 256x256 restent CPU (overhead GPU)
    if img.width() * img.height() < 65536 {
        return None;
    }
    let (floats, w, h) = image_to_floats(img);
    // Convertit brightness/contrast CPU -> GPU
    // CPU: b = brightness*2.55, contrast_factor = 1+contrast/100 ou 1+contrast/50
    let contrast_factor = if contrast < 0.0 {
        1.0 + contrast / 100.0
    } else {
        1.0 + contrast / 50.0
    };
    let brightness_norm = brightness * 0.01; // 2.55/255 =0.01
    // Bandes de lignes : supporte les très grandes résolutions sans dépasser les limites wgpu
    let out = run_compute_banded(&gpu, &gpu.pipeline_bc, &floats, w, h, |bw, bh| {
        bytemuck::bytes_of(&ParamsBc {
            width: bw,
            height: bh,
            brightness: brightness_norm,
            contrast: contrast_factor,
        })
        .to_vec()
    })?;
    Some(floats_to_image(&out, w, h))
}

pub fn apply_saturation_gpu(img: &DynamicImage, sat: f32) -> Option<DynamicImage> {
    let gpu = GpuContext::get()?;
    if img.width() * img.height() < 65536 {
        return None;
    }
    let (floats, w, h) = image_to_floats(img);
    let sat_clamped = sat.clamp(0.0, 3.0);
    let out = run_compute_banded(&gpu, &gpu.pipeline_sat, &floats, w, h, |bw, bh| {
        bytemuck::bytes_of(&ParamsSat {
            width: bw,
            height: bh,
            sat: sat_clamped,
            _pad: 0.0,
        })
        .to_vec()
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
    if radius <= 0.1 {
        return Some(img.clone());
    }
    if img.width() * img.height() < 65536 {
        return None;
    }
    let r = (radius as i32).clamp(1, 50);
    let (floats, w, h) = image_to_floats(img);
    let params = ParamsBlur {
        width: w,
        height: h,
        radius: r,
        _pad: 0,
    };
    let out = run_compute(
        &gpu,
        &gpu.pipeline_blur,
        &floats,
        w,
        h,
        bytemuck::bytes_of(&params),
    )?;
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

    let buf_a = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("srcA"),
            contents: bytemuck::cast_slice(src_a),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
    let buf_b = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("srcB"),
            contents: bytemuck::cast_slice(src_b),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
    let dst = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dst"),
        size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let params_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: params_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bg2"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buf_a.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: buf_b.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: dst.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("enc2"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("compute2"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
    }
    encoder.copy_buffer_to_buffer(&dst, 0, &readback, 0, size);
    gpu.queue.submit(Some(encoder.finish()));
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
    if rx.recv().ok()?.is_err() {
        return None;
    }
    let data = readback.slice(..).get_mapped_range().ok()?.to_vec();
    readback.unmap();
    let floats: Vec<f32> = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
    if floats.len() != src_a.len() {
        return None;
    }
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
    if base.width() * base.height() < 65536 {
        return None;
    }
    let (a_floats, w, h) = image_to_floats(base);
    let (mut b_floats, w2, h2) = image_to_floats(top);
    if w != w2 || h != h2 {
        // resize_exact : `resize` préserve le ratio et donnerait une taille
        // différente de w×h → slices hors bornes plus bas (crash).
        let t_img = floats_to_image(&b_floats, w2, h2).resize_exact(
            w,
            h,
            ::image::imageops::FilterType::Triangle,
        );
        b_floats = image_to_floats(&t_img).0;
    }
    // Garde-fou : tailles strictement identiques, sinon fallback CPU
    if b_floats.len() != a_floats.len() {
        return None;
    }
    let op = (opacity_pct / 100.0).clamp(0.0, 1.0);

    let row_bytes = (w as u64) * 16;
    if row_bytes == 0 {
        return None;
    }
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
            &gpu,
            &gpu.pipeline_blend,
            &a_floats[lo..hi],
            &b_floats[lo..hi],
            w,
            rows as u32,
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

/// Brightness/contrast en cheminement TEXTURE -> STORAGE TEXTURE : les pixels
/// ne quittent jamais la VRAM (aucun readback CPU). Les dimensions sont lues
/// directement depuis la texture d'entrée (pas de désynchronisation possible).
/// NB : pas d'affectation par swizzle multiple (`c.rgb = …`) — non supportée
/// par naga ; on passe par une variable intermédiaire.
const SHADER_BC_TEX: &str = r#"
struct Uniforms { brightness: f32, contrast: f32 };
@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> u: Uniforms;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(input_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }
    let c = textureLoad(input_tex, gid.xy, 0);
    var rgb = c.rgb;
    rgb = (rgb - vec3<f32>(0.5)) * u.contrast + vec3<f32>(0.5) + vec3<f32>(u.brightness);
    // Alpha préservé, rgb borné (rgba8unorm sature aussi à l'écriture)
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    textureStore(output_tex, gid.xy, vec4<f32>(rgb, c.a));
}
"#;

// ---------------------------------------------------------------------------
// Filtre texture -> texture (zéro readback)
//
// Contrairement au chemin storage-buffer ci-dessus (aller-retour CPU <-> GPU),
// ce pipeline lit une texture et écrit dans une autre : les pixels ne quittent
// JAMAIS la VRAM. Compatible modèle « state-only » : changer un réglage se
// réduit à queue.write_buffer + rediffusion, sans ré-upload des pixels.
// ---------------------------------------------------------------------------

/// Format commun aux textures d'entrée/sortie du filtre.
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Taille du workgroup du filtre : 16x16 pixels par groupe de travail.
pub const WORKGROUP_SIZE: u32 = 16;

/// Paramètres du filtre, envoyés tels quels via un uniform buffer.
///
/// Plages applicatives : `brightness` dans [-1.0 ; 1.0], `contrast` dans [0.0 ; 4.0].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub brightness: f32,
    pub contrast: f32,
}

impl Uniforms {
    /// Crée des paramètres bornés : brightness [-1.0 ; 1.0], contrast [0.0 ; 4.0].
    #[must_use]
    pub fn new(brightness: f32, contrast: f32) -> Self {
        Self {
            brightness: brightness.clamp(-1.0, 1.0),
            contrast: contrast.clamp(0.0, 4.0),
        }
    }
}

impl Default for Uniforms {
    /// Réglage neutre : aucune transformation.
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 1.0,
        }
    }
}

// Verrous de compilation : le layout mémoire doit refléter le struct WGSL à l'octet
const _: () = assert!(size_of::<Uniforms>() == 8);
const _: () = assert!(align_of::<Uniforms>() == 4);

/// Nombre de workgroups nécessaire pour couvrir `width x height` pixels.
#[must_use]
pub fn workgroup_count(width: u32, height: u32) -> (u32, u32) {
    (
        width.div_ceil(WORKGROUP_SIZE),
        height.div_ceil(WORKGROUP_SIZE),
    )
}

/// Construit le pipeline de calcul à partir d'un module déjà compilé.
#[must_use]
pub fn create_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
) -> wgpu::ComputePipeline {
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("bc_tex"),
        layout: None,
        module: shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

/// Assemble le bind group : texture d'entrée (lecture), texture de sortie
/// (écriture storage), uniform buffer de réglages.
#[must_use]
pub fn create_bind_group(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    input_view: &wgpu::TextureView,
    output_view: &wgpu::TextureView,
    uniforms: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bc_tex_bg"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(input_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(output_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniforms.as_entire_binding(),
            },
        ],
    })
}

/// Enregistre la diffusion du filtre dans `encoder` (aucune soumission ici :
/// l'appelant garde la main sur le batching de ses passes).
pub fn dispatch_filter(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    bind_group: &wgpu::BindGroup,
    width: u32,
    height: u32,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("bc_tex_dispatch"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    let (wx, wy) = workgroup_count(width, height);
    pass.dispatch_workgroups(wx, wy, 1);
}

/// Pas d'une ligne une fois alignée sur COPY_BYTES_PER_ROW_ALIGNMENT (256 octets),
/// exigence de wgpu pour write_texture/copy_texture_to_texture.
fn row_pitch(width: u32) -> u32 {
    let raw = width * 4;
    raw.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

impl GpuContext {
    /// Crée une texture RGBA8 prête à recevoir un upload CPU (usage échantillonnage + copie).
    #[must_use]
    pub fn create_input_texture(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bc_tex_in"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    /// Convertit `img` en RGBA8 puis la charge dans une texture GPU.
    /// Unique passage par la RAM système ; ensuite tout reste en VRAM.
    #[must_use]
    pub fn upload_image(&self, img: &DynamicImage) -> wgpu::Texture {
        let rgba = img.to_rgba8();
        let texture = self.create_input_texture(rgba.width(), rgba.height());
        let _ = self.upload_rgba8(&texture, &rgba);
        texture
    }

    /// Écrit des pixels RGBA8 dans `texture` en gérant le rembourrage de lignes
    /// imposé par wgpu (pitch aligné 256 octets).
    ///
    /// Retourne `false` si `rgba` ne contient pas exactement largeur x hauteur x 4 octets.
    pub fn upload_rgba8(&self, texture: &wgpu::Texture, rgba: &[u8]) -> bool {
        let size = texture.size();
        let expected = size.width as usize * size.height as usize * 4;
        if expected == 0 || rgba.len() != expected {
            return false;
        }
        let pitch = row_pitch(size.width);
        let layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(pitch),
            rows_per_image: None,
        };
        if pitch == size.width * 4 {
            // Lignes déjà alignées : copie directe zéro transformation
            self.queue
                .write_texture(texture.as_image_copy(), rgba, layout, size);
        } else {
            let stride = size.width as usize * 4;
            let mut padded = vec![0u8; pitch as usize * size.height as usize];
            for (dst, src) in padded
                .chunks_exact_mut(pitch as usize)
                .zip(rgba.chunks_exact(stride))
            {
                dst[..stride].copy_from_slice(src);
            }
            self.queue
                .write_texture(texture.as_image_copy(), &padded, layout, size);
        }
        true
    }

    /// Applique luminosité/contraste : lit `input`, retourne une NOUVELLE texture.
    ///
    /// ZÉRO readback : les pixels restent en VRAM, la sortie est directement
    /// chaînable vers un autre filtre ou un render pass.
    ///
    /// Le réglage est poussé dans l'uniform buffer persistant du contexte ;
    /// l'ordre des opérations de file garantit que chaque diffusion lit bien la
    /// valeur écrite juste avant elle.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use photo_engine::gpu::{GpuContext, Uniforms};
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let gpu = GpuContext::get().ok_or("GPU indisponible")?;
    ///     let input = gpu.upload_image(&image::DynamicImage::new_rgba8(1920, 1080));
    ///     let output = gpu
    ///         .apply_brightness_contrast_texture(&input, Uniforms::new(0.1, 1.2))
    ///         .ok_or("dimensions invalides")?;
    ///     // `output` reste en VRAM : prête pour un autre filtre ou l'affichage.
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Retourne `None` si les dimensions sont nulles ou si le contexte est indisponible.
    pub fn apply_brightness_contrast_texture(
        &self,
        input: &wgpu::Texture,
        uniforms: Uniforms,
    ) -> Option<wgpu::Texture> {
        let size = input.size();
        if size.width == 0 || size.height == 0 {
            return None;
        }
        let output = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bc_tex_out"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let in_view = input.create_view(&wgpu::TextureViewDescriptor::default());
        let out_view = output.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = create_bind_group(
            &self.device,
            &self.pipeline_bc_tex,
            &in_view,
            &out_view,
            &self.uniforms_bc_tex,
        );
        self.queue
            .write_buffer(&self.uniforms_bc_tex, 0, bytemuck::bytes_of(&uniforms));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bc_tex_enc"),
            });
        dispatch_filter(
            &mut encoder,
            &self.pipeline_bc_tex,
            &bind_group,
            size.width,
            size.height,
        );
        self.queue.submit(Some(encoder.finish()));
        Some(output)
    }
}

// Pour compat avec ancien code qui appelle evaluate_gpu_available
pub fn evaluate_gpu_available() -> bool {
    GpuContext::is_available()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniforms_occupe_huit_octets_comme_en_wgsl() {
        assert_eq!(size_of::<Uniforms>(), 8);
        assert_eq!(align_of::<Uniforms>(), 4);
    }

    #[test]
    fn uniforms_par_defaut_sont_neutres() {
        let u = Uniforms::default();
        assert!((u.brightness - 0.0).abs() < f32::EPSILON);
        assert!((u.contrast - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn uniforms_new_borne_les_plages() {
        let hors_plage = Uniforms::new(-5.0, 42.0);
        assert!((hors_plage.brightness - (-1.0)).abs() < f32::EPSILON);
        assert!((hors_plage.contrast - 4.0).abs() < f32::EPSILON);
        let aux_limites = Uniforms::new(1.0, 0.0);
        assert!((aux_limites.brightness - 1.0).abs() < f32::EPSILON);
        assert!((aux_limites.contrast - 0.0).abs() < f32::EPSILON);
        let valide = Uniforms::new(0.25, 1.5);
        assert!((valide.brightness - 0.25).abs() < f32::EPSILON);
        assert!((valide.contrast - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn nb_workgroups_couvre_toute_la_surface() {
        assert_eq!(workgroup_count(1, 1), (1, 1));
        assert_eq!(workgroup_count(WORKGROUP_SIZE, WORKGROUP_SIZE), (1, 1));
        assert_eq!(
            workgroup_count(WORKGROUP_SIZE + 1, WORKGROUP_SIZE + 1),
            (2, 2)
        );
        assert_eq!(workgroup_count(1600, 1200), (100, 75));
        assert_eq!(workgroup_count(0, 0), (0, 0));
    }

    #[test]
    fn pas_de_ligne_aligne_sur_256_octets() {
        // 64 px * 4 = 256 octets : déjà multiple de 256
        assert_eq!(row_pitch(64), 256);
        // 640 px * 4 = 2560 octets : déjà multiple de 256
        assert_eq!(row_pitch(640), 2560);
        // 800 px * 4 = 3200 octets : arrondi au multiple de 256 supérieur
        assert_eq!(row_pitch(800), 3328);
        assert_eq!(row_pitch(1), 256);
    }
}
