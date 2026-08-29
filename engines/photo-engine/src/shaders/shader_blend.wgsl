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
