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

//! État applicatif Photo : document, canvas, outils, préférences.

use iced::widget::{image as iced_image, pane_grid};
use iced::{Color, Point, Rectangle, Size, Task, Vector};

use crate::components;
use crate::layers::Layer;
use crate::message::{Message, PanelType, PendingPaint, Tool};

pub struct PhotoApp {
    pub zoom_level: u32,
    pub panes: pane_grid::State<PanelType>,
    pub focus: Option<pane_grid::Pane>,
    // ---- Pile de calques (index 0 = bas de la pile) ----
    pub layers: Vec<Layer>,
    pub next_layer_id: u64,
    pub selected_layer: Option<u64>,
    /// Dimensions du document (fixées par la première image ouverte) — pour export/titre
    pub doc_size: Option<(u32, u32)>,
    /// Taille du composite fallback (modes de fusion non-Normal)
    pub fallback_size: Option<Size>,
    /// Composite CPU unique — UNIQUEMENT si un calque visible a un mode
    /// de fusion non-Normal (sinon chemin rapide par calque, zéro recomposite)
    pub fallback_handle: Option<iced_image::Handle>,
    pub image_path: Option<String>,
    pub image_error: Option<String>,
    // Canvas interaction (outil Main + pan/zoom)
    pub selected_tool: Tool,
    pub canvas_pan: Vector,
    pub color_profile: String,
    pub canvas_selection: Option<Rectangle>,
    /// Taille du viewport du canvas (publiée par le widget image_canvas)
    pub canvas_viewport: Size,
    /// Barre d'outils flottante visible ou masquée
    pub tools_visible: bool,
    /// Ancre de déplacement du calque sélectionné (outil Déplacer)
    pub move_anchor: Option<(u64, f32, f32)>,
    /// Fond composite PRÉ-CALCULÉ au début du drag (sans le calque déplacé).
    /// Pendant le drag : zéro recomposite — on dessine ce fond + le calque
    /// par-dessus. Le vrai blend est recalculé au relâchement.
    pub drag_background: Option<iced_image::Handle>,
    pub drag_background_size: Option<Size>,
    /// Traitements en arrière-plan (libellés affichés dans le menu du spinner)
    pub background_tasks: Vec<String>,
    /// Menu des tâches ouvert (clic sur le spinner)
    pub task_menu_open: bool,
    /// Angle du spinner d'activité (animé par TickFrame)
    pub spinner_angle: f32,
    /// Table de raccourcis clavier (persistée en JSON)
    pub shortcuts: ui::shortcuts::Shortcuts,
    /// Fenêtre principale — son Id (ouverte au boot)
    pub main_window: Option<iced::window::Id>,
    // ---- Historique (undo/redo) ----
    /// Historique du DOCUMENT (calques + dimensions). Les états sont des
    /// snapshots bon marché : les pixels sont partagés via Arc.
    pub history: photo_engine::history::History,
    /// Chemin du projet .csphoto courant (None = jamais enregistré)
    pub project_path: Option<std::path::PathBuf>,
    // ---- Pinceau ----
    pub brush_color: Color,
    /// Diamètre du pinceau en pixels DOCUMENT
    pub brush_size: f32,
    /// Opacité globale du trait [0.05..1]
    pub brush_opacity: f32,
    pub color_picker_open: bool,
    /// Trait en cours : calque cible + polyligne en coordonnées DOCUMENT
    pub stroke_layer: Option<u64>,
    /// Commit lourd EN COURS hors thread UI — l'aperçu reste figé à l'écran
    /// jusqu'à l'application (aucun gel de l'interface).
    pub pending_paint: Option<PendingPaint>,

    // ---- Écran d'accueil (nouveau document) ----
    pub new_doc_w: String,
    pub new_doc_h: String,
    pub welcome_error: Option<String>,

    /// Modal Préférences ouverte
    pub show_prefs: bool,
    /// Section active dans la fenêtre Préférences
    pub prefs_section: components::preferences::PrefsSection,
    /// Capture de touche en cours (action en attente d'un nouveau raccourci)
    pub capturing: Option<ui::shortcuts::Action>,
    // ---- Générateur de textures (graphe nodal — futur usage filtres/génération) ----
    pub gen_graph: suite_core::Graph,
    pub gen_selected_node: Option<datatypes::NodeId>,
    pub gen_previews: std::collections::HashMap<datatypes::NodeId, iced_image::Handle>,
    pub node_context_menu: Option<Point>,
    pub node_context_world: Option<datatypes::Vec2>,
    pub pending_connect: Option<(datatypes::NodeId, String, datatypes::SocketType, bool)>,
    // Options / Hardware
    pub gpu_info: Option<String>,
    pub gpu_available: bool,

    /// Handles iced par calque (cache dérivé des buffers purs du moteur —
    /// voir `crate::ui_handles` ; purgé/reconstruit automatiquement).
    pub preview_cache: crate::ui_handles::PreviewCache,
}

impl PhotoApp {
    /// Boot daemon : le daemon n'ouvre AUCUNE fenêtre automatiquement —
    /// la fenêtre principale doit être créée ici via `window::open`
    /// (cf. iced examples/multi_window).
    // L'Id de fenêtre n'existe qu'APRÈS `Self::default()` (layout des
    // panneaux) : la réassignation est le pattern demandé par iced.
    #[allow(clippy::field_reassign_with_default)]
    pub fn new() -> (Self, Task<Message>) {
        let (main_id, open) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(1280.0, 820.0),
            min_size: Some(iced::Size::new(960.0, 600.0)),
            ..iced::window::Settings::default()
        });
        let mut app = Self::default();
        app.main_window = Some(main_id);
        app.history.reset();
        (app, open.map(|_| Message::MockAction))
    }

    pub(crate) fn alloc_layer_id(&mut self) -> u64 {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        id
    }

    pub(crate) fn layer_index(&self, id: u64) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub(crate) fn selected_layer_mut(&mut self) -> Option<&mut Layer> {
        let sel = self.selected_layer?;
        self.layer_index(sel).map(|i| &mut self.layers[i])
    }

    /// Snapshot complet du document pour l'historique (pixels partagés via Arc).
    pub(crate) fn snapshot(&self) -> photo_engine::history::Snapshot {
        photo_engine::history::Snapshot {
            doc_size: self.doc_size,
            layers: self.layers.clone(),
        }
    }

    /// Raccourci clavier → Message applicatif.
    /// SEULE correspondance action ↔ logique : ajouter une action ici la
    /// branche au clavier partout.
    pub(crate) fn message_for(action: ui::shortcuts::Action) -> Option<Message> {
        use ui::shortcuts::Action;
        let sel = None; // sélection courante indisponible hors update (abonnement statique)
        match action {
            Action::NewProject => Some(Message::NewProject),
            Action::Open => Some(Message::OpenProject),
            Action::Save => Some(Message::SaveProject),
            Action::SaveAs => Some(Message::SaveProjectAs),
            Action::Quit => Some(Message::Quit),
            Action::Undo => Some(Message::Undo),
            Action::Redo => Some(Message::Redo),
            Action::Preferences => Some(Message::OpenPreferences),
            Action::ToggleTools => Some(Message::ToggleToolsPanel),
            Action::ToggleLayersPanel => Some(Message::TogglePanel(PanelType::Layers)),
            Action::TogglePropertiesPanel => Some(Message::TogglePanel(PanelType::Properties)),
            Action::LayerNew => Some(Message::AddEmptyLayer),
            Action::LayerDuplicate => sel.map(Message::DuplicateLayer),
            Action::LayerDelete => sel.map(Message::DeleteLayer),
            Action::LayerMoveUp => sel.map(Message::MoveLayerUp),
            Action::LayerMoveDown => sel.map(Message::MoveLayerDown),
            Action::Rotate90 => sel.map(|id| Message::RotateLayer { id, delta: 90.0 }),
            Action::Rotate180 => sel.map(|id| Message::RotateLayer { id, delta: 180.0 }),
            Action::RotateN90 => sel.map(|id| Message::RotateLayer { id, delta: -90.0 }),
            Action::RotateN180 => sel.map(|id| Message::RotateLayer { id, delta: -180.0 }),
            Action::ResetTransform => sel.map(Message::ResetLayerTransform),
            Action::CropToSelection => Some(Message::CropLayerToSelection),
            Action::ToolBrush => Some(Message::SelectTool(Tool::Brush)),
            Action::ToolEraser => Some(Message::SelectTool(Tool::Eraser)),
            Action::ToolHand => Some(Message::SelectTool(Tool::Hand)),
            Action::ZoomIn => Some(Message::ZoomInPressed),
            Action::ZoomOut => Some(Message::ZoomOutPressed),
            Action::FitToScreen => Some(Message::CanvasFit),
        }
    }

    /// Un calque visible a-t-il un mode de fusion non-Normal ?
    /// (l'opacité est gérée au draw, elle ne force plus le fallback)
    pub(crate) fn needs_fallback(&self) -> bool {
        self.layers
            .iter()
            .any(|l| l.visible && l.blend_mode != "Normal")
    }

    /// Composite CPU — UNIQUEMENT pour les modes de fusion non-Normal
    /// (le chemin rapide par calque couvre Normal/opacité sans recomposite).
    pub(crate) fn refresh_fallback(&mut self) {
        use photo_engine::document::{LayerData, composite_preview};
        if !self.needs_fallback() {
            self.fallback_handle = None;
            self.fallback_size = None;
            return;
        }
        let data: Vec<LayerData> = self.layers.iter().map(LayerData::from).collect();
        let (doc_w, doc_h) = self.doc_size.unwrap_or((800, 600));
        if let Some(img) = composite_preview(&data, doc_w, doc_h) {
            let (w, h) = (img.width() as f32, img.height() as f32);
            let rgba = img.to_rgba8();
            self.fallback_size = Some(Size::new(w, h));
            self.fallback_handle = Some(iced_image::Handle::from_rgba(
                rgba.width(),
                rgba.height(),
                rgba.into_raw(),
            ));
        } else {
            self.fallback_size = None;
            self.fallback_handle = None;
        }
    }

    /// Pré-calcule le fond composite SANS le calque sur le point d'être
    /// déplacé — appelé UNE FOIS au début du drag (MoveLayerStart).
    /// Pendant tout le drag, ce fond est dessiné tel quel + le calque
    /// déplacé par-dessus : zéro recomposite, drag fluide même en
    /// fallback (fusion non-Normal) sur de grosses images.
    pub(crate) fn prepare_drag_background(&mut self, exclude_id: u64) {
        use photo_engine::document::{LayerData, composite_preview};
        self.drag_background = None;
        self.drag_background_size = None;
        let data: Vec<LayerData> = self
            .layers
            .iter()
            .filter(|l| l.id != exclude_id && l.visible)
            .map(LayerData::from)
            .collect();
        let (doc_w, doc_h) = self.doc_size.unwrap_or((800, 600));
        if let Some(img) = composite_preview(&data, doc_w, doc_h) {
            let (w, h) = (img.width() as f32, img.height() as f32);
            let rgba = img.to_rgba8();
            self.drag_background_size = Some(Size::new(w, h));
            self.drag_background = Some(iced_image::Handle::from_rgba(
                rgba.width(),
                rgba.height(),
                rgba.into_raw(),
            ));
        }
    }
}

impl Default for PhotoApp {
    fn default() -> Self {
        // Layout : Canvas à gauche, à droite Propriétés (haut) + Calques (bas)
        let (mut panes, canvas_pane) = pane_grid::State::new(PanelType::Canvas);

        let (right_pane, _split_canvas_right) = panes
            .split(pane_grid::Axis::Vertical, canvas_pane, PanelType::Layers)
            .expect("Erreur lors de l'ajout du panneau Calques");

        let (_props_pane, _split_right_panel) = panes
            .split(
                pane_grid::Axis::Horizontal,
                right_pane,
                PanelType::Properties,
            )
            .expect("Erreur lors de l'ajout des Propriétés");

        panes.resize(_split_canvas_right, 0.74);
        panes.resize(_split_right_panel, 0.55);

        Self {
            zoom_level: 100,
            panes,
            focus: Some(canvas_pane),
            layers: Vec::new(),
            next_layer_id: 0,
            selected_layer: None,
            doc_size: None,
            fallback_size: None,
            fallback_handle: None,
            image_path: None,
            image_error: None,
            selected_tool: Tool::Hand,
            canvas_pan: Vector::new(0.0, 0.0),
            color_profile: "sRGB IEC61966-2.1".into(),
            canvas_selection: None,
            canvas_viewport: Size::new(800.0, 600.0),
            tools_visible: true,
            move_anchor: None,
            drag_background: None,
            drag_background_size: None,
            background_tasks: Vec::new(),
            task_menu_open: false,
            spinner_angle: 0.0,
            shortcuts: ui::shortcuts::Shortcuts::load(),
            main_window: None,
            history: photo_engine::history::History::new(),
            project_path: None,
            brush_color: Color::from_rgb8(0x1E, 0x1E, 0x22),
            brush_size: 12.0,
            brush_opacity: 1.0,
            color_picker_open: false,
            stroke_layer: None,
            pending_paint: None,
            new_doc_w: "1920".to_string(),
            new_doc_h: "1080".to_string(),
            welcome_error: None,
            show_prefs: false,
            prefs_section: components::preferences::PrefsSection::Shortcuts,
            capturing: None,
            gen_graph: components::node_registry::create_empty_graph(),
            gen_selected_node: None,
            gen_previews: Default::default(),
            node_context_menu: None,
            node_context_world: None,
            pending_connect: None,
            gpu_info: None,
            gpu_available: components::gpu::GpuContext::is_available(),
            preview_cache: crate::ui_handles::PreviewCache::default(),
        }
    }
}
