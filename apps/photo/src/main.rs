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

use crate::layers::Layer;
use datatypes::{NodeId, ParamValue};
use iced::widget::{image as iced_image, pane_grid};

use iced::{Alignment, Element, Length, Point, Rectangle, Size, Subscription, Task, Vector};
use std::sync::Arc;

mod components;
mod layers;

/// Menus applicatifs affichés dans le shell (Fichier / Édition / Affichage).
fn app_menus(tools_visible: bool, selected_layer: Option<u64>) -> Vec<ui::menu::Menu<Message>> {
    vec![
        ui::menu::Menu::new(
            "Fichier",
            vec![
                ui::menu::Item::Action {
                    label: "Nouveau".into(),
                    shortcut: "Ctrl+N".to_string(),
                    checked: false,
                    message: Message::NewProject,
                },
                ui::menu::Item::Action {
                    label: "Ouvrir...".into(),
                    shortcut: "Ctrl+O".to_string(),
                    checked: false,
                    message: Message::OpenProject,
                },
                ui::menu::Item::Action {
                    label: "Ouvrir un élément récent".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Enregistrer".into(),
                    shortcut: "Ctrl+S".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Action {
                    label: "Enregistrer sous...".into(),
                    shortcut: "Ctrl+Maj+S".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Quitter".into(),
                    shortcut: "Ctrl+Q".to_string(),
                    checked: false,
                    message: Message::Quit,
                },
            ],
        ),
        ui::menu::Menu::new(
            "Édition",
            vec![
                ui::menu::Item::Action {
                    label: "Annuler".into(),
                    shortcut: "Ctrl+Z".to_string(),
                    checked: false,
                    message: Message::Undo,
                },
                ui::menu::Item::Action {
                    label: "Rétablir".into(),
                    shortcut: "Ctrl+Y".to_string(),
                    checked: false,
                    message: Message::Redo,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Couper".into(),
                    shortcut: "Ctrl+X".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Action {
                    label: "Copier".into(),
                    shortcut: "Ctrl+C".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Action {
                    label: "Coller".into(),
                    shortcut: "Ctrl+V".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Préférences...".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::OpenPreferences,
                },
            ],
        ),
        ui::menu::Menu::new(
            "Calque",
            vec![
                ui::menu::Item::Action {
                    label: "Nouveau calque vide".into(),
                    shortcut: "Ctrl+Maj+N".to_string(),
                    checked: false,
                    message: Message::AddEmptyLayer,
                },
                ui::menu::Item::Action {
                    label: "Calque depuis une image...".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::OpenImage,
                },
                ui::menu::Item::Action {
                    label: "Dupliquer le calque".into(),
                    shortcut: "Ctrl+J".to_string(),
                    checked: false,
                    message: Message::DuplicateLayer(selected_layer.unwrap_or(u64::MAX)),
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Supprimer le calque".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::DeleteLayer(selected_layer.unwrap_or(u64::MAX)),
                },
                ui::menu::Item::Separator,
                ui::menu::Item::SubMenu {
                    label: "Transformation".into(),
                    items: vec![
                        ui::menu::Item::Action {
                            label: "Rotation 90°".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::RotateLayer { id: selected_layer.unwrap_or(u64::MAX), delta: 90.0 },
                        },
                        ui::menu::Item::Action {
                            label: "Rotation 180°".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::RotateLayer { id: selected_layer.unwrap_or(u64::MAX), delta: 180.0 },
                        },
                        ui::menu::Item::Action {
                            label: "Rotation -90°".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::RotateLayer { id: selected_layer.unwrap_or(u64::MAX), delta: -90.0 },
                        },
                        ui::menu::Item::Action {
                            label: "Rotation -180°".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::RotateLayer { id: selected_layer.unwrap_or(u64::MAX), delta: -180.0 },
                        },
                        ui::menu::Item::Separator,
                        ui::menu::Item::Action {
                            label: "Réinitialiser transformation".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::ResetLayerTransform(selected_layer.unwrap_or(u64::MAX)),
                        },
                    ],
                },
            ],
        ),
        ui::menu::Menu::new(
            "Affichage",
            vec![
                ui::menu::Item::Action {
                    label: "Propriétés".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::TogglePanel(PanelType::Properties),
                },
                ui::menu::Item::Action {
                    label: "Calques".into(),
                    shortcut: "F7".to_string(),
                    checked: false,
                    message: Message::TogglePanel(PanelType::Layers),
                },
                ui::menu::Item::Action {
                    label: "Générateur de textures".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::TogglePanel(PanelType::Generator),
                },
                ui::menu::Item::Action {
                    label: "Barre d'outils".into(),
                    shortcut: "Tab".to_string(),
                    checked: tools_visible,
                    message: Message::ToggleToolsPanel,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Réinitialiser l'interface".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
            ],
        ),
    ]
}

pub fn main() -> iced::Result {
    // Force rayon à utiliser tous les cœurs
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(cores)
        .thread_name(|i| format!("rayon-photo-{}", i))
        .build_global();
    // Warmup GPU en arrière-plan pour que le canvas principal intègre wgpu dès le démarrage
    std::thread::spawn(|| {
        let _ = crate::components::gpu::GpuContext::get();
    });
    // Daemon : multi-fenêtres (principale + Préférences), cf. examples/multi_window
    iced::daemon(PhotoApp::new, update, view)
        .title(|_app: &PhotoApp, _window: iced::window::Id| {
            "Creative Suite Open Photo".to_string()
        })
        .subscription(subscription)
        .font(include_bytes!(
            "../../../assets/fonts/MaterialIcons-Regular.ttf"
        ))
        .font(include_bytes!(
            "../../../assets/fonts/HankenGrotesk-Regular.ttf"
        ))
        .font(include_bytes!(
            "../../../assets/fonts/HankenGrotesk-SemiBold.ttf"
        ))
        .font(include_bytes!(
            "../../../assets/fonts/HankenGrotesk-Bold.ttf"
        ))
        .default_font(ui::theme::fonts::SANS)
        .run()
}

/// Tick d'animation uniquement pendant un chargement (spinner + barre)
fn subscription(app: &PhotoApp) -> Subscription<Message> {
    let tick = if !app.background_tasks.is_empty() {
        iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::TickFrame)
    } else {
        Subscription::none()
    };
    Subscription::batch([
        tick,
        ui::shortcuts::subscription(
            &app.shortcuts,
            app.capturing.is_some(),
            PhotoApp::message_for,
            Message::ShortcutCaptured,
        ),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Hand,
    Zoom,
    Select,
    Eyedropper,
    Move,
}

struct PhotoApp {
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
    /// Modal Préférences ouverte
    pub show_prefs: bool,
    /// Section active dans la fenêtre Préférences
    pub prefs_section: components::preferences::PrefsSection,
    /// Capture de touche en cours (action en attente d'un nouveau raccourci)
    pub capturing: Option<ui::shortcuts::Action>,
    // ---- Générateur de textures (graphe nodal — futur usage filtres/génération) ----
    pub gen_graph: suite_core::Graph,
    pub gen_selected_node: Option<NodeId>,
    pub gen_previews: std::collections::HashMap<NodeId, iced_image::Handle>,
    pub node_context_menu: Option<Point>,
    pub node_context_world: Option<datatypes::Vec2>,
    pub pending_connect: Option<(NodeId, String, datatypes::SocketType, bool)>,
    // Options / Hardware
    pub gpu_info: Option<String>,
    pub gpu_available: bool,
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
    SetLayerRotation { id: u64, degrees: f32 },
    /// Rotation rapide ±90° (true = horaire)
    RotateLayer90 { id: u64, clockwise: bool },
    /// Rotation relative (delta en degrés, ex: 90, -90, 180)
    RotateLayer { id: u64, delta: f32 },
    /// Échelle uniforme du calque (1.0 = 100 %)
    SetLayerScale { id: u64, scale: f32 },
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
    /// Section active dans la fenêtre Préférences
    PrefsSection(components::preferences::PrefsSection),

    // Générateur de textures (graphe nodal)
    NodeGraphEvent(ui::node_graph::NodeGraphEvent),
    UpdateParam {
        node: NodeId,
        key: String,
        value: ParamValue,
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

impl PhotoApp {
    /// Boot daemon : le daemon n'ouvre AUCUNE fenêtre automatiquement —
    /// la fenêtre principale doit être créée ici via `window::open`
    /// (cf. iced examples/multi_window).
    fn new() -> (Self, Task<Message>) {
        let (main_id, open) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(1280.0, 820.0),
            min_size: Some(iced::Size::new(960.0, 600.0)),
            ..iced::window::Settings::default()
        });
        let mut app = Self::default();
        app.main_window = Some(main_id);
        (app, open.map(|_| Message::MockAction))
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
        }
    }
}

impl PhotoApp {
    fn alloc_layer_id(&mut self) -> u64 {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        id
    }

    fn layer_index(&self, id: u64) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    fn selected_layer_mut(&mut self) -> Option<&mut Layer> {
        let sel = self.selected_layer?;
        self.layer_index(sel).map(|i| &mut self.layers[i])
    }

    /// Raccourci clavier → Message applicatif.
    /// SEULE correspondance action ↔ logique : ajouter une action ici la
    /// branche au clavier partout.
    fn message_for(action: ui::shortcuts::Action) -> Option<Message> {
        use ui::shortcuts::Action;
        let sel = None; // sélection courante indisponible hors update (abonnement statique)
        match action {
            Action::NewProject => Some(Message::NewProject),
            Action::Open => Some(Message::OpenProject),
            Action::Save | Action::SaveAs => Some(Message::MockAction),
            Action::Quit => Some(Message::Quit),
            Action::Undo => Some(Message::Undo),
            Action::Redo => Some(Message::Redo),
            Action::Preferences => Some(Message::OpenPreferences),
            Action::ToggleTools => Some(Message::ToggleToolsPanel),
            Action::ToggleLayersPanel => Some(Message::TogglePanel(PanelType::Layers)),
            Action::TogglePropertiesPanel => {
                Some(Message::TogglePanel(PanelType::Properties))
            }
            Action::LayerNew => Some(Message::AddEmptyLayer),
            Action::LayerDuplicate => sel.map(Message::DuplicateLayer),
            Action::LayerDelete => {
                if sel.is_some() { // duplication : au moins un calque sélectionné
                    sel.map(Message::DeleteLayer)
                } else {
                    None
                }
            }
            Action::LayerMoveUp => sel.map(Message::MoveLayerUp),
            Action::LayerMoveDown => sel.map(Message::MoveLayerDown),
            Action::Rotate90 => sel.map(|id| Message::RotateLayer { id, delta: 90.0 }),
            Action::Rotate180 => sel.map(|id| Message::RotateLayer { id, delta: 180.0 }),
            Action::RotateN90 => sel.map(|id| Message::RotateLayer { id, delta: -90.0 }),
            Action::RotateN180 => sel.map(|id| Message::RotateLayer { id, delta: -180.0 }),
            Action::ResetTransform => sel.map(Message::ResetLayerTransform),
            Action::CropToSelection => Some(Message::CropLayerToSelection),
            Action::ZoomIn => Some(Message::ZoomInPressed),
            Action::ZoomOut => Some(Message::ZoomOutPressed),
            Action::FitToScreen => Some(Message::CanvasFit),
        }
    }

    /// Un calque visible a-t-il un mode de fusion non-Normal ?
    /// (l'opacité est gérée au draw, elle ne force plus le fallback)
    fn needs_fallback(&self) -> bool {
        self.layers
            .iter()
            .any(|l| l.visible && l.blend_mode != "Normal")
    }

    /// Composite CPU — UNIQUEMENT pour les modes de fusion non-Normal
    /// (le chemin rapide par calque couvre Normal/opacité sans recomposite).
    fn refresh_fallback(&mut self) {
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
    fn prepare_drag_background(&mut self, exclude_id: u64) {
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

fn update(app: &mut PhotoApp, message: Message) -> Task<Message> {
    match message {
        Message::NewProject => {
            app.layers.clear();
            app.selected_layer = None;
            app.next_layer_id = 0;
            app.doc_size = None;
            app.gen_graph = components::node_registry::create_empty_graph();
            app.canvas_pan = Vector::new(0.0, 0.0);
            app.zoom_level = 100;
            app.fallback_size = None;
            app.fallback_handle = None;
            app.image_path = None;
            app.image_error = None;
            app.move_anchor = None;
        }
        Message::OpenProject => {
            if app.background_tasks.is_empty() {
                return pick_image_task(Message::ImagePicked);
            }
        }
        Message::OpenImage => {
            if app.background_tasks.is_empty() {
                return pick_image_task(Message::ImagePicked);
            }
        }
        Message::ImagePicked(path_opt) => {
            if let Some(path) = path_opt
                && app.background_tasks.is_empty()
            {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("image")
                    .to_string();
                app.background_tasks.push(format!("Lecture de {name}"));
                return Task::perform(
                    async move {
                        let bytes =
                            std::fs::read(&path).map_err(|e| format!("Lecture échouée: {e}"))?;
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("image")
                            .to_string();
                        Ok::<(Vec<u8>, String), String>((bytes, name))
                    },
                    Message::ImageRead,
                );
            }
        }
        Message::ImageRead(Ok((bytes, name))) => {
            app.image_path = Some(name.clone());
            app.image_error = None;
            app.background_tasks.clear();
            app.background_tasks.push(format!("Décodage de {name}"));
            // Le décodage + la construction de la texture tournent hors UI
            // (Task::perform) — le spinner continue d'animer pendant ce temps
            let id = app.alloc_layer_id();
            return Task::perform(
                async move {
                    match ::image::load_from_memory(&bytes) {
                        Ok(dyn_img) => Ok(DecodedLayer(Layer::new(id, name, Arc::new(dyn_img)))),
                        Err(e) => Err(format!("Décodage échoué: {e}")),
                    }
                },
                Message::ImageDecoded,
            );
        }
        Message::ImageRead(Err(e)) => {
            app.background_tasks.clear();
            app.image_error = Some(e);
        }
        Message::ImageDecoded(Ok(decoded)) => {
            app.background_tasks.clear();
            let layer = decoded.0;
            // Le document prend les dimensions de la première image
            if app.doc_size.is_none() {
                let (w, h) = layer.dimensions();
                app.doc_size = Some((w, h));
                app.canvas_pan = Vector::new(0.0, 0.0);
                app.canvas_selection = None;
                app.zoom_level = 100;
            }
            app.layers.push(layer);
            app.selected_layer = app.layers.last().map(|l| l.id);
            app.refresh_fallback();
        }
        Message::ImageDecoded(Err(e)) => {
            app.background_tasks.clear();
            app.image_error = Some(e);
        }
        Message::ToggleTaskMenu => {
            app.task_menu_open = !app.task_menu_open;
        }

        // ---- Raccourcis clavier ----
        Message::OpenPreferences => {
            app.show_prefs = true;
            app.prefs_section =
                components::preferences::PrefsSection::General;
            app.capturing = None;
            // Détection GPU async pour la section Général
            return Task::perform(
                async { components::gpu::detect_gpu_info().await },
                Message::GpuDetected,
            );
        }
        Message::ClosePreferences => {
            app.show_prefs = false;
            app.capturing = None;
        }
        Message::PrefsSection(section) => {
            app.prefs_section = section;
        }
        Message::ShortcutCapture(action) => {
            app.capturing = Some(action);
        }
        Message::ShortcutCaptured(binding) => {
            if let Some(action) = app.capturing {
                if let Some(b) = binding {
                    app.shortcuts.set(action, b);
                    app.shortcuts.save();
                }
                app.capturing = None;
            }
        }
        Message::ShortcutCancelCapture => {
            app.capturing = None;
        }
        Message::ShortcutReset(action) => {
            app.shortcuts.reset(action);
            app.shortcuts.save();
        }
        Message::ShortcutResetAll => {
            app.shortcuts.reset_all();
            app.shortcuts.save();
        }
        Message::ShortcutAction(action) => {
            // Résolution action → Message (déléguée, une seule place)
            if let Some(msg) = PhotoApp::message_for(action) {
                // Re-dispatch récursif : réutilise tous les handlers existants
                return update(app, msg);
            }
        }
        Message::TickFrame => {
            // Animation du spinner (~30 fps)
            app.spinner_angle = (app.spinner_angle + 24.0) % 360.0;
        }

        // ---- Calques ----
        Message::SelectLayer(id) => {
            app.selected_layer = Some(id);
            app.move_anchor = None;
        }
        Message::ToggleLayerVisible(id) => {
            if let Some(i) = app.layer_index(id) {
                app.layers[i].visible = !app.layers[i].visible;
                app.refresh_fallback();
            }
        }
        Message::SetLayerOpacity { id, opacity } => {
            // Simple changement d'état : l'opacité est appliquée au draw
            // (GPU) — zéro régénération de pixels, zéro clignotement
            if let Some(i) = app.layer_index(id) {
                app.layers[i].opacity = opacity;
            }
        }
        Message::SetLayerBlend { id, mode } => {
            if let Some(i) = app.layer_index(id) {
                app.layers[i].blend_mode = mode;
                // Bascule chemin rapide ↔ fallback selon le mode
                if app.needs_fallback() {
                    app.refresh_fallback();
                } else {
                    app.fallback_handle = None;
                    app.fallback_size = None;
                }
            }
        }
        Message::RenameLayer { id, name } => {
            if let Some(i) = app.layer_index(id) {
                app.layers[i].name = name;
            }
        }
        Message::SetLayerOffset { id, axis, value } => {
            if let Some(i) = app.layer_index(id) {
                match axis {
                    OffsetAxis::X => app.layers[i].offset_x = value,
                    OffsetAxis::Y => app.layers[i].offset_y = value,
                }
                app.refresh_fallback();
            }
        }
        Message::SetLayerRotation { id, degrees } => {
            // Rotation au draw (GPU) — zéro travail sur les pixels
            if let Some(i) = app.layer_index(id) {
                app.layers[i].rotation = degrees.clamp(-360.0, 360.0);
            }
        }
        Message::RotateLayer90 { id, clockwise } => {
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(tid) = target
                && let Some(i) = app.layer_index(tid)
            {
                let delta = if clockwise { 90.0 } else { -90.0 };
                // Normalise dans [-180, 180[ pour garder des valeurs lisibles
                let r = (app.layers[i].rotation + delta + 180.0).rem_euclid(360.0) - 180.0;
                app.layers[i].rotation = r;
            }
        }
        Message::RotateLayer { id, delta } => {
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(tid) = target
                && let Some(i) = app.layer_index(tid)
            {
                let r = (app.layers[i].rotation + delta + 180.0).rem_euclid(360.0) - 180.0;
                // -180 et 180 sont équivalents, on garde 180 pour lisibilité
                app.layers[i].rotation = if r == -180.0 { 180.0 } else { r };
            }
        }
        Message::SetLayerScale { id, scale } => {
            if let Some(i) = app.layer_index(id) {
                app.layers[i].scale = scale.clamp(0.05, 8.0);
            }
        }
        Message::ResetLayerTransform(id) => {
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(tid) = target
                && let Some(i) = app.layer_index(tid)
            {
                app.layers[i].rotation = 0.0;
                app.layers[i].scale = 1.0;
            }
        }
        Message::CropLayerToSelection => {
            // Garde-fous explicites (démarche incrémentale) : un calque
            // sélectionné, une sélection rectangulaire, transform neutre
            let Some(id) = app.selected_layer else {
                app.image_error = Some("Rogner : aucun calque sélectionné".into());
                return Task::none();
            };
            let Some(sel) = app.canvas_selection else {
                app.image_error =
                    Some("Rogner : faites d'abord une sélection rectangulaire (outil Sélect)".into());
                return Task::none();
            };
            let Some(i) = app.layer_index(id) else {
                return Task::none();
            };
            if app.layers[i].rotation.abs() > 0.01 || (app.layers[i].scale - 1.0).abs() > 0.01 {
                app.image_error =
                    Some("Rogner : réinitialisez d'abord rotation/échelle du calque".into());
                return Task::none();
            }
            // Écran → monde → coordonnées calque
            let zoom = (app.zoom_level as f32 / 100.0).max(0.001);
            let (doc_w, doc_h) = app.doc_size.unwrap_or((800, 600));
            let to_layer = |sx: f32, sy: f32| {
                let wx = (sx - app.canvas_viewport.width / 2.0 - app.canvas_pan.x) / zoom
                    + doc_w as f32 / 2.0;
                let wy = (sy - app.canvas_viewport.height / 2.0 - app.canvas_pan.y) / zoom
                    + doc_h as f32 / 2.0;
                (wx - app.layers[i].offset_x, wy - app.layers[i].offset_y)
            };
            let (x0, y0) = to_layer(sel.x, sel.y);
            let (x1, y1) = to_layer(sel.x + sel.width, sel.y + sel.height);
            let cx0 = x0.min(x1).round() as i32;
            let cy0 = y0.min(y1).round() as i32;
            let cw = ((x1 - x0).abs().round() as u32).max(1);
            let ch = ((y1 - y0).abs().round() as u32).max(1);
            match app.layers[i].crop(cx0, cy0, cw, ch) {
                Ok(()) => {
                    app.image_error = None;
                    app.refresh_fallback();
                }
                Err(e) => app.image_error = Some(e),
            }
        }
        Message::AddEmptyLayer => {
            let (w, h) = app.doc_size.unwrap_or((800, 600));
            let transparent = ::image::DynamicImage::ImageRgba8(::image::ImageBuffer::from_pixel(
                w.max(1),
                h.max(1),
                ::image::Rgba([0, 0, 0, 0]),
            ));
            let id = app.alloc_layer_id();
            let idx = app.selected_layer.and_then(|s| app.layer_index(s));
            let layer = Layer::new(id, format!("Calque {}", id + 1), Arc::new(transparent));
            // Insère AU-DESSUS du calque sélectionné (sinon tout en haut)
            match idx {
                Some(i) => app.layers.insert(i + 1, layer),
                None => app.layers.push(layer),
            }
            app.selected_layer = Some(id);
            app.refresh_fallback();
        }
        Message::DuplicateLayer(id) => {
            let src = id;
            let src = if app.layer_index(src).is_some() {
                src
            } else {
                app.selected_layer.unwrap_or(src)
            };
            if let Some(i) = app.layer_index(src) {
                let mut copy = Layer::new(
                    app.alloc_layer_id(),
                    format!("{} copie", app.layers[i].name),
                    app.layers[i].image.clone(),
                );
                copy.opacity = app.layers[i].opacity;
                copy.blend_mode = app.layers[i].blend_mode.clone();
                copy.visible = app.layers[i].visible;
                copy.offset_x = app.layers[i].offset_x;
                copy.offset_y = app.layers[i].offset_y;
                copy.rotation = app.layers[i].rotation;
                copy.scale = app.layers[i].scale;
                app.layers.insert(i + 1, copy);
                app.selected_layer = Some(app.layers[i + 1].id);
                app.refresh_fallback();
            }
        }
        Message::DeleteLayer(id) => {
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(t) = target
                && app.layers.len() > 1
                && let Some(i) = app.layer_index(t)
            {
                app.layers.remove(i);
                app.selected_layer = app
                    .layers
                    .get(i.min(app.layers.len() - 1))
                    .or_else(|| app.layers.last())
                    .map(|l| l.id);
                app.refresh_fallback();
            }
        }
        Message::MoveLayerUp(id) => {
            if let Some(i) = app.layer_index(id)
                && i + 1 < app.layers.len()
            {
                app.layers.swap(i, i + 1);
                app.refresh_fallback();
            }
        }
        Message::MoveLayerDown(id) => {
            if let Some(i) = app.layer_index(id)
                && i > 0
            {
                app.layers.swap(i, i - 1);
                app.refresh_fallback();
            }
        }

        Message::SelectTool(tool) => {
            app.selected_tool = tool;
            app.canvas_selection = None;
            app.move_anchor = None;
        }
        Message::ToggleToolsPanel => {
            app.tools_visible = !app.tools_visible;
        }
        Message::CanvasFit => {
            // Zoom pour voir toute l'image, centrée (pan nul)
            if let Some((iw, ih)) = app.doc_size.map(|(w, h)| (w as f32, h as f32)) {
                let vw = app.canvas_viewport.width.max(1.0);
                let vh = app.canvas_viewport.height.max(1.0);
                let fit = (vw / iw).min(vh / ih) * 0.95; // 5% de marge
                let zoom = fit.clamp(0.08, 6.0);
                app.zoom_level = (zoom * 100.0).round() as u32;
                app.canvas_pan = Vector::new(0.0, 0.0);
                app.canvas_selection = None;
            }
        }
        Message::ImageCanvasEvent(evt) => match evt {
            ui::image_canvas::ImageCanvasEvent::Viewport(size) => {
                app.canvas_viewport = size;
            }
            ui::image_canvas::ImageCanvasEvent::Pan(pan) => {
                if app.selected_tool == Tool::Hand {
                    app.canvas_pan = pan;
                }
            }
            ui::image_canvas::ImageCanvasEvent::ZoomPan { zoom, pan } => {
                app.zoom_level = (zoom * 100.0) as u32;
                app.canvas_pan = pan;
            }
            ui::image_canvas::ImageCanvasEvent::ZoomAt { zoom, pan } => {
                app.zoom_level = (zoom * 100.0) as u32;
                app.canvas_pan = pan;
            }
            ui::image_canvas::ImageCanvasEvent::SelectRect(rect) => {
                if app.selected_tool == Tool::Select || app.selected_tool == Tool::Zoom {
                    if app.selected_tool == Tool::Zoom {
                        // Zoom sur la zone sélectionnée
                        if let Some(r) = rect
                            && r.width > 10.0
                            && r.height > 10.0
                        {
                            let sx = 800.0 / r.width;
                            let sy = 600.0 / r.height;
                            let new_zoom =
                                (sx.min(sy) * app.zoom_level as f32 / 100.0).clamp(0.08, 6.0);
                            app.zoom_level = (new_zoom * 100.0) as u32;
                            let cx = r.x + r.width / 2.0 - 400.0;
                            let cy = r.y + r.height / 2.0 - 300.0;
                            app.canvas_pan = Vector::new(-cx, -cy);
                        }
                    } else {
                        app.canvas_selection = rect;
                    }
                }
            }
            ui::image_canvas::ImageCanvasEvent::MoveLayerStart => {
                if app.selected_tool == Tool::Move {
                    // Lit l'ancre avant toute mutation (règle own-borrow-over-clone)
                    let anchor = app
                        .selected_layer_mut()
                        .map(|l| (l.id, l.offset_x, l.offset_y));
                    if let Some((id, ox, oy)) = anchor {
                        app.move_anchor = Some((id, ox, oy));
                        // Fallback (fusion non-Normal) : pré-calcule UNE FOIS le
                        // fond sans le calque déplacé — coût unique au début du
                        // drag, ensuite zéro recomposite pendant tout le geste
                        if app.needs_fallback() {
                            app.prepare_drag_background(id);
                        }
                    }
                }
            }
            ui::image_canvas::ImageCanvasEvent::MoveLayer { dx, dy } => {
                if app.selected_tool == Tool::Move
                    && let Some((id, ax, ay)) = app.move_anchor
                    && Some(id) == app.selected_layer
                {
                    let zoom = app.zoom_level as f32 / 100.0;
                    if zoom > 0.001 {
                        // ZÉRO recomposite dans les deux chemins :
                        // - rapide : le canvas redessine la texture à sa
                        //   nouvelle position (modèle Affinity)
                        // - fallback : fond pré-calculé + calque dessiné
                        //   par-dessus (approximation Normal pendant le geste)
                        let new_x = ax + dx / zoom;
                        let new_y = ay + dy / zoom;
                        if let Some(i) = app.layer_index(id) {
                            app.layers[i].offset_x = new_x;
                            app.layers[i].offset_y = new_y;
                        }
                    }
                }
            }
            ui::image_canvas::ImageCanvasEvent::MoveLayerEnd => {
                app.move_anchor = None;
                // Vrai recomposite : le blend réel du calque à sa position
                // finale remplace l'approximation du drag
                let was_fallback = app.needs_fallback();
                app.drag_background = None;
                app.drag_background_size = None;
                if was_fallback {
                    app.refresh_fallback();
                }
            }
        },
        Message::Quit => {
            std::process::exit(0);
        }
        Message::Undo => {}
        Message::Redo => {}
        Message::ZoomInPressed => {
            app.zoom_level = (app.zoom_level + 10).clamp(5, 1600);
        }
        Message::ZoomOutPressed => {
            app.zoom_level = app.zoom_level.saturating_sub(10).max(5);
        }
        Message::MockAction => {
            // Keep silent to avoid spam from subscription
        }

        Message::TogglePanel(panel_type) => {
            let existing_pane = app
                .panes
                .iter()
                .find(|(_, p)| **p == panel_type)
                .map(|(pane, _)| *pane);

            if let Some(pane) = existing_pane {
                app.panes.close(pane);
            } else {
                let target_canvas_pane = app
                    .panes
                    .iter()
                    .find(|(_, p)| **p == PanelType::Canvas)
                    .map(|(p, _)| *p);

                if let Some(canvas_pane) = target_canvas_pane {
                    let axis = match panel_type {
                        PanelType::Generator => pane_grid::Axis::Horizontal,
                        _ => pane_grid::Axis::Vertical,
                    };
                    app.panes.split(axis, canvas_pane, panel_type);
                }
            }
        }
        Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
            app.panes.resize(split, ratio);
        }
        Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
            app.panes.drop(pane, target);
        }
        Message::PaneDragged(_) => {}
        Message::PaneClicked(pane) => {
            app.focus = Some(pane);
        }
        Message::ClosePane(pane) => {
            app.panes.close(pane);
        }
        Message::CloseNodeContextMenu => {
            app.node_context_menu = None;
        }
        // ---- Générateur de textures (graphe nodal, usage futur filtres/génération) ----
        Message::NodeGraphEvent(evt) => match evt {
            ui::node_graph::NodeGraphEvent::NodeSelected(id) => {
                app.gen_selected_node = Some(id);
                app.node_context_menu = None;
            }
            ui::node_graph::NodeGraphEvent::NodeMoved { id, position } => {
                app.gen_graph.move_node(id, position);
            }
            ui::node_graph::NodeGraphEvent::BackgroundClicked => {
                app.gen_selected_node = None;
                app.node_context_menu = None;
            }
            ui::node_graph::NodeGraphEvent::RequestContextMenu(world, local) => {
                app.pending_connect = None;
                app.node_context_menu = Some(local);
                app.node_context_world = Some(world);
            }
            ui::node_graph::NodeGraphEvent::Connect {
                from,
                from_socket,
                to,
                to_socket,
            } => {
                let existing = app
                    .gen_graph
                    .connections
                    .iter()
                    .find(|c| c.to_node == to && c.to_socket == to_socket)
                    .cloned();
                if let Some(conn) = existing {
                    app.gen_graph.disconnect(&conn);
                }
                let from_ty = if from_socket == "factor" || to_socket == "factor" {
                    datatypes::SocketType::Float
                } else {
                    datatypes::SocketType::Image
                };
                let _ = app.gen_graph.connect(suite_core::Connection::new(
                    from,
                    from_socket.clone(),
                    to,
                    to_socket.clone(),
                    from_ty,
                ));
                app.node_context_menu = None;
            }
            ui::node_graph::NodeGraphEvent::Disconnect { node, socket } => {
                app.gen_graph.disconnect_input(node, &socket);
            }
            // Événements non utilisés par le générateur (ignorés silencieusement)
            _ => {}
        },
        Message::UpdateParam { node, key, value } => {
            app.gen_graph.update_param(node, key.clone(), value.clone());
        }
        Message::AddNodeAt { type_id, world_pos } => {
            let pos = datatypes::Vec2::new(
                world_pos.x.max(-2000.0).min(3000.0),
                world_pos.y.max(-2000.0).min(3000.0),
            );
            if let Some(id) =
                components::node_registry::create_node_for_type(&type_id, pos, &mut app.gen_graph)
            {
                app.gen_selected_node = Some(id);
            }
            app.node_context_menu = None;
            app.node_context_world = None;
        }
        Message::DeleteSelectedNode => {
            if let Some(id) = app.gen_selected_node {
                app.gen_graph.remove_node(id);
                app.gen_selected_node = None;
            }
        }

        Message::DetectGpu => {
            return Task::perform(
                async { components::gpu::detect_gpu_info().await },
                Message::GpuDetected,
            );
        }
        Message::GpuDetected(info) => {
            app.gpu_info = Some(info);
            app.gpu_available = true;
        }
    }
    Task::none()
}

fn pick_image_task(map: fn(Option<std::path::PathBuf>) -> Message) -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter(
                    "Images",
                    &["png", "jpg", "jpeg", "bmp", "tiff", "webp", "gif"],
                )
                .set_title("Ouvrir une image")
                .pick_file()
                .await
                .map(|h| h.path().to_path_buf())
        },
        map,
    )
}

fn view(app: &PhotoApp, _window: iced::window::Id) -> Element<'_, Message> {
    let doc_size = app.doc_size.map(|(w, h)| Size::new(w as f32, h as f32));
    // Contenu central : barre contextuelle (projet/zoom/export) + workspace
    let menus = app_menus(app.tools_visible, app.selected_layer);
    let menu_buttons = ui::menu::bar(&menus);

    // Bouton spinner façon Final Cut Pro : toujours visible, tourne pendant
    // un traitement en arrière-plan, clic → menu des tâches en cours
    let spinning = !app.background_tasks.is_empty();
    let spinner_btn = iced::widget::button(
        // Canvas 20 px centré dans un bouton 30 px sans padding → pas de crop
        iced::widget::container(ui::spinner::circle(
            if spinning { app.spinner_angle } else { 0.0 },
            20.0,
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(30.0))
    .padding(0)
    .style(|_, s| {
        let mut st = iced::widget::button::Style::default();
        st.background = Some(if s == iced::widget::button::Status::Hovered {
            ui::theme::colors::HOVER_OVERLAY.into()
        } else {
            iced::Color::TRANSPARENT.into()
        });
        st.border.radius = ui::theme::metrics::RADIUS_DROPDOWN.into();
        st
    })
    .on_press(Message::ToggleTaskMenu);

    let task_menu = {
        let items: Vec<iced::Element<'_, Message>> = if app.background_tasks.is_empty() {
            vec![iced::widget::container(iced::widget::text("Aucun traitement en cours")
                    .size(12)
                    .color(ui::theme::colors::TEXT_MUTED))
                .padding(iced::Padding::new(8.0).left(10.0).right(10.0))
                .into()]
        } else {
            app.background_tasks
                .iter()
                .map(|label| {
                    iced::widget::row![
                        iced::widget::text(label)
                            .size(12)
                            .color(ui::theme::colors::TEXT_PRIMARY),
                        iced::widget::Space::new().width(Length::Fill),
                        ui::spinner::circle(app.spinner_angle, 12.0),
                    ]
                    .align_y(Alignment::Center)
                    .into()
                })
                .collect()
        };
        iced::widget::container(iced::widget::column(items).spacing(2).padding(4))
            .width(Length::Fixed(240.0))
            .style(|_| iced::widget::container::Style {
                background: Some(ui::theme::colors::BG_DROPDOWN.into()),
                border: iced::Border {
                    width: 1.0,
                    color: ui::theme::colors::BORDER_SUBTLE,
                    radius: ui::theme::metrics::RADIUS_DROPDOWN.into(),
                    ..Default::default()
                },
                shadow: ui::theme::shadows::dropdown(),
                ..Default::default()
            })
    };

    let spinner = Some(
        iced_aw::DropDown::new(spinner_btn, task_menu, app.task_menu_open)
            .width(Length::Fixed(240.0))
            .alignment(iced_aw::drop_down::Alignment::BottomEnd)
            .on_dismiss(Message::ToggleTaskMenu)
            .into(),
    );

    let central = iced::widget::column![
        components::toolbar::context_bar(app.image_path.as_deref()),
        components::workspace::render(
            &app.panes,
            app.focus,
            &app.layers,
            app.selected_layer,
            doc_size,
            app.fallback_handle.clone(),
            app.fallback_size,
            app.move_anchor.map(|(id, _, _)| id),
            app.drag_background.clone(),
            app.drag_background_size,
            app.image_path.clone(),
            app.image_error.clone(),
            app.selected_tool,
            app.tools_visible,
            app.canvas_pan,
            app.zoom_level,
            app.canvas_selection,
            app.color_profile.clone(),
            app.canvas_viewport,
            &app.gen_graph,
            app.gen_selected_node,
            &app.gen_previews,
            app.node_context_menu,
            app.node_context_world,
        )
    ];
    // Shell : menus intégrés à la top bar — outils Photo en flottant sur le canvas
    let base_layout = ui::shell::minimalist_layout_menus_only(
        "Creative Suite Open Photo",
        menu_buttons,
        central,
        spinner,
    );


    // Modal Préférences unifiée (Général / Raccourcis clavier / À propos)
    if app.show_prefs {
        let prefs_overlay = iced::widget::stack![
            // Scrim : clic hors modal ferme
            iced::widget::mouse_area(
                iced::widget::container(
                    iced::widget::Space::new().width(Length::Fill).height(Length::Fill)
                )
                .style(|_| iced::widget::container::Style {
                    background: Some(ui::theme::colors::CABLE_SHADOW.into()),
                    ..Default::default()
                })
                .width(Length::Fill)
                .height(Length::Fill)
            )
            .on_press(Message::ClosePreferences),
            components::preferences::view(
                &app.shortcuts,
                app.capturing,
                app.prefs_section,
                app.gpu_info.clone(),
                app.gpu_available,
            ),
        ];
        return iced::widget::stack![base_layout, prefs_overlay].into();
    }

    // Les dropdowns des menus sont gérés nativement par iced_aw::DropDown
    base_layout
}
