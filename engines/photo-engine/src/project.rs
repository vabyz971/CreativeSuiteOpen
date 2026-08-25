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

//! Format projet natif `.csophoto` — indépendant de l'UI, réutilisable
//! par les autres apps de la suite (compositing vidéo de calques…).
//!
//! Conteneur JSON versionné : métadonnées du document + un calque par
//! entrée, chaque image encodée PNG puis base64 (lisibilité, diff partiel,
//! zéro dépendance d'archive). Le décodage régénère les buffers d'aperçu.

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use base64::Engine as _;
use image::DynamicImage;
use serde::{Deserialize, Serialize};

use crate::document::{BLEND_MODES, Layer};

/// Version du format — incrémenter à toute évolution incompatible.
const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    version: u32,
    width: u32,
    height: u32,
    layers: Vec<LayerDto>,
}

#[derive(Serialize, Deserialize)]
struct LayerDto {
    id: u64,
    name: String,
    opacity: f32,
    blend_mode: String,
    visible: bool,
    offset_x: f32,
    offset_y: f32,
    rotation: f32,
    scale: f32,
    /// Pixels du calque encodés PNG puis base64
    png_base64: String,
}

/// Document rechargé depuis un `.csophoto`.
pub struct LoadedProject {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    /// Chemin du fichier chargé (devient le chemin d'enregistrement courant)
    pub path: Option<std::path::PathBuf>,
    /// Nom de fichier sans extension — alimente le titre du canvas
    pub source_name: Option<String>,
}

impl Clone for LoadedProject {
    fn clone(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            layers: self.layers.clone(),
            path: self.path.clone(),
            source_name: self.source_name.clone(),
        }
    }
}

impl std::fmt::Debug for LoadedProject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedProject")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("layers", &self.layers.len())
            .field("path", &self.path)
            .field("source_name", &self.source_name)
            .finish()
    }
}

/// Enregistre la pile de calques dans un fichier `.csophoto`.
pub fn save(path: &Path, layers: &[Layer], doc_w: u32, doc_h: u32) -> Result<(), String> {
    let mut dtos = Vec::with_capacity(layers.len());
    for l in layers {
        let mut png = Vec::new();
        l.image
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| format!("Encodage PNG du calque « {} » : {e}", l.name))?;
        dtos.push(LayerDto {
            id: l.id,
            name: l.name.clone(),
            opacity: l.opacity,
            blend_mode: l.blend_mode.clone(),
            visible: l.visible,
            offset_x: l.offset_x,
            offset_y: l.offset_y,
            rotation: l.rotation,
            scale: l.scale,
            png_base64: base64::engine::general_purpose::STANDARD.encode(png),
        });
    }
    let file = ProjectFile {
        version: FORMAT_VERSION,
        width: doc_w,
        height: doc_h,
        layers: dtos,
    };
    let json =
        serde_json::to_vec_pretty(&file).map_err(|e| format!("Sérialisation du projet : {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("Écriture de {}: {e}", path.display()))
}

/// Charge un `.csophoto` et reconstruit la pile (aperçus/miniatures régénérés).
pub fn load(path: &Path) -> Result<LoadedProject, String> {
    let json = std::fs::read(path).map_err(|e| format!("Lecture de {}: {e}", path.display()))?;
    let file: ProjectFile =
        serde_json::from_slice(&json).map_err(|e| format!("Projet invalide : {e}"))?;
    if file.version != FORMAT_VERSION {
        return Err(format!(
            "Version de projet non supportée : {} (attendu {FORMAT_VERSION})",
            file.version
        ));
    }

    let source_name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .map(str::to_string);

    let mut layers = Vec::with_capacity(file.layers.len());
    for dto in file.layers {
        let png = base64::engine::general_purpose::STANDARD
            .decode(&dto.png_base64)
            .map_err(|e| format!("Calque « {} » corrompu : {e}", dto.name))?;
        let img: DynamicImage = image::load_from_memory(&png)
            .map_err(|e| format!("Calque « {} » illisible : {e}", dto.name))?;
        let mut layer = Layer::new(dto.id, dto.name, Arc::new(img));
        layer.opacity = dto.opacity.clamp(0.0, 100.0);
        layer.blend_mode = if BLEND_MODES.contains(&dto.blend_mode.as_str()) {
            dto.blend_mode
        } else {
            "Normal".into()
        };
        layer.visible = dto.visible;
        layer.offset_x = dto.offset_x;
        layer.offset_y = dto.offset_y;
        layer.rotation = dto.rotation;
        layer.scale = dto.scale.clamp(0.05, 8.0);
        layers.push(layer);
    }

    Ok(LoadedProject {
        width: file.width,
        height: file.height,
        layers,
        path: Some(path.to_path_buf()),
        source_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn sample_layers() -> Vec<Layer> {
        let img = DynamicImage::ImageRgba8({
            let mut b = RgbaImage::new(3, 2);
            b.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
            b
        });
        let mut l = Layer::new(42, "fond".into(), Arc::new(img));
        l.opacity = 75.0;
        l.offset_x = 12.5;
        vec![l]
    }

    #[test]
    fn aller_retour_projet() {
        let layers = sample_layers();
        let path = std::env::temp_dir().join(format!(
            "cso-test-{}.csophoto",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("horloge")
                .as_nanos()
        ));
        save(&path, &layers, 800, 600).expect("sauvegarde");

        let loaded = load(&path).expect("chargement");
        std::fs::remove_file(&path).ok();

        assert_eq!(loaded.width, 800);
        assert_eq!(loaded.height, 600);
        assert_eq!(loaded.layers.len(), 1);
        let l = &loaded.layers[0];
        assert_eq!((l.id, l.name.as_str()), (42, "fond"));
        assert_eq!(l.opacity, 75.0);
        assert_eq!(l.offset_x, 12.5);
        // Pixels identiques après l'aller-retour PNG
        assert_eq!(*l.image, *layers[0].image);
    }
}
