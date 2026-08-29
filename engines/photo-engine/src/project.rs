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
//! FORMAT_VERSION 2 : arbre hiérarchique (LayerTree). Chaque nœud est un
//! calque pixels (source PNG + chaîne de live filters), un groupe (enfants
//! récursifs) ou un calque d'ajustement. Conteneur JSON versionné, images
//! encodées PNG puis base64.
//!
//! Politique de compatibilité : STRICTE — seuls les projets v2 sont lus ;
//! toute autre version est refusée avec un message clair.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use base64::Engine as _;
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::document::{
    AdjustmentLayer, BlendMode, Document, FilterNode, GroupLayer, LayerNode, PixelLayer,
    Transform2D,
};

/// Version du format — incrémenter à toute évolution incompatible.
pub const FORMAT_VERSION: u32 = 2;

/// Extension canonique des projets photo : `cso` (CreativeSuiteOpen) + `photo`.
pub const PROJECT_EXTENSION: &str = "csophoto";

/// Ancienne extension (pré-convention `cso*`) encore acceptée EN LECTURE.
pub const LEGACY_PROJECT_EXTENSION: &str = "csphoto";

/// L'extension du fichier correspond-elle à un projet photo (canonique
/// `.csophoto` ou héritée `.csphoto`) ? Insensible à la casse.
#[must_use]
pub fn is_project_path(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            ext == PROJECT_EXTENSION || ext == LEGACY_PROJECT_EXTENSION
        }
        None => false,
    }
}

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    version: u32,
    width: u32,
    height: u32,
    root: Vec<LayerNodeDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum LayerNodeDto {
    Pixel(PixelDto),
    Group(GroupDto),
    Adjustment(AdjustmentDto),
}

#[derive(Serialize, Deserialize)]
struct MaskDto {
    png_base64: String,
    enabled: bool,
    inverted: bool,
}

#[derive(Serialize, Deserialize)]
struct PixelDto {
    id: Uuid,
    name: String,
    /// Pixels de la SOURCE (jamais l'apparence filtrée) encodés PNG + base64
    png_base64: String,
    #[serde(default)]
    live_filters: Vec<FilterDto>,
    transform: Transform2D,
    opacity: f32,
    blend_mode: BlendMode,
    visible: bool,
    #[serde(default)]
    mask: Option<MaskDto>,
}

#[derive(Serialize, Deserialize)]
struct GroupDto {
    id: Uuid,
    name: String,
    children: Vec<LayerNodeDto>,
    #[serde(default)]
    collapsed: bool,
    opacity: f32,
    blend_mode: BlendMode,
    visible: bool,
    #[serde(default)]
    mask: Option<MaskDto>,
}

#[derive(Serialize, Deserialize)]
struct AdjustmentDto {
    id: Uuid,
    name: String,
    filters: Vec<FilterDto>,
    opacity: f32,
    visible: bool,
}

#[derive(Serialize, Deserialize)]
struct FilterDto {
    id: Uuid,
    type_id: String,
    params: HashMap<String, datatypes::ParamValue>,
    enabled: bool,
}

impl FilterNode {
    fn to_dto(&self) -> FilterDto {
        FilterDto {
            id: self.id,
            type_id: self.type_id.clone(),
            params: self.params.clone(),
            enabled: self.enabled,
        }
    }

    fn from_dto(dto: FilterDto) -> Self {
        Self {
            id: dto.id,
            type_id: dto.type_id,
            params: dto.params,
            enabled: dto.enabled,
        }
    }
}

fn png_encode(img: &DynamicImage, name: &str) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("Encodage PNG du calque « {name} » : {e}"))?;
    Ok(png)
}

fn png_decode(png_base64: &str, name: &str) -> Result<DynamicImage, String> {
    let png = base64::engine::general_purpose::STANDARD
        .decode(png_base64)
        .map_err(|e| format!("Calque « {name} » corrompu : {e}"))?;
    image::load_from_memory(&png).map_err(|e| format!("Calque « {name} » illisible : {e}"))
}

fn mask_to_dto(mask: &crate::document::LayerMask, name: &str) -> Result<MaskDto, String> {
    let dyn_img = image::DynamicImage::ImageRgba8((*mask.image).clone());
    Ok(MaskDto {
        png_base64: base64::engine::general_purpose::STANDARD.encode(png_encode(&dyn_img, name)?),
        enabled: mask.enabled,
        inverted: mask.inverted,
    })
}

fn mask_from_dto(dto: MaskDto, name: &str) -> Result<crate::document::LayerMask, String> {
    let img = png_decode(&dto.png_base64, name)?;
    Ok(crate::document::LayerMask {
        image: std::sync::Arc::new(img.to_rgba8()),
        enabled: dto.enabled,
        inverted: dto.inverted,
        version: crate::document::next_appearance_version(),
    })
}

fn node_to_dto(node: &LayerNode) -> Result<LayerNodeDto, String> {
    Ok(match node {
        LayerNode::Pixel(l) => {
            // Encodage de la SOURCE : les filtres restent vivants au rechargement
            LayerNodeDto::Pixel(PixelDto {
                id: l.id,
                name: l.name.clone(),
                png_base64: base64::engine::general_purpose::STANDARD
                    .encode(png_encode(l.source_image.as_ref(), &l.name)?),
                live_filters: l.live_filters.iter().map(FilterNode::to_dto).collect(),
                transform: l.transform,
                opacity: l.opacity,
                blend_mode: l.blend_mode,
                visible: l.visible,
                mask: l
                    .mask
                    .as_ref()
                    .map(|m| mask_to_dto(m, &l.name))
                    .transpose()?,
            })
        }
        LayerNode::Group(g) => {
            let children: Result<Vec<LayerNodeDto>, String> =
                g.children.iter().map(node_to_dto).collect();
            LayerNodeDto::Group(GroupDto {
                id: g.id,
                name: g.name.clone(),
                children: children?,
                collapsed: g.collapsed,
                opacity: g.opacity,
                blend_mode: g.blend_mode,
                visible: g.visible,
                mask: g
                    .mask
                    .as_ref()
                    .map(|m| mask_to_dto(m, &g.name))
                    .transpose()?,
            })
        }
        LayerNode::Adjustment(a) => LayerNodeDto::Adjustment(AdjustmentDto {
            id: a.id,
            name: a.name.clone(),
            filters: a.filters.iter().map(FilterNode::to_dto).collect(),
            opacity: a.opacity,
            visible: a.visible,
        }),
    })
}

fn sanitize_transform(t: Transform2D) -> Transform2D {
    Transform2D {
        offset_x: t.offset_x,
        offset_y: t.offset_y,
        rotation_deg: t.rotation_deg,
        scale: t.scale.clamp(0.05, 8.0),
    }
}

fn node_from_dto(dto: LayerNodeDto) -> Result<LayerNode, String> {
    Ok(match dto {
        LayerNodeDto::Pixel(p) => {
            let img = png_decode(&p.png_base64, &p.name)?;
            let mut layer = PixelLayer::new(p.name.clone(), Arc::new(img));
            layer.id = p.id;
            layer.live_filters = p
                .live_filters
                .into_iter()
                .map(FilterNode::from_dto)
                .collect();
            layer.transform = sanitize_transform(p.transform);
            layer.opacity = p.opacity.clamp(0.0, 100.0);
            layer.blend_mode = p.blend_mode;
            layer.visible = p.visible;
            layer.mask = p.mask.map(|m| mask_from_dto(m, &p.name)).transpose()?;
            LayerNode::Pixel(layer)
        }
        LayerNodeDto::Group(g) => {
            let children: Result<Vec<LayerNode>, String> =
                g.children.into_iter().map(node_from_dto).collect();
            let mut group = GroupLayer::new(g.name.clone(), children?);
            group.id = g.id;
            group.collapsed = g.collapsed;
            group.opacity = g.opacity.clamp(0.0, 100.0);
            group.blend_mode = g.blend_mode;
            group.visible = g.visible;
            group.mask = g.mask.map(|m| mask_from_dto(m, &g.name)).transpose()?;
            LayerNode::Group(group)
        }
        LayerNodeDto::Adjustment(a) => {
            let mut adj = AdjustmentLayer::new(
                a.name,
                a.filters.into_iter().map(FilterNode::from_dto).collect(),
            );
            adj.id = a.id;
            adj.opacity = a.opacity.clamp(0.0, 100.0);
            adj.visible = a.visible;
            LayerNode::Adjustment(adj)
        }
    })
}

/// Document rechargé depuis un `.csophoto`.
pub struct LoadedProject {
    pub document: Document,
    /// Chemin du fichier chargé (devient le chemin d'enregistrement courant)
    pub path: Option<std::path::PathBuf>,
    /// Nom de fichier sans extension — alimente le titre du canvas
    pub source_name: Option<String>,
}

impl Clone for LoadedProject {
    fn clone(&self) -> Self {
        let mut document = Document::new(self.document.width, self.document.height);
        document.restore_snapshot(self.document.snapshot());
        Self {
            document,
            path: self.path.clone(),
            source_name: self.source_name.clone(),
        }
    }
}

impl std::fmt::Debug for LoadedProject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedProject")
            .field("width", &self.document.width)
            .field("height", &self.document.height)
            .field("root", &self.document.root.len())
            .field("path", &self.path)
            .field("source_name", &self.source_name)
            .finish()
    }
}

/// Enregistre l'arbre de calques dans un fichier `.csophoto` (v2).
///
/// # Errors
/// Erreur d'encodage PNG d'un calque, de sérialisation JSON ou d'écriture
/// disque — message descriptif en français pour l'utilisateur.
pub fn save(path: &Path, doc: &Document) -> Result<(), String> {
    let root: Result<Vec<LayerNodeDto>, String> = doc.root.iter().map(node_to_dto).collect();
    let file = ProjectFile {
        version: FORMAT_VERSION,
        width: doc.width,
        height: doc.height,
        root: root?,
    };
    let json =
        serde_json::to_vec_pretty(&file).map_err(|e| format!("Sérialisation du projet : {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("Écriture de {}: {e}", path.display()))
}

/// Charge un `.csophoto`. Strict : seule la version courante est acceptée.
///
/// # Errors
/// Fichier illisible, JSON invalide, version étrangère (v1 incluse), ou
/// calque corrompu — message descriptif en français.
pub fn load(path: &Path) -> Result<LoadedProject, String> {
    let json = std::fs::read(path).map_err(|e| format!("Lecture de {}: {e}", path.display()))?;
    // Sonde de version AVANT désérialisation complète : un projet d'une
    // autre époque doit être refusé avec le bon message, pas avec une
    // erreur de champs manquants.
    #[derive(Deserialize)]
    struct VersionProbe {
        version: u32,
    }
    let probe: VersionProbe =
        serde_json::from_slice(&json).map_err(|e| format!("Projet invalide : {e}"))?;
    if probe.version != FORMAT_VERSION {
        return Err(format!(
            "Version de projet non supportée : {} (attendu {FORMAT_VERSION}). \
             Les projets au format 1 ne sont plus lisibles.",
            probe.version
        ));
    }
    let file: ProjectFile =
        serde_json::from_slice(&json).map_err(|e| format!("Projet invalide : {e}"))?;

    let root: Result<Vec<LayerNode>, String> = file.root.into_iter().map(node_from_dto).collect();
    let root = root?;

    let source_name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .map(str::to_string);

    let mut document = Document::new(file.width.max(1), file.height.max(1));
    document.restore(file.width, file.height, root);

    Ok(LoadedProject {
        document,
        path: Some(path.to_path_buf()),
        source_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use datatypes::ParamValue;
    use image::RgbaImage;

    fn red_img() -> Arc<DynamicImage> {
        Arc::new(DynamicImage::ImageRgba8({
            let mut b = RgbaImage::new(3, 2);
            b.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
            b
        }))
    }

    fn green_img() -> Arc<DynamicImage> {
        Arc::new(DynamicImage::ImageRgba8({
            let mut b = RgbaImage::new(2, 2);
            b.put_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
            b
        }))
    }

    /// Racine : [Groupe(haut, fond filtré), Ajustement(color_correct)]
    fn sample_document() -> Document {
        let mut doc = Document::new(800, 600);
        let mut haut = PixelLayer::new("haut", green_img());
        haut.transform.offset_y = -4.0;

        let mut fond = PixelLayer::new("fond", red_img());
        fond.opacity = 75.0;
        fond.transform.offset_x = 12.5;
        let mut filtre = FilterNode::new("brightness_contrast");
        filtre
            .params
            .insert("brightness".into(), ParamValue::Float(25.0));
        fond.live_filters.push(filtre);

        doc.push_layer(LayerNode::Group(GroupLayer::new(
            "groupe",
            vec![LayerNode::Pixel(haut), LayerNode::Pixel(fond)],
        )));
        doc.push_layer(LayerNode::Adjustment(AdjustmentLayer::new(
            "courbe",
            vec![FilterNode::new("color_correct")],
        )));
        doc
    }

    fn temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "cso-{tag}-{}.csophoto",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("horloge")
                .as_nanos()
        ))
    }

    #[test]
    fn aller_retour_projet_v2_conserve_arbre_et_filtres() {
        let doc = sample_document();
        let path = temp_path("rt");
        save(&path, &doc).expect("sauvegarde");

        let loaded = load(&path).expect("chargement");
        std::fs::remove_file(&path).ok();

        assert_eq!((loaded.document.width, loaded.document.height), (800, 600));
        // Racine : [Groupe(pixels ×2), Ajustement]
        assert_eq!(loaded.document.root.len(), 2);
        let Some(LayerNode::Group(g)) = loaded.document.root.first() else {
            panic!("groupe attendu à la racine");
        };
        assert_eq!(g.name, "groupe");
        assert_eq!(g.children.len(), 2);

        let LayerNode::Pixel(fond) = &g.children[1] else {
            panic!("calque pixels attendu");
        };
        assert_eq!(fond.id.to_string().len(), 36, "uuid préservé");
        assert_eq!(fond.opacity, 75.0);
        assert_eq!(fond.transform.offset_x, 12.5);
        assert_eq!(fond.blend_mode, BlendMode::Normal);
        assert_eq!(fond.live_filters.len(), 1);
        assert_eq!(fond.live_filters[0].type_id, "brightness_contrast");
        assert_eq!(
            fond.live_filters[0].params.get("brightness"),
            Some(&ParamValue::Float(25.0)),
            "paramètres de filtre préservés"
        );
        // Pixels identiques après l'aller-retour PNG
        let reference = sample_document();
        let Some(LayerNode::Group(gr)) = reference.root.first() else {
            panic!()
        };
        let LayerNode::Pixel(fond_ref) = &gr.children[1] else {
            panic!()
        };
        assert_eq!(*fond.source_image, *fond_ref.source_image);
    }

    #[test]
    fn detection_extension_projet_canonique_et_heritee() {
        assert!(is_project_path(Path::new("mon-projet.csophoto")));
        assert!(is_project_path(Path::new("MON-PROJET.CSOPHOTO")));
        // Ancienne extension encore reconnue en lecture
        assert!(is_project_path(Path::new("ancien.csphoto")));
        assert!(!is_project_path(Path::new("image.png")));
        assert!(!is_project_path(Path::new("sans-extension")));
    }

    #[test]
    fn version_etrangere_rejetee_proprement() {
        let path = temp_path("bad");
        std::fs::write(&path, r#"{"version":1,"width":4,"height":4,"layers":[]}"#)
            .expect("écriture fixture v1");
        let err = load(&path).expect_err("v1 doit être refusée");
        assert!(err.contains("non supportée"), "{err}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn apparence_regeneree_apres_chargement() {
        let doc = sample_document();
        let path = temp_path("appear");
        save(&path, &doc).expect("sauvegarde");
        let loaded = load(&path).expect("chargement");
        std::fs::remove_file(&path).ok();

        let Some(LayerNode::Group(g)) = loaded.document.root.first() else {
            panic!()
        };
        let LayerNode::Pixel(fond) = &g.children[1] else {
            panic!()
        };
        let id = fond.id;
        let appearance = loaded.document.appearance(id).expect("apparence");
        // preview aux dimensions source, thumb standard 48 px
        assert_eq!(
            (appearance.preview.width, appearance.preview.height),
            (3, 2)
        );
        assert_eq!(appearance.thumb.width, 48);
        // L'ajustement est bien présent et actif
        assert!(loaded.document.needs_fallback());
    }

    #[test]
    fn aller_retour_masque_conserve_pixels_et_flags() {
        let mut doc = Document::new(4, 4);
        let img = Arc::new(DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([10, 20, 30, 255]),
        )));
        let mut layer = PixelLayer::new("masqué", img);
        let mut mask = crate::document::LayerMask::full(2, 2);
        mask.enabled = false;
        mask.inverted = true;
        // pixel distinct pour vérifier PNG
        mask.image = std::sync::Arc::new(RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 255])));
        layer.mask = Some(mask);
        doc.push_layer(LayerNode::Pixel(layer));
        let path = temp_path("mask");
        save(&path, &doc).expect("save masque");
        let loaded = load(&path).expect("load masque");
        std::fs::remove_file(&path).ok();
        let Some(LayerNode::Pixel(l)) = loaded.document.root.first() else {
            panic!("pixel attendu");
        };
        let m = l.mask.as_ref().expect("masque préservé");
        assert!(!m.enabled);
        assert!(m.inverted);
        assert_eq!(m.image.get_pixel(0, 0)[0], 0);
    }
}
