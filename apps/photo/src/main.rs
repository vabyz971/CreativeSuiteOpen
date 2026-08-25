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

//! Point d'entrée de l'app Photo : câblage iced (daemon multi-fenêtres).
//!
//! Découpage :
//! - `message`  : enum Message + types partagés (outils, panneaux)
//! - `state`    : PhotoApp (état) + helpers document/canvas
//! - `update`   : boucle de mise à jour (un handler par message)
//! - `view`     : rendu + abonnements
//! - `menus`    : menus applicatifs du shell
//! - `ui_handles`: frontière moteur pur → handles iced (cache)

mod menus;
mod message;
mod state;
mod ui_handles;
mod update;
mod view;

pub mod components;
pub mod layers;

pub use message::{DecodedLayer, Message, OffsetAxis, PanelType, PendingPaint, Tool};
pub use state::PhotoApp;

use update::update;
use view::{subscription, view};

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
        .title(
            |app: &PhotoApp, _window: iced::window::Id| match &app.project_path {
                Some(path) => {
                    let name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("projet");
                    format!("Creative Suite Open Photo — {name}")
                }
                None => "Creative Suite Open Photo".to_string(),
            },
        )
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
        .default_font(ui_kit::theme::fonts::SANS)
        .run()
}
