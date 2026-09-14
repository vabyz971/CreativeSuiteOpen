// Cygnus — Suite créative professionnelle open source
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

//! Base minimale de l'app Audio (prompt v2) : boot eframe + thème.
//!
//! `audio-engine` étant en fondation, pas d'interface complexe pour
//! le moment : un placeholder central. Le transport, les pistes, le
//! piano roll et le mixer seront construits ici quand le moteur
//! arrivera.

mod app;

fn main() {
    eframe::run_native(
        "Cygnus Audio",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            ui_kit::theme::setup_fonts(&cc.egui_ctx);
            ui_kit::theme::apply_cygnus_theme(&cc.egui_ctx);
            Ok(Box::new(app::AudioApp::new()))
        }),
    )
    .expect("Failed to start eframe");
}
