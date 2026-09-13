struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct Params {
    screen_doc: vec4<f32>,   // xy = viewport widget px, zw = document px
    pan_zoom: vec4<f32>,     // xy = pan, z = zoom, w = layer opacity
    mode_sizes: vec4<u32>,   // x = blend mode, y/z = top texture dims, w = image flag
    off_sel: vec4<f32>,      // xy = réservé (placement = xform), zw = position selection
    sel_size: vec4<f32>,     // xy = size selection (x > 0 = active)
    mask_info: vec4<u32>,    // x = 1 si masque, y/z = dims texture masque, w = 1 si masque en espace document
    xform: vec4<f32>,        // matrice inverse 2x2 (n00, n01, n10, n11) : (doc - origine) -> local recentré
    xform_off: vec4<f32>,    // xy = origine doc, zw = centre local (cx, cy)
    adjust: vec4<f32>,       // x = luminosité normalisée, y = facteur contraste, z = saturation, w = réservé
    blur_px: vec4<f32>,      // x = rayon px doc, y = axe (0 = H, 1 = V), zw réservés
    cursor: vec4<f32>,       // xy = curseur pinceau (doc), z = rayon px doc, w : 0 inactif, 1 pinceau, 2 gomme
    loupe: vec4<f32>,        // xy = centre doc du patch, z = côté px doc, w = 1 si active
    cadre_a: vec4<f32>,      // coins 0-1 (doc) du calque sélectionné
    cadre_b: vec4<f32>,      // coins 2-3 (doc) du calque sélectionné
    cadre_info: vec4<u32>,   // x = contour actif, y = poignée active (0 aucune)
    poig_a: vec4<f32>,       // poignées rotation/échelle (doc)
    poig_b: vec4<f32>,       // poignées inclinaisons X/Y (doc)
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
@group(0) @binding(5) var mask_tex: texture_2d<f32>;
@group(0) @binding(6) var mask_samp: sampler;

// Coordonnées source du calque pour un point document : la matrice inverse
// (uniformes xform / xform_off, précalculée côté CPU depuis Transform2D)
// applique offset + échelle + rotation + skew en une fois.
fn layer_px(doc_px: vec2<f32>) -> vec2<f32> {
    let dl = doc_px - bp.xform_off.xy;
    let lx = bp.xform.x * dl.x + bp.xform.y * dl.y + bp.xform_off.z;
    let ly = bp.xform.z * dl.x + bp.xform.w * dl.y + bp.xform_off.w;
    return vec2<f32>(lx, ly);
}

@fragment
fn fs_blend(in: VOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let b = textureSampleLevel(base_tex, base_samp, uv, 0.0);
    // Coordonnees pixel document de ce fragment
    let doc_px = uv * bp.screen_doc.zw;
    let t_px = layer_px(doc_px);
    let t_uv = t_px / vec2<f32>(f32(bp.mode_sizes.y), f32(bp.mode_sizes.z));
    var t = vec4<f32>(0.0);
    if (t_uv.x >= 0.0 && t_uv.x <= 1.0 && t_uv.y >= 0.0 && t_uv.y <= 1.0) {
        t = textureSampleLevel(top_tex, top_samp, t_uv, 0.0);
    }
    // Masque échantillonné AU DRAW : peindre un masque ne régénère jamais
    // la texture du calque. Espace source du calque par défaut ; espace
    // document pour les masques de groupe (mask_info.w = 1).
    // Hors bornes, l'échantillonneur clamp (comme le rééchantillonnage CPU).
    var cov = 1.0;
    if (bp.mask_info.x == 1u) {
        var m_px = t_px;
        if (bp.mask_info.w == 1u) { m_px = doc_px; }
        let m_uv = m_px / vec2<f32>(f32(bp.mask_info.y), f32(bp.mask_info.z));
        cov = textureSampleLevel(mask_tex, mask_samp, m_uv, 0.0).r;
    }
    let ta = t.a * cov * bp.pan_zoom.w;
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

// Passe d'ajustement : UNE opération couleur (luminosité/contraste OU
// saturation, les autres au neutre) appliquée à l'accumulateur. Mêmes maths
// que le chemin CPU (contraste autour de 0.5 puis luminosité ; gris
// Rec.601 pour la saturation).
@fragment
fn fs_adjust(in: VOut) -> @location(0) vec4<f32> {
    var c = textureSampleLevel(base_tex, base_samp, in.uv, 0.0);
    c = vec4<f32>((c.rgb - vec3<f32>(0.5)) * bp.adjust.y + vec3<f32>(0.5) + vec3<f32>(bp.adjust.x), c.a);
    let gray = dot(c.rgb, vec3<f32>(0.299, 0.587, 0.114));
    c = vec4<f32>(mix(vec3<f32>(gray), c.rgb, bp.adjust.z), c.a);
    return clamp(c, vec4<f32>(0.0), vec4<f32>(1.0));
}

// Flou gaussien séparable (sigma = rayon) : une passe H (blur_px.y = 0)
// puis une passe V (= 1). Noyau discrétisé sur 33 prises max ; au-delà de
// 16 px de chaque côté on sous-échantillonne (approximation documentée,
// l'export exact reste CPU).
@fragment
fn fs_blur(in: VOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / bp.screen_doc.zw;
    let radius = max(bp.blur_px.x, 0.5);
    var dir = vec2<f32>(texel.x, 0.0);
    if (bp.blur_px.y > 0.5) { dir = vec2<f32>(0.0, texel.y); }
    let taps_f = min(ceil(radius), 16.0);
    let taps = i32(taps_f);
    let step_len = max(radius / max(taps_f, 1.0), 1.0);
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    for (var i: i32 = -16; i <= 16; i++) {
        if (i > taps || i < -taps) { continue; }
        let d = f32(i) * step_len;
        let w = exp(-0.5 * (d / radius) * (d / radius));
        acc += textureSampleLevel(base_tex, base_samp, in.uv + dir * d, 0.0) * w;
        wsum += w;
    }
    return acc / max(wsum, 0.0001);
}

// Distance point-segment (espace document) pour les contours.
fn dist_seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 0.0001), 0.0, 1.0);
    return distance(p, a + ab * t);
}

// Présentation écran : `base_tex` reçoit la texture accumulée finale,
// `top_tex` le patch loupe éventuel (ou une texture neutre si inactive).
// Un seul uniform et un seul layout pour toutes les passes.
const GRID_BG = vec3<f32>(0.0549, 0.0549, 0.0549);   // theme::SURFACE_CONTAINER_LOWEST #0E0E0E
const GRID_DOT = vec3<f32>(0.2078, 0.2078, 0.2039);  // theme::SURFACE_CONTAINER_HIGHEST #353534
const ACCENT = vec3<f32>(0.0, 0.4784, 1.0);          // theme::ACCENT #007AFF
const SUR_ACCENT = vec3<f32>(1.0, 1.0, 1.0);         // theme::TEXT_ON_ACCENT

@fragment
fn fs_present(in: VOut) -> @location(0) vec4<f32> {
    let screen_px = in.uv * bp.screen_doc.xy;
    let pan = bp.pan_zoom.xy;
    let zoom = bp.pan_zoom.z;

    // Solid background — checker removed for performance and resize stability
    var col = GRID_BG;

    // Coordonnées document du fragment (centre doc = milieu du widget + pan)
    let doc_px = (screen_px - bp.screen_doc.xy / 2.0 - pan) / zoom + bp.screen_doc.zw / 2.0;

    // Image composite (espace document), si document present
    if (bp.mode_sizes.w == 1u) {
        if (doc_px.x >= 0.0 && doc_px.y >= 0.0 && doc_px.x <= bp.screen_doc.z && doc_px.y <= bp.screen_doc.w) {
            let auv = doc_px / bp.screen_doc.zw;
            let img = textureSampleLevel(base_tex, base_samp, auv, 0.0);
            col = mix(col, img.rgb, img.a);
        }
    }

    // Anneau curseur pinceau / gomme (rayon en px document, tracé net).
    if (bp.cursor.w > 0.5) {
        let d = distance(doc_px, bp.cursor.xy);
        let wpx = 1.5 / max(zoom, 0.001);
        if (abs(d - bp.cursor.z) < wpx) {
            col = mix(col, vec3<f32>(1.0), 0.9);
        }
        // Point central (pinceau seul, pas la gomme)
        if (bp.cursor.w < 1.5 && d < wpx) {
            col = vec3<f32>(1.0);
        }
    }

    // Loupe pipette : patch grossi x8 ancré en haut à droite du curseur
    // (pixels document bien visibles) + réticule central de visée.
    if (bp.loupe.w > 0.5) {
        let side_doc = max(bp.loupe.z, 1.0);
        let size_scr = side_doc * zoom * 8.0;
        let cur_scr = (bp.loupe.xy - bp.screen_doc.zw * 0.5) * zoom + pan + bp.screen_doc.xy * 0.5;
        let origin = cur_scr + vec2<f32>(16.0, -size_scr - 16.0);
        if (all(screen_px >= origin) && all(screen_px <= origin + vec2<f32>(size_scr))) {
            let p_uv = (screen_px - origin) / size_scr;
            var loupe_col = textureSampleLevel(top_tex, top_samp, p_uv, 0.0).rgb;
            let centre = origin + vec2<f32>(size_scr * 0.5);
            if (abs(screen_px.x - centre.x) < 1.0 || abs(screen_px.y - centre.y) < 1.0) {
                loupe_col = mix(loupe_col, vec3<f32>(1.0) - loupe_col, 0.8);
            }
            col = loupe_col;
        }
    }

    // Bordure du document (dimension visible du plan de travail).
    if (bp.mode_sizes.w == 1u) {
        let ex = min(doc_px.x, bp.screen_doc.z - doc_px.x);
        let ey = min(doc_px.y, bp.screen_doc.w - doc_px.y);
        if (doc_px.x >= 0.0 && doc_px.y >= 0.0 && doc_px.x <= bp.screen_doc.z && doc_px.y <= bp.screen_doc.w
            && min(ex, ey) * zoom < 1.5) {
            col = mix(col, GRID_DOT, 0.9);
        }
    }

    // Contour du calque sélectionné + poignées (coins : disques ;
    // inclinaisons : losanges ; échelle : carré ; rotation : disque + tige).
    // Poignée active : remplissage accent, sinon blanc ; anneau accent.
    if (bp.cadre_info.x == 1u) {
        let q0 = bp.cadre_a.xy;
        let q1 = bp.cadre_a.zw;
        let q2 = bp.cadre_b.xy;
        let q3 = bp.cadre_b.zw;
        var d = dist_seg(doc_px, q0, q1);
        d = min(d, dist_seg(doc_px, q1, q2));
        d = min(d, dist_seg(doc_px, q2, q3));
        d = min(d, dist_seg(doc_px, q3, q0));
        if (d * zoom < 1.5) {
            col = mix(col, ACCENT, 0.9);
        }
        let actif = bp.cadre_info.y;
        // Disques des 4 coins.
        var dd = distance(doc_px, q0);
        dd = min(dd, distance(doc_px, q1));
        dd = min(dd, distance(doc_px, q2));
        dd = min(dd, distance(doc_px, q3));
        if (dd * zoom < 5.0) {
            var rempl = SUR_ACCENT;
            if (actif == 1u) { rempl = ACCENT; }
            col = mix(col, rempl, 0.95);
        }
        if (abs(dd * zoom - 5.0) < 1.0) {
            col = mix(col, ACCENT, 0.9);
        }
        // Tige de rotation + disque.
        let haut = (q0 + q1) * 0.5;
        let rot = bp.poig_a.xy;
        if (dist_seg(doc_px, haut, rot) * zoom < 1.0) {
            col = mix(col, ACCENT, 0.9);
        }
        if (distance(doc_px, rot) * zoom < 5.0) {
            var rempl_r = SUR_ACCENT;
            if (actif == 2u) { rempl_r = ACCENT; }
            col = mix(col, rempl_r, 0.95);
        }
        if (abs(distance(doc_px, rot) * zoom - 5.0) < 1.0) {
            col = mix(col, ACCENT, 0.9);
        }
        // Losanges d'inclinaison.
        let skx = bp.poig_b.xy;
        let sky = bp.poig_b.zw;
        let los_x = abs(doc_px.x - skx.x) + abs(doc_px.y - skx.y);
        let los_y = abs(doc_px.x - sky.x) + abs(doc_px.y - sky.y);
        if (los_x * zoom < 5.0) {
            var rempl_x = SUR_ACCENT;
            if (actif == 3u) { rempl_x = ACCENT; }
            col = mix(col, rempl_x, 0.95);
        }
        if (los_y * zoom < 5.0) {
            var rempl_y = SUR_ACCENT;
            if (actif == 4u) { rempl_y = ACCENT; }
            col = mix(col, rempl_y, 0.95);
        }
        // Carré d'échelle.
        let ech = bp.poig_a.zw;
        let carre = max(abs(doc_px.x - ech.x), abs(doc_px.y - ech.y));
        if (carre * zoom < 7.0) {
            var rempl_e = SUR_ACCENT;
            if (actif == 5u) { rempl_e = ACCENT; }
            col = mix(col, rempl_e, 0.95);
        }
        if (abs(carre * zoom - 7.0) < 1.0) {
            col = mix(col, ACCENT, 0.9);
        }
    }

    // Rectangle de selection (coords ecran)
    if (bp.sel_size.x > 0.0) {
        let smin = min(bp.off_sel.zw, bp.off_sel.zw + bp.sel_size.xy);
        let smax = max(bp.off_sel.zw, bp.off_sel.zw + bp.sel_size.xy);
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
