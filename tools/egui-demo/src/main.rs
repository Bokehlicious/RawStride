//! Minimal eframe/egui spike for evaluating a native GUI shell for PhotoWorkflow.
//!
//! Standalone crate with its own `[workspace]`; it is not a member of the root
//! workspace and is therefore not built by `cargo test` at the repository root.

use eframe::egui;
use eframe::{NativeOptions, Renderer};

const APP_TITLE: &str = "PhotoWorkflow";

#[derive(Debug)]
struct DemoApp {
    label: String,
}

impl Default for DemoApp {
    fn default() -> Self {
        Self {
            label: "Click Testa to see magic!".to_owned(),
        }
    }
}

impl eframe::App for DemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(APP_TITLE);

            if ui.button("Testa").clicked() {
                "It works!".clone_into(&mut self.label);
            }

            ui.label(self.label.as_str());
        });
    }
}

fn main() -> eframe::Result {
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title(APP_TITLE),
        renderer: Renderer::Glow,
        ..NativeOptions::default()
    };

    eframe::run_native(
        APP_TITLE,
        native_options,
        Box::new(|_cc| Ok(Box::new(DemoApp::default()))),
    )
}
