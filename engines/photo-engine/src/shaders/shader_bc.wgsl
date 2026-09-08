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
