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

//! Messages applicatifs + petits types partagés (outils, panneaux).

use iced::Color;
use iced::widget::pane_grid;
use std::sync::Arc;
use uuid::Uuid;

use crate::layers::PixelLayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Hand,
    Zoom,
    Select,
    Eyedropper,
    Move,
    /// Pinceau : peint sur le calque sélectionné
    Brush,
    /// Gomme : efface (réduit l'alpha) sur le calque sélectionné
    Eraser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelType {
    Canvas,
    Properties,
    Layers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetAxis {
    X,
    Y,
}

/// Calque pixels décodé (thread async) — Debug manuel car la texture n'est pas formattable
#[derive(Clone)]
pub struct DecodedLayer(pub PixelLayer);
impl std::fmt::Debug for DecodedLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (w, h) = self.0.dimensions();
        f.debug_struct("DecodedLayer")
            .field("id", &self.0.id)
            .field("dims", &(w, h))
            .finish()
    }
}

/// Trait terminé dont les pixels sont en cours de fusion hors thread UI.
/// La texture d'aperçu (rastérisée par le canvas) reste affichée telle
/// quelle jusqu'à PaintApplied — continuité visuelle parfaite.
#[derive(Clone)]
pub struct PendingPaint {
    pub layer_id: Uuid,
    /// Masque ciblé si le trait peignait un masque (None = pixels du calque).
    pub mask_id: Option<Uuid>,
    pub tex: ui_kit::image_canvas::StrokeTex,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Keyboard Actions
    ZoomInPressed,
    ZoomOutPressed,
    MockAction,

    // Panel Actions
    TogglePanel(PanelType),
    PaneResized(pane_grid::ResizeEvent),
    PaneDragged(pane_grid::DragEvent),
    PaneClicked(pane_grid::Pane),
    ClosePane(pane_grid::Pane),

    // UI Toolbar
    NewProject,
    OpenProject,
    SaveProject,
    SaveProjectAs,
    Quit,
    Undo,
    Redo,

    // Mouse & Context Menu Actions
    /// Ajuste le zoom/pan pour voir toute l'image dans le viewport
    CanvasFit,
    /// Affiche/masque la barre d'outils flottante
    ToggleToolsPanel,

    // Outils
    SelectTool(Tool),
    ImageCanvasEvent(ui_kit::image_canvas::ImageCanvasEvent),

    // Calques (arbre LayerTree)
    SelectLayer(Uuid),
    ToggleLayerVisible(Uuid),
    SetLayerOpacity {
        id: Uuid,
        opacity: f32,
    },
    SetLayerBlend {
        id: Uuid,
        mode: crate::layers::BlendMode,
    },
    RenameLayer {
        id: Uuid,
        name: String,
    },
    SetLayerOffset {
        id: Uuid,
        axis: OffsetAxis,
        value: f32,
    },
    /// Rotation du calque (degrés, absolu)
    SetLayerRotation {
        id: Uuid,
        degrees: f32,
    },
    /// Rotation rapide ±90° (true = horaire)
    RotateLayer90 {
        id: Uuid,
        clockwise: bool,
    },
    /// Retourne le calque (miroir horizontal/vertical)
    FlipLayer {
        id: Uuid,
        horizontal: bool,
    },
    /// Rotation relative (delta en degrés, ex: 90, -90, 180)
    RotateLayer {
        id: Uuid,
        delta: f32,
    },
    /// Échelle uniforme du calque (1.0 = 100 %)
    SetLayerScale {
        id: Uuid,
        scale: f32,
    },
    /// Réinitialise rotation + échelle du calque
    ResetLayerTransform(Uuid),
    /// Rogne le calque sélectionné à la sélection rectangulaire active
    CropLayerToSelection,
    AddEmptyLayer,
    AddSolidColorLayer,
    DuplicateLayer(Uuid),
    DeleteLayer(Uuid),
    MoveLayerUp(Uuid),
    MoveLayerDown(Uuid),
    /// Regroupe le nœud donné dans un nouveau groupe
    GroupLayers(Uuid),
    /// Dissout le groupe donné : ses enfants remontent d'un cran
    UngroupLayers(Uuid),
    /// Replie/déplie un groupe dans le panneau Calques
    ToggleGroupCollapsed(Uuid),

    // Live filters / calques d'ajustement
    /// Ajoute un filtre dynamique en fin de chaîne du nœud
    AddLiveFilter {
        id: Uuid,
        type_id: String,
    },
    /// Retire un filtre de la chaîne
    RemoveLiveFilter {
        layer_id: Uuid,
        filter_id: Uuid,
    },
    /// Réglage continu d'un paramètre de filtre (coalescé)
    SetFilterParam {
        layer_id: Uuid,
        filter_id: Uuid,
        key: String,
        value: datatypes::ParamValue,
    },
    /// Active/désactive un filtre sans perdre ses réglages
    ToggleFilterEnabled {
        layer_id: Uuid,
        filter_id: Uuid,
    },

    // Image - utilise le picker natif via rfd
    OpenImage,
    ImagePicked(Option<std::path::PathBuf>),
    /// Fichier lu (async) — le décodage démarre ensuite
    ImageRead(Result<(Vec<u8>, String), String>),
    /// Image décodée + texture construite (async) — ajout à l'arbre
    ImageDecoded(Result<DecodedLayer, String>),
    // Projet .csophoto
    /// Chemin choisi pour l'ouverture (projet ou image)
    ProjectOpenPicked(Option<std::path::PathBuf>),
    /// Projet chargé hors thread UI — remplace le document courant
    ProjectOpened(Result<photo_engine::project::LoadedProject, String>),
    /// Enregistre au chemin courant (ou ouvre la boîte « Enregistrer sous »)
    SaveProjectPathPicked(Option<std::path::PathBuf>),
    /// Résultat d'un enregistrement (nom du fichier pour statut/erreur)
    ProjectSaved(Result<String, String>),
    /// Opération destructrice terminée (Flip, Crop, etc.) — porte les
    /// nouveaux buffers (source + masques). Les échecs transportent un
    /// message d'erreur ; le calque reste inchangé.
    DestructiveOpComputed {
        layer_id: Uuid,
        op: DestructiveOp,
        result: Result<DestructiveResult, String>,
    },
    /// Ouvre la boîte « Exporter l'image » (PNG/JPEG)
    ExportImage,
    /// Chemin d'export choisi — le décodage du format vient de l'extension
    ExportPathPicked(Option<std::path::PathBuf>),
    /// Résultat d'un export (nom du fichier ou erreur)
    ImageExported(Result<String, String>),
    /// Tick d'animation (spinner / barre de progression)
    TickFrame,
    /// Événement plateforme « non consommé », taggé par sa fenêtre d'origine
    Event {
        event: iced::Event,
        window: iced::window::Id,
    },
    /// La fenêtre de préférences vient d'être créée par l'OS
    WindowOpened(iced::window::Id),
    /// Une fenêtre a été fermée (croix OS incluse)
    WindowClosed(iced::window::Id),
    /// Action raccourci résolue → dispatch vers les messages existants
    ExecuteAction(preferences::PhotoAction),
    /// Message interne de la fenêtre de préférences flottante
    PreferencesMsg(crate::preferences_window::Message),
    /// Rapport matériel calculé hors thread UI
    HardwareDetected(preferences::HardwareReport),
    /// Composite fallback calculée HORS thread UI (génération : anti-désync)
    FallbackComputed {
        generation: u64,
        result: Result<Option<(Vec<u8>, u32, u32)>, String>,
    },
    /// Fond de drag (composite sans le sous-arbre déplacé) prêt
    DragBackgroundComputed {
        layer_id: Uuid,
        result: Option<(Vec<u8>, u32, u32)>,
    },
    /// Composite du calque seul (avec masque) prêt — affiché en surimpression
    /// pendant le drag en mode fallback, pour préserver le rendu du masque.
    DragLayerCompositeComputed {
        layer_id: Uuid,
        result: Option<(Vec<u8>, u32, u32)>,
    },
    /// Ouvre/ferme le menu des traitements en arrière-plan
    ToggleTaskMenu,

    /// Ouvre la fenêtre de préférences flottante
    OpenPreferences,

    // ---- Pinceau / Gomme ----
    /// Début d'un trait (coordonnées document)
    BrushStart {
        x: f32,
        y: f32,
        /// true = gomme (destination-out), false = pinceau
        erase: bool,
    },
    /// Relâchement : lance le commit des pixels HORS thread UI
    BrushEnd {
        points: Vec<(f32, f32)>,
        tex: Option<ui_kit::image_canvas::StrokeTex>,
        /// true = gomme (destination-out), false = pinceau
        erase: bool,
    },
    /// Résultat du calcul lourd — applique pixels + buffers au calque
    PaintApplied {
        layer_id: Uuid,
        mask_id: Option<Uuid>,
        buf: photo_engine::paint::StrokeCommit,
    },
    /// Le worker de peinture a échoué : retire l'aperçu figé sans panic
    PaintFailed {
        layer_id: Uuid,
        mask_id: Option<Uuid>,
    },
    SetBrushColor(Color),
    SetBrushSize(f32),
    SetBrushOpacity(f32),
    ToggleColorPicker,

    // ---- Écran d'accueil ----
    NewDocWidth(String),
    NewDocHeight(String),
    /// Preset : fixe largeur + hauteur d'un coup
    SetDocPreset {
        w: u32,
        h: u32,
    },
    /// Crée le document : fond blanc plein cadre + calque sélectionné
    CreateDocument,

    // Drag & drop calques
    SetDraggedLayer(Uuid),
    DropLayerOn(Uuid),
    // Document
    ShowResizeDialog,
    SetResizeWidth(String),
    SetResizeHeight(String),
    ResizeDocument {
        width: u32,
        height: u32,
    },
    ReorderLayer {
        dragged: Uuid,
        target: Uuid,
        before: bool,
    },

    // Hardware
    DetectGpu,
    GpuDetected(String),

    // Masques de calque : un calque peut en porter plusieurs ; les messages
    // ciblent un masque précis par (layer_id, mask_id).
    SetActiveMask(Option<MaskTarget>),
    AddLayerMask(Uuid),
    AddLayerMaskComputed {
        layer_id: Uuid,
        mask: photo_engine::LayerMask,
    },
    RemoveLayerMask(Uuid, Uuid),
    ToggleLayerMaskEnabled(Uuid, Uuid),
    InvertLayerMask(Uuid, Uuid),
    /// Déplie/replie la liste des masques d'un calque dans le panneau Calques.
    ToggleMaskList(Uuid),
    /// Bascule la couleur du pinceau masque entre noir et blanc.
    ToggleMaskColor,
}

/// Identifie un masque précis parmi les N masques d'un calque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaskTarget {
    pub layer_id: Uuid,
    pub mask_id: Uuid,
}

/// Type d'opération destructrice asynchrone (Flip, Crop).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructiveOp {
    FlipHorizontal,
    FlipVertical,
    Crop,
}

/// Résultat d'une opération destructrice calculée hors thread UI.
/// Contient le nouveau buffer source et les nouveaux buffers de masques
/// (un par masque existant, dans le même ordre). Pour Crop, contient
/// aussi le décalage de transform à appliquer.
#[derive(Debug, Clone)]
pub struct DestructiveResult {
    pub source: Arc<image::DynamicImage>,
    pub masks: Vec<Arc<image::DynamicImage>>,
    /// Décalage de transform à ajouter (Crop seulement ; `(0, 0)` pour Flip).
    pub offset_delta: (f32, f32),
}
