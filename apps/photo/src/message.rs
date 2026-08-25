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

use crate::layers::Layer;

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
    Generator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetAxis {
    X,
    Y,
}

/// Layer décodé (thread async) — Debug manuel car la texture n'est pas formattable
#[derive(Clone)]
pub struct DecodedLayer(pub Layer);
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
    pub layer_id: u64,
    pub tex: ui::image_canvas::StrokeTex,
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
    CloseNodeContextMenu,

    // Outils
    SelectTool(Tool),
    ImageCanvasEvent(ui::image_canvas::ImageCanvasEvent),

    // Calques
    SelectLayer(u64),
    ToggleLayerVisible(u64),
    SetLayerOpacity {
        id: u64,
        opacity: f32,
    },
    SetLayerBlend {
        id: u64,
        mode: String,
    },
    RenameLayer {
        id: u64,
        name: String,
    },
    SetLayerOffset {
        id: u64,
        axis: OffsetAxis,
        value: f32,
    },
    /// Rotation du calque (degrés, absolu)
    SetLayerRotation {
        id: u64,
        degrees: f32,
    },
    /// Rotation rapide ±90° (true = horaire)
    RotateLayer90 {
        id: u64,
        clockwise: bool,
    },
    /// Retourne le calque (miroir horizontal/vertical)
    FlipLayer {
        id: u64,
        horizontal: bool,
    },
    /// Rotation relative (delta en degrés, ex: 90, -90, 180)
    RotateLayer {
        id: u64,
        delta: f32,
    },
    /// Échelle uniforme du calque (1.0 = 100 %)
    SetLayerScale {
        id: u64,
        scale: f32,
    },
    /// Réinitialise rotation + échelle du calque
    ResetLayerTransform(u64),
    /// Rogne le calque sélectionné à la sélection rectangulaire active
    CropLayerToSelection,
    AddEmptyLayer,
    DuplicateLayer(u64),
    DeleteLayer(u64),
    MoveLayerUp(u64),
    MoveLayerDown(u64),

    // Image - utilise le picker natif via rfd
    OpenImage,
    ImagePicked(Option<std::path::PathBuf>),
    /// Fichier lu (async) — le décodage démarre ensuite
    ImageRead(Result<(Vec<u8>, String), String>),
    /// Image décodée + texture construite (async) — ajout à la pile
    ImageDecoded(Result<DecodedLayer, String>),
    // Projet .csphoto
    /// Chemin choisi pour l'ouverture (projet ou image)
    ProjectOpenPicked(Option<std::path::PathBuf>),
    /// Projet chargé hors thread UI — remplace le document courant
    ProjectOpened(Result<photo_engine::project::LoadedProject, String>),
    /// Enregistre au chemin courant (ou ouvre la boîte « Enregistrer sous »)
    SaveProjectPathPicked(Option<std::path::PathBuf>),
    /// Résultat d'un enregistrement (nom du fichier pour statut/erreur)
    ProjectSaved(Result<String, String>),
    /// Tick d'animation (spinner / barre de progression)
    TickFrame,
    /// Ouvre/ferme le menu des traitements en arrière-plan
    ToggleTaskMenu,

    // Raccourcis clavier (préférences)
    /// Ouvre la fenêtre Préférences → Raccourcis
    OpenPreferences,
    ClosePreferences,
    /// Démarre la capture d'une nouvelle combinaison pour l'action
    ShortcutCapture(ui::shortcuts::Action),
    /// Touche capturée (None = Échap → annule)
    ShortcutCaptured(Option<ui::shortcuts::Binding>),
    /// Annule la capture en cours
    ShortcutCancelCapture,
    /// Remet le raccourci par défaut d'une action
    ShortcutReset(ui::shortcuts::Action),
    /// Remet toute la table par défaut
    ShortcutResetAll,
    /// Raccourci clavier résolu → action sémantique
    ShortcutAction(ui::shortcuts::Action),

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
        tex: Option<ui::image_canvas::StrokeTex>,
        /// true = gomme (destination-out), false = pinceau
        erase: bool,
    },
    /// Résultat du calcul lourd — applique pixels + buffers au calque
    PaintApplied {
        layer_id: u64,
        buf: photo_engine::paint::StrokeCommit,
    },
    /// Le worker de peinture a échoué : retire l'aperçu figé sans panic
    PaintFailed {
        layer_id: u64,
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
    /// Section active dans la fenêtre Préférences
    PrefsSection(crate::components::preferences::PrefsSection),

    // Générateur de textures (graphe nodal)
    NodeGraphEvent(ui::node_graph::NodeGraphEvent),
    UpdateParam {
        node: datatypes::NodeId,
        key: String,
        value: datatypes::ParamValue,
    },
    AddNodeAt {
        type_id: String,
        world_pos: datatypes::Vec2,
    },
    DeleteSelectedNode,

    // Hardware
    DetectGpu,
    GpuDetected(String),
}
