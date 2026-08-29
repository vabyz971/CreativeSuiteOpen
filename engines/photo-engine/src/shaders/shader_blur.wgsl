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
