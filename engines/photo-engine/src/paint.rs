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

//! Peinture destructive sur calque : rastérisation d'un trait de pinceau
//! ou de gomme.
//!
//! Le trait est tamponné dans un masque de couverture (évite le
//! assombrissement aux recouvrements), puis composé sur la base :
//! - [`StrokeMode::Paint`] : source-over avec opacité uniforme — comme un
//!   vrai coup de pinceau
//! - [`StrokeMode::Erase`] : destination-out — réduit l'alpha des pixels
//!   visés sans toucher à leur couleur

/// Ce que fait le trait sur les pixels du calque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeMode {
    /// Peint la couleur du pinceau (source-over)
    Paint,
    /// Efface : réduit l'alpha proportionnellement à l'opacité du trait
    Erase,
}

/// Réglages d'un outil à trait (pinceau ou gomme).
#[derive(Clone, Copy, Debug)]
pub struct BrushParams {
    /// Rayon en pixels CALQUE
    pub radius: f32,
    /// Couleur RGB (ignorée en mode Erase)
    pub color: [u8; 3],
    /// Opacité globale du trait [0..1] (les recouvrements internes ne
    /// s'accumulent PAS — un seul composite à la fin)
    pub opacity: f32,
    pub mode: StrokeMode,
}

/// Rastérise un trait dans un tampon RGBA8 (w×h, espace CALQUE en pixels).
///
/// * `points` : polyligne en coordonnées calque (centre du pixel (0,0) = 0.0)
pub fn paint_stroke_rgba(rgba: &mut [u8], w: u32, h: u32, points: &[(f32, f32)], b: &BrushParams) {
    if points.is_empty() || w == 0 || h == 0 || b.radius <= 0.0 || b.opacity <= 0.0 {
        return;
    }
    let opacity = b.opacity.clamp(0.0, 1.0);
    let radius = b.radius.max(0.5);

    // --- Bounding box du trait (limité au calque) ---
    let pad = radius.ceil() + 1.0;
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for &(x, y) in points {
        min_x = min_x.min(x - pad);
        min_y = min_y.min(y - pad);
        max_x = max_x.max(x + pad);
        max_y = max_y.max(y + pad);
    }
    let bx0 = min_x.floor().max(0.0) as u32;
    let by0 = min_y.floor().max(0.0) as u32;
    let bx1 = (max_x.ceil() as u32).min(w);
    let by1 = (max_y.ceil() as u32).min(h);
    if bx0 >= bx1 || by0 >= by1 {
        return;
    }
    let bw = (bx1 - bx0) as usize;
    let bh = (by1 - by0) as usize;

    // --- Masque de couverture 0/255 ---
    let mut mask = vec![0u8; bw * bh];
    let stamp = |mask: &mut [u8], cx: f32, cy: f32| {
        let r2 = radius * radius;
        let x0 = (cx - radius).floor().max(bx0 as f32) as i64;
        let x1 = (cx + radius).ceil().min(bx1 as f32) as i64;
        let y0 = (cy - radius).floor().max(by0 as f32) as i64;
        let y1 = (cy + radius).ceil().min(by1 as f32) as i64;
        for py in y0..y1 {
            for px in x0..x1 {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                if dx * dx + dy * dy <= r2 {
                    let mi = ((py - by0 as i64) as usize) * bw + ((px - bx0 as i64) as usize);
                    mask[mi] = 255;
                }
            }
        }
    };

    // Tampons espacés le long des segments (pas ~ rayon/3 → trait continu)
    let step = (radius / 3.0).max(0.5);
    let mut prev = points[0];
    stamp(&mut mask, prev.0, prev.1);
    for &p in &points[1..] {
        let dx = p.0 - prev.0;
        let dy = p.1 - prev.1;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < f32::EPSILON {
            continue;
        }
        let n = (dist / step).ceil() as usize;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            stamp(&mut mask, prev.0 + dx * t, prev.1 + dy * t);
        }
        prev = p;
    }

    // --- Composite selon le mode ---
    let a_paint = opacity;
    let (cr, cg, cb) = (b.color[0] as f32, b.color[1] as f32, b.color[2] as f32);
    for my in 0..bh as u32 {
        for mx in 0..bw as u32 {
            let cov = mask[my as usize * bw + mx as usize] as f32 / 255.0;
            if cov <= 0.0 {
                continue;
            }
            let a = a_paint * cov;
            let x = bx0 + mx;
            let y = by0 + my;
            let idx = ((y as usize * w as usize) + x as usize) * 4;
            let sa = rgba[idx + 3] as f32 / 255.0;
            match b.mode {
                StrokeMode::Paint => {
                    let sr = rgba[idx] as f32;
                    let sg = rgba[idx + 1] as f32;
                    let sb = rgba[idx + 2] as f32;
                    // source-over : out = src*a + dst*(1-a)
                    let out_a = a + sa * (1.0 - a);
                    if out_a <= 0.0 {
                        continue;
                    }
                    rgba[idx] = ((cr * a + sr * sa * (1.0 - a)) / out_a)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    rgba[idx + 1] = ((cg * a + sg * sa * (1.0 - a)) / out_a)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    rgba[idx + 2] = ((cb * a + sb * sa * (1.0 - a)) / out_a)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    rgba[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
                }
                StrokeMode::Erase => {
                    // destination-out : l'alpha est réduit, la couleur reste
                    // valide (pixels droits — RGB inchangé quand alpha baisse).
                    let out_a = sa * (1.0 - a);
                    rgba[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

/// Résultat d'un commit de trait : buffers prêts pour la couche UI
/// (aucun calcul lourd restant côté interface).
#[derive(Clone)]
pub struct StrokeCommit {
    /// Pixels calque complets (w×h RGBA8)
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Aperçu interactif (≤2048 px) — buffer pur, conversion UI côté app
    pub preview: crate::document::RgbaBuf,
    /// Miniature 48×32 — buffer pur
    pub thumb: crate::document::RgbaBuf,
}

impl std::fmt::Debug for StrokeCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Pixels omis volontairement (buffers volumineux)
        f.debug_struct("StrokeCommit")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

/// Travail LOURD d'un coup de pinceau/gomme — à exécuter HORS thread UI
/// (`Task::perform`) : copie du buffer, rastérisation, aperçu, miniature.
///
/// * `base`      : image source du calque (Arc partagé, non modifiée)
/// * `pts_doc`   : polyligne en coordonnées DOCUMENT
/// * offset/scale/rotation_deg : transform courant du calque (doc → calque)
pub fn commit_stroke(
    base: &image::DynamicImage,
    pts_doc: &[(f32, f32)],
    offset: (f32, f32),
    scale: f32,
    rotation_deg: f32,
    brush: &BrushParams,
) -> StrokeCommit {
    use ::image::GenericImageView;
    let (lw, lh) = base.dimensions();

    // Espace DOCUMENT → espace CALQUE (offset + échelle + rotation inversées
    // autour du centre du calque) — identique au transform de draw.
    let theta = -rotation_deg.to_radians();
    let (cos, sin) = (theta.cos(), theta.sin());
    let cx = offset.0 + lw as f32 * scale / 2.0;
    let cy = offset.1 + lh as f32 * scale / 2.0;
    let pts: Vec<(f32, f32)> = pts_doc
        .iter()
        .map(|&(dx, dy)| {
            let (rx, ry) = (
                (dx - cx) * cos - (dy - cy) * sin,
                (dx - cx) * sin + (dy - cy) * cos,
            );
            (rx / scale + lw as f32 / 2.0, ry / scale + lh as f32 / 2.0)
        })
        .collect();

    let mut rgba = base.to_rgba8().into_raw();
    paint_stroke_rgba(&mut rgba, lw, lh, &pts, brush);
    let painted = ::image::DynamicImage::ImageRgba8(
        ::image::RgbaImage::from_raw(lw, lh, rgba.clone()).expect("taille inchangée"),
    );

    StrokeCommit {
        width: lw,
        height: lh,
        rgba,
        preview: crate::document::preview_buf(&painted),
        thumb: crate::document::thumb_buf(&painted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_opaque_sur_fond_transparent() {
        let w = 16u32;
        let h = 16u32;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        paint_stroke_rgba(
            &mut rgba,
            w,
            h,
            &[(4.0, 8.0), (12.0, 8.0)],
            &BrushParams {
                radius: 2.0,
                color: [255, 0, 0],
                opacity: 1.0,
                mode: StrokeMode::Paint,
            },
        );
        // Centre du trait : rouge opaque
        let idx = ((8 * w + 8) * 4) as usize;
        assert_eq!(rgba[idx], 255);
        assert_eq!(rgba[idx + 3], 255);
        // Coin hors trait : transparent
        assert_eq!(rgba[3], 0);
    }

    #[test]
    fn opacite_50_sur_fond_blanc() {
        let w = 8u32;
        let h = 8u32;
        let mut rgba = vec![255u8; (w * h * 4) as usize];
        paint_stroke_rgba(
            &mut rgba,
            w,
            h,
            &[(4.0, 4.0)],
            &BrushParams {
                radius: 2.0,
                color: [0, 0, 0],
                opacity: 0.5,
                mode: StrokeMode::Paint,
            },
        );
        let idx = ((4 * w + 4) * 4) as usize;
        assert_eq!(rgba[idx], 128); // mélange 50/50
        assert_eq!(rgba[idx + 3], 255); // fond opaque préservé
    }

    #[test]
    fn gomme_opaque_efface_le_centre_preserve_les_bords() {
        let w = 16u32;
        let h = 16u32;
        let mut rgba = vec![255u8; (w * h * 4) as usize]; // blanc opaque
        paint_stroke_rgba(
            &mut rgba,
            w,
            h,
            &[(8.0, 8.0)],
            &BrushParams {
                radius: 3.0,
                color: [0, 0, 0],
                opacity: 1.0,
                mode: StrokeMode::Erase,
            },
        );
        let centre = ((8 * w + 8) * 4) as usize;
        assert_eq!(rgba[centre + 3], 0); // alpha effacé
        assert_eq!(rgba[centre], 255); // RGB inchangé (droits)
        let coin = 0usize; // pixel document (0, 0), hors trait
        assert_eq!(rgba[coin + 3], 255); // hors trait : intact
    }

    #[test]
    fn gomme_50_reduit_alpha_de_moitie() {
        let w = 8u32;
        let h = 8u32;
        let mut rgba = vec![200u8; (w * h * 4) as usize];
        paint_stroke_rgba(
            &mut rgba,
            w,
            h,
            &[(4.0, 4.0)],
            &BrushParams {
                radius: 2.0,
                color: [0, 0, 0],
                opacity: 0.5,
                mode: StrokeMode::Erase,
            },
        );
        let idx = ((4 * w + 4) * 4) as usize;
        assert!((rgba[idx + 3] as i16 - 100).abs() <= 1); // 200 → ≈100
        assert_eq!(rgba[idx], 200); // couleur préservée
    }

    #[test]
    fn gomme_sur_pixel_deja_transparent_sans_effet_bord() {
        let w = 8u32;
        let h = 8u32;
        let mut rgba = vec![0u8; (w * h * 4) as usize]; // tout transparent
        paint_stroke_rgba(
            &mut rgba,
            w,
            h,
            &[(4.0, 4.0)],
            &BrushParams {
                radius: 2.0,
                color: [9, 9, 9],
                opacity: 1.0,
                mode: StrokeMode::Erase,
            },
        );
        // Rien ne peut devenir opaque ni coloré par une gomme
        assert!(rgba.iter().all(|&v| v == 0));
    }
}
