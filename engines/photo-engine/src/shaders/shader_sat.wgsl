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
