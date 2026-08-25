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

//! Modèle document (pile de calques) + compositing rapide.
//!
//! Conçu pour l'interaction temps réel, comme les éditeurs pro :
//! - fusion DIRECTE dans un buffer accumulateur (aucune copie intermédiaire)
//! - chemin CPU rayon RGBA8 : la fusion est memory-bound, le round-trip GPU
//!   (conversions f32 + upload + readback bloquant) coûtait plus cher que
//!   le calcul lui-même — le GPU reste réservé aux filtres lourds (blur…)
//! - [`Quality::Preview`] : rendu à échelle réduite pendant les gestes
//!   (drag/sliders), raffiné en pleine résolution au repos (débounced côté app)

use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use rayon::prelude::*;
use std::sync::Arc;

pub const BLEND_MODES: [&str; 6] =
    ["Normal", "Multiply", "Screen", "Overlay", "Darken", "Lighten"];

/// Échelle de l'aperçu interactif (1/4 linéaire = 16× moins de pixels).
pub const PREVIEW_SCALE: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    /// Pleine résolution (au repos, export)
    Full,
    /// Aperçu réduit pendant l'interaction (drag, sliders)
    Preview,
}

#[derive(Clone)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub image: Arc<DynamicImage>,
    /// Texture du calque — l'opacité est appliquée au draw (GPU),
    /// les pixels ne sont JAMAIS régénérés lors des réglages
    pub handle: iced::widget::image::Handle,
    pub thumb: iced::widget::image::Handle,
    pub opacity: f32,
    pub blend_mode: String,
    pub visible: bool,
    pub offset_x: f32,
    pub offset_y: f32,
    /// Rotation en degrés (sens horaire), appliquée au draw autour du centre
    pub rotation: f32,
    /// Échelle uniforme (1.0 = 100 %)
    pub scale: f32,
}

impl Layer {
    pub fn new(id: u64, name: String, image: Arc<DynamicImage>) -> Self {
        let thumb = thumb_handle(&image);
        // Pour les très grandes images, on crée une texture d'aperçu downscalée
        // pour garder la rotation fluide (GPU) — l'original est conservé pour l'export.
        let handle = Self::make_preview_handle(&image);
        Self {
            id,
            name,
            image,
            handle,
            thumb,
            opacity: 100.0,
            blend_mode: "Normal".into(),
            visible: true,
            offset_x: 0.0,
            offset_y: 0.0,
            rotation: 0.0,
            scale: 1.0,
        }
    }

    fn make_preview_handle(image: &DynamicImage) -> iced::widget::image::Handle {
        const MAX_PREVIEW: u32 = 2048;
        let (w, h) = image.dimensions();
        if w.max(h) <= MAX_PREVIEW {
            let rgba = image.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
        } else {
            // Downscale pour l'affichage interactif — garde l'original pour l'export
            let preview = image.resize(
                MAX_PREVIEW,
                MAX_PREVIEW,
                ::image::imageops::FilterType::Triangle,
            );
            let rgba = preview.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
        }
    }

    /// Dimensions du calque
    pub fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    /// Retourne le calque horizontalement/verticalement (destructif).
    pub fn flip(&mut self, horizontal: bool) {
        let flipped = if horizontal {
            self.image.fliph()
        } else {
            self.image.flipv()
        };
        self.apply_edit(flipped);
    }

    /// Remplace le contenu pixels du calque (peinture…) et régénère
    /// texture d'affichage + miniature.
    pub fn apply_edit(&mut self, new_image: ::image::DynamicImage) {
        self.handle = Self::make_preview_handle(&new_image);
        self.thumb = thumb_handle(&new_image);
        self.image = Arc::new(new_image);
    }

    /// Rogne le calque au rect (coordonnées CALQUE, pixels).
    /// Destructif : régénère texture + miniature. Le contenu reste en place
    /// dans le monde (l'offset compense l'origine du crop).
    /// Retourne une erreur descriptive si le rect est invalide.
    pub fn crop(&mut self, x: i32, y: i32, w: u32, h: u32) -> Result<(), String> {
        let (iw, ih) = self.dimensions();
        if w == 0 || h == 0 {
            return Err("rogner : dimensions nulles".into());
        }
        if x < 0 || y < 0 || x + w as i32 > iw as i32 || y + h as i32 > ih as i32 {
            return Err("rogner : la sélection dépasse les bords du calque".into());
        }
        let cropped = self.image.crop_imm(x as u32, y as u32, w, h);
        self.handle = Self::make_preview_handle(&cropped);
        self.thumb = thumb_handle(&cropped);
        self.image = Arc::new(cropped);
        // Compense l'origine : le pixel (x,y) d'origine reste à sa place monde
        self.offset_x += x as f32;
        self.offset_y += y as f32;
        Ok(())
    }
}

/// Miniature 48×32 pour le panneau
pub fn thumb_handle(img: &DynamicImage) -> iced::widget::image::Handle {
    let t = img.resize(48, 32, ::image::imageops::FilterType::Triangle);
    let rgba = t.to_rgba8();
    iced::widget::image::Handle::from_rgba(rgba.width(), rgba.height(), rgba.into_raw())
}

/// Donnée légère envoyée au worker (Arc = partage sans copie).
/// `id` sert de clé de cache pour les aperçus réduits côté worker.
#[derive(Clone)]
pub struct LayerData {
    pub id: u64,
    pub image: Arc<DynamicImage>,
    pub opacity: f32,
    pub blend_mode: String,
    pub offset_x: f32,
    pub offset_y: f32,
    pub rotation: f32,
    pub scale: f32,
    pub visible: bool,
}

impl From<&Layer> for LayerData {
    fn from(l: &Layer) -> Self {
        Self {
            id: l.id,
            image: l.image.clone(),
            opacity: l.opacity,
            blend_mode: l.blend_mode.clone(),
            offset_x: l.offset_x,
            offset_y: l.offset_y,
            rotation: l.rotation,
            scale: l.scale,
            visible: l.visible,
        }
    }
}

/// Identifiant numérique d'un mode de fusion (shader GPU + CPU)
pub fn blend_mode_id(mode: &str) -> u32 {
    match mode {
        "Multiply" => 1,
        "Screen" => 2,
        "Overlay" => 3,
        "Darken" => 4,
        "Lighten" => 5,
        _ => 0, // Normal
    }
}

fn mode_id(mode: &str) -> u32 {
    blend_mode_id(mode)
}

/// Fusion d'un pixel : renvoie la couleur composée (top sur base)
#[inline]
fn blend_pixel(b: [f32; 4], t: [f32; 4], mode: u32) -> [f32; 4] {
    let blended = match mode {
        1 => [b[0] * t[0], b[1] * t[1], b[2] * t[2]],           // Multiply
        2 => [                                                  // Screen
            1.0 - (1.0 - b[0]) * (1.0 - t[0]),
            1.0 - (1.0 - b[1]) * (1.0 - t[1]),
            1.0 - (1.0 - b[2]) * (1.0 - t[2]),
        ],
        3 => [                                                  // Overlay
            if b[0] < 0.5 { 2.0 * b[0] * t[0] } else { 1.0 - 2.0 * (1.0 - b[0]) * (1.0 - t[0]) },
            if b[1] < 0.5 { 2.0 * b[1] * t[1] } else { 1.0 - 2.0 * (1.0 - b[1]) * (1.0 - t[1]) },
            if b[2] < 0.5 { 2.0 * b[2] * t[2] } else { 1.0 - 2.0 * (1.0 - b[2]) * (1.0 - t[2]) },
        ],
        4 => [b[0].min(t[0]), b[1].min(t[1]), b[2].min(t[2])],  // Darken
        5 => [b[0].max(t[0]), b[1].max(t[1]), b[2].max(t[2])],  // Lighten
        _ => [t[0], t[1], t[2]],                                // Normal
    };
    // Alpha compositing : top au-dessus de base, pondéré par l'alpha du top
    let ta = t[3];
    let a = ta + b[3] * (1.0 - ta);
    let mut out = [b[0], b[1], b[2], a];
    if a > 0.0001 {
        for c in 0..3 {
            out[c] = blended[c] * ta + b[c] * b[3] * (1.0 - ta);
            out[c] /= a;
        }
    }
    out
}

/// Fusionne `top` DANS `base` en place — zéro allocation intermédiaire.
pub fn blend_into(
    base: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    top: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    opacity: f32,
    blend_mode: &str,
    offset_x: f32,
    offset_y: f32,
) {
    let op = (opacity / 100.0).clamp(0.0, 1.0);
    if op <= 0.0 {
        return;
    }
    let mode = mode_id(blend_mode);
    let w = base.width();
    let (tw, th) = (top.width() as i32, top.height() as i32);
    let ox = offset_x.round() as i32;
    let oy = offset_y.round() as i32;
    let t_raw = top.as_raw();

    base.as_flat_samples_mut()
        .samples
        .par_chunks_exact_mut(4)
        .enumerate()
        .for_each(|(i, px)| {
            let x = (i as u32 % w) as i32;
            let y = (i as u32 / w) as i32;
            // Échantillonne le dessus à (x - ox, y - oy) — transparent hors bornes
            let tx = x - ox;
            let ty = y - oy;
            let t: [f32; 4] = if tx >= 0 && tx < tw && ty >= 0 && ty < th {
                let o = (ty as u32 * tw as u32 + tx as u32) as usize * 4;
                [
                    t_raw[o] as f32 / 255.0,
                    t_raw[o + 1] as f32 / 255.0,
                    t_raw[o + 2] as f32 / 255.0,
                    t_raw[o + 3] as f32 / 255.0 * op,
                ]
            } else {
                [0.0, 0.0, 0.0, 0.0]
            };
            // Pixel du dessus absent/translucide nul : base inchangée (skip rapide)
            if t[3] <= 0.001 {
                return;
            }
            let b: [f32; 4] = [
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
                px[3] as f32 / 255.0,
            ];
            let o = blend_pixel(b, t, mode);
            px[0] = (o[0].clamp(0.0, 1.0) * 255.0) as u8;
            px[1] = (o[1].clamp(0.0, 1.0) * 255.0) as u8;
            px[2] = (o[2].clamp(0.0, 1.0) * 255.0) as u8;
            px[3] = (o[3].clamp(0.0, 1.0) * 255.0) as u8;
        });
}

fn prepare_top(l: &LayerData) -> (ImageBuffer<Rgba<u8>, Vec<u8>>, f32, f32) {
    // Retourne (buffer transformé, offset_x ajusté, offset_y ajusté)
    // Gère échelle + rotation (autour du centre, comme le canvas) sans rognage.
    let (w0, h0) = l.image.dimensions();
    let scale = l.scale.clamp(0.05, 8.0);
    let mut buf: ImageBuffer<Rgba<u8>, Vec<u8>> = match l.image.as_ref() {
        DynamicImage::ImageRgba8(b) => {
            if (scale - 1.0).abs() > 0.001 {
                let nw = ((w0 as f32 * scale).round() as u32).max(1);
                let nh = ((h0 as f32 * scale).round() as u32).max(1);
                image::imageops::resize(b, nw, nh, image::imageops::FilterType::Triangle)
            } else {
                b.clone()
            }
        }
        other => {
            let rgba = other.to_rgba8();
            if (scale - 1.0).abs() > 0.001 {
                let nw = ((w0 as f32 * scale).round() as u32).max(1);
                let nh = ((h0 as f32 * scale).round() as u32).max(1);
                image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle)
            } else {
                rgba
            }
        }
    };
    let (mut tw, mut th) = (buf.width() as f32, buf.height() as f32);
    let mut ox = l.offset_x;
    let mut oy = l.offset_y;
    let rot = l.rotation.rem_euclid(360.0); // ∈ [0, 360)
    // Multiples de 90° avec epsilon (les rotations libres arrondies
    // silencieusement seraient une erreur visuelle)
    let is_0 = rot < 0.01 || rot > 359.99;
    let is_90 = (rot - 90.0).abs() < 0.01;
    let is_180 = (rot - 180.0).abs() < 0.01;
    let is_270 = (rot - 270.0).abs() < 0.01;
    if is_90 || is_270 {
        // 90° / 270° — swap via imageops (rapide, sans interpolation)
        let rotated = if is_90 {
            image::imageops::rotate90(&buf)
        } else {
            image::imageops::rotate270(&buf)
        };
        let (nw, nh) = (rotated.width() as f32, rotated.height() as f32);
        ox += (tw - nw) / 2.0;
        oy += (th - nh) / 2.0;
        buf = rotated;
        tw = nw;
        th = nh;
    } else if is_180 {
        buf = image::imageops::rotate180(&buf);
    } else if !is_0 {
        // Rotation arbitraire : bounding box + échantillonnage bilinéaire
        let rad = rot.to_radians();
        let cos = rad.cos().abs();
        let sin = rad.sin().abs();
        let bbox_w = (tw * cos + th * sin).ceil().max(1.0) as u32;
        let bbox_h = (tw * sin + th * cos).ceil().max(1.0) as u32;
        let mut out = ImageBuffer::from_pixel(bbox_w, bbox_h, Rgba([0, 0, 0, 0]));
        let cx0 = tw / 2.0;
        let cy0 = th / 2.0;
        let cx1 = bbox_w as f32 / 2.0;
        let cy1 = bbox_h as f32 / 2.0;
        let cos_r = rad.cos();
        let sin_r = rad.sin();
        // Remplissage en parallèle (rayon)
        out.enumerate_pixels_mut()
            .par_bridge()
            .for_each(|(x, y, px)| {
                // Destination -> source (rotation inverse)
                let dx = x as f32 - cx1;
                let dy = y as f32 - cy1;
                let sx = dx * cos_r + dy * sin_r + cx0;
                let sy = -dx * sin_r + dy * cos_r + cy0;
                if sx >= 0.0 && sy >= 0.0 && sx < tw - 1.0 && sy < th - 1.0 {
                    // Bilinéaire
                    let x0 = sx.floor() as u32;
                    let y0 = sy.floor() as u32;
                    let fx = sx - x0 as f32;
                    let fy = sy - y0 as f32;
                    let p00 = buf.get_pixel(x0, y0);
                    let p10 = buf.get_pixel((x0 + 1).min(buf.width() - 1), y0);
                    let p01 = buf.get_pixel(x0, (y0 + 1).min(buf.height() - 1));
                    let p11 = buf.get_pixel(
                        (x0 + 1).min(buf.width() - 1),
                        (y0 + 1).min(buf.height() - 1),
                    );
                    for c in 0..4 {
                        let v = (p00[c] as f32 * (1.0 - fx) * (1.0 - fy)
                            + p10[c] as f32 * fx * (1.0 - fy)
                            + p01[c] as f32 * (1.0 - fx) * fy
                            + p11[c] as f32 * fx * fy)
                            .round() as u8;
                        px[c] = v;
                    }
                }
            });
        ox += (tw - bbox_w as f32) / 2.0;
        oy += (th - bbox_h as f32) / 2.0;
        buf = out;
    }
    let _ = (tw, th);
    (buf, ox, oy)
}

/// Composite la pile (index 0 = bas) sur un document `doc_w × doc_h`.
/// CROP au document — utilisé pour l'export. Voir `composite_preview` pour
/// le plan de travail infini (sans crop).
pub fn composite(layers: &[LayerData], doc_w: u32, doc_h: u32) -> Option<DynamicImage> {
    let mut acc: Option<ImageBuffer<Rgba<u8>, Vec<u8>>> = None;

    for l in layers.iter().filter(|l| l.visible && l.opacity > 0.01) {
        let acc_buf = acc.get_or_insert_with(|| {
            ImageBuffer::from_pixel(doc_w.max(1), doc_h.max(1), Rgba([0, 0, 0, 0]))
        });
        let (top, ox, oy) = prepare_top(l);
        blend_into(acc_buf, &top, l.opacity, &l.blend_mode, ox, oy);
    }

    acc.map(DynamicImage::ImageRgba8)
}

/// Composite pour le plan de travail infini : aucun crop au document.
/// Le document reste centré (comme Affinity/Photoshop) et les calques
/// hors document restent visibles. Le masquage sera par calque.
pub fn composite_preview(layers: &[LayerData], doc_w: u32, doc_h: u32) -> Option<DynamicImage> {
    let visible: Vec<&LayerData> = layers
        .iter()
        .filter(|l| l.visible && l.opacity > 0.01)
        .collect();
    if visible.is_empty() {
        return None;
    }

    // Plan infini centré sur le document : on agrandit symétriquement autour
    // du centre document pour que l'overlay reste fixe quand on déplace un calque.
    let doc_cx = doc_w as f32 / 2.0;
    let doc_cy = doc_h as f32 / 2.0;
    let mut half_w = doc_w as f32 / 2.0;
    let mut half_h = doc_h as f32 / 2.0;
    for l in &visible {
        let (w0, h0) = l.image.dimensions();
        let scale = l.scale.clamp(0.05, 8.0);
        let mut tw = w0 as f32 * scale;
        let mut th = h0 as f32 * scale;
        let rot = l.rotation.rem_euclid(360.0); // ∈ [0, 360)
        let is_0 = rot < 0.01 || rot > 359.99;
        let is_90 = (rot - 90.0).abs() < 0.01;
        let is_180 = (rot - 180.0).abs() < 0.01;
        let is_270 = (rot - 270.0).abs() < 0.01;
        if is_90 || is_270 {
            std::mem::swap(&mut tw, &mut th);
        } else if !is_0 && !is_180 {
            let rad = rot.to_radians();
            let cos = rad.cos().abs();
            let sin = rad.sin().abs();
            let bbox_w = tw * cos + th * sin;
            let bbox_h = tw * sin + th * cos;
            tw = bbox_w;
            th = bbox_h;
        }
        // Centre du calque après rotation autour de son centre
        let mut cx = l.offset_x + w0 as f32 * scale / 2.0;
        let mut cy = l.offset_y + h0 as f32 * scale / 2.0;
        if !is_0 {
            let adj_x = (w0 as f32 * scale - tw) / 2.0;
            let adj_y = (h0 as f32 * scale - th) / 2.0;
            cx = l.offset_x + adj_x + tw / 2.0;
            cy = l.offset_y + adj_y + th / 2.0;
        }
        half_w = half_w.max((cx - doc_cx).abs() + tw / 2.0);
        half_h = half_h.max((cy - doc_cy).abs() + th / 2.0);
    }
    // Clamp pour éviter OOM (16384 ≈ 1GB RGBA)
    let w = (half_w * 2.0).clamp(1.0, 16384.0) as u32;
    let h = (half_h * 2.0).clamp(1.0, 16384.0) as u32;

    let mut acc = ImageBuffer::from_pixel(w, h, Rgba([0, 0, 0, 0]));
    // Origine monde (0,0) = centre du buffer - doc_center
    let origin_x = half_w - doc_cx;
    let origin_y = half_h - doc_cy;
    for l in visible {
        let (top, ox0, oy0) = prepare_top(l);
        let ox = ox0 + origin_x;
        let oy = oy0 + origin_y;
        blend_into(&mut acc, &top, l.opacity, &l.blend_mode, ox, oy);
    }

    Some(DynamicImage::ImageRgba8(acc))
}
