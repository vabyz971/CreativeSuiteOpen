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

//! Export d'image vers les formats d'échange (PNG / JPEG).
//!
//! - PNG : transparence préservée (plan de travail infini inclus).
//! - JPEG : ne gère pas l'alpha → aplatement automatique sur fond BLANC
//!   (comportement des éditeurs pro), qualité réglable.

use std::path::Path;

use image::{DynamicImage, ImageBuffer, Rgba};

/// Qualité JPEG par défaut (échelle libturbo-jpeg 1..=100).
pub const DEFAULT_JPEG_QUALITY: u8 = 90;

/// Format d'export demandé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    /// Transparence préservée
    Png,
    /// Alpha aplati sur fond blanc ; `quality` dans 1..=100
    Jpeg { quality: u8 },
}

impl ExportFormat {
    /// Déduit le format de l'extension du chemin (`.png`, `.jpg`, `.jpeg`),
    /// insensible à la casse. Toute autre extension → PNG (valeur sûre :
    /// sans perte, transparence conservée).
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        match ext.as_deref() {
            Some("jpg") | Some("jpeg") => Self::Jpeg {
                quality: DEFAULT_JPEG_QUALITY,
            },
            _ => Self::Png,
        }
    }
}

/// Le JPEG ignore l'alpha : on composite l'image sur un fond opaque BLANC
/// (et non un simple décapage du canal alpha qui rendrait les zones
/// transparentes noires ou arbitraires).
#[must_use]
pub fn flatten_on_white(img: &DynamicImage) -> DynamicImage {
    let rgba = img.to_rgba8();
    if rgba.pixels().all(|p| p.0[3] == 255) {
        return DynamicImage::ImageRgba8(rgba); // déjà opaque : zéro copie utile
    }
    let (w, h) = rgba.dimensions();
    let mut canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(w.max(1), h.max(1), Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut canvas, &rgba, 0, 0);
    DynamicImage::ImageRgba8(canvas)
}

/// Écrit l'image sur disque au format demandé.
///
/// # Errors
/// Erreur d'encodage ou d'écriture disque — message descriptif en français.
pub fn export_image(img: &DynamicImage, path: &Path, format: ExportFormat) -> Result<(), String> {
    let mut file =
        std::fs::File::create(path).map_err(|e| format!("Création de {}: {e}", path.display()))?;
    match format {
        ExportFormat::Png => img
            .write_to(&mut file, image::ImageFormat::Png)
            .map_err(|e| format!("Encodage PNG : {e}")),
        ExportFormat::Jpeg { quality } => {
            let flattened = flatten_on_white(img);
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut file,
                quality.clamp(1, 100),
            );
            flattened
                .write_with_encoder(encoder)
                .map_err(|e| format!("Encodage JPEG : {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_transparent() -> DynamicImage {
        // Moitié gauche rouge opaque, moitié droite TRANSPARENTE
        let mut b = ImageBuffer::from_pixel(4, 2, Rgba([0, 0, 0, 0]));
        for y in 0..2 {
            for x in 0..2 {
                b.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        DynamicImage::ImageRgba8(b)
    }

    fn temp_path(tag: &str, ext: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "cso-export-{tag}-{}.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("horloge")
                .as_nanos(),
            ext
        ))
    }

    #[test]
    fn format_deduit_de_lextension_insensible_casse() {
        assert_eq!(
            ExportFormat::from_path(Path::new("a.png")),
            ExportFormat::Png
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("a.PNG")),
            ExportFormat::Png
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("b.JpG")),
            ExportFormat::Jpeg {
                quality: DEFAULT_JPEG_QUALITY
            }
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("c.jpeg")),
            ExportFormat::Jpeg {
                quality: DEFAULT_JPEG_QUALITY
            }
        );
        // Extension inconnue : PNG par défaut (sûr, sans perte)
        assert_eq!(
            ExportFormat::from_path(Path::new("d.webp")),
            ExportFormat::Png
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("sans-ext")),
            ExportFormat::Png
        );
    }

    #[test]
    fn png_conserve_la_transparence() {
        let img = sample_transparent();
        let path = temp_path("png", "png");
        export_image(&img, &path, ExportFormat::Png).expect("export png");

        let reloaded = image::open(&path).expect("relecture");
        std::fs::remove_file(&path).ok();
        let rgba = reloaded.to_rgba8();
        let p = rgba.get_pixel(3, 0);
        assert_eq!(p[3], 0, "alpha conservé en PNG");
    }

    #[test]
    fn jpeg_aplatit_sur_blanc_et_perd_l_alpha() {
        let img = sample_transparent();
        let path = temp_path("jpg", "jpg");
        export_image(&img, &path, ExportFormat::Jpeg { quality: 95 }).expect("export jpg");

        let reloaded = image::open(&path).expect("relecture");
        std::fs::remove_file(&path).ok();

        // Zone transparente → blanc pur (tolérance compression)
        let rgb = reloaded.to_rgb8();
        let bg = rgb.get_pixel(3, 0);
        assert!(
            i16::from(bg[0]) >= 252 && i16::from(bg[1]) >= 252 && i16::from(bg[2]) >= 252,
            "fond attendu blanc, obtenu {bg:?}"
        );
        // Zone opaque rouge → reste dominée par le rouge
        let fg = rgb.get_pixel(0, 0);
        assert!(
            fg[0] > 200 && fg[1] < 80 && fg[2] < 80,
            "rouge attendu, obtenu {fg:?}"
        );
    }

    #[test]
    fn image_deja_opaque_n_est_pas_recopiee_par_flatten() {
        let opaque =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 2, Rgba([10, 20, 30, 255])));
        // Même contenu pixel : le fast-path évite l'overlay inutile
        let out = flatten_on_white(&opaque);
        assert_eq!(*out.to_rgba8(), *opaque.to_rgba8());
    }
}
