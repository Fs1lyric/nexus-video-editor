mod ui;
mod editor;
mod timeline;
mod effects;
mod export;
mod project;

use eframe::egui;
use log::info;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    info!("Starting Nexus Video Editor");

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Nexus Video Editor",
        options,
        Box::new(|_cc| Ok(Box::<NexusApp>::default())),
    )
}

pub struct NexusApp {
    project: project::Project,
    timeline: timeline::Timeline,
    selected_clip: Option<usize>,
}

impl Default for NexusApp {
    fn default() -> Self {
        Self {
            project: project::Project::new(),
            timeline: timeline::Timeline::new(),
            selected_clip: None,
        }
    }
}

impl eframe::App for NexusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project").clicked() {
                        self.project = project::Project::new();
                    }
                    if ui.button("Open Project").clicked() {
                        // TODO: Implement file dialog
                    }
                    if ui.button("Save Project").clicked() {
                        // TODO: Implement save
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        std::process::exit(0);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        // TODO: Implement undo
                    }
                    if ui.button("Redo").clicked() {
                        // TODO: Implement redo
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Full Screen").clicked() {
                        // TODO: Implement fullscreen
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        // TODO: Show about dialog
                    }
                });
            });
        });

        egui::SidePanel::left("asset_panel")
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.label("📁 Assets");
                ui.separator();
                if ui.button("+ Import Video").clicked() {
                    // TODO: Implement video import
                }
                if ui.button("+ Import Audio").clicked() {
                    // TODO: Implement audio import
                }
                if ui.button("+ Import Image").clicked() {
                    // TODO: Implement image import
                }
            });

        egui::SidePanel::right("effects_panel")
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.label("✨ Effects");
                ui.separator();
                ui.label("Video Effects:");
                for effect in &effects::AVAILABLE_EFFECTS {
                    if ui.button(effect).clicked() {
                        info!("Selected effect: {}", effect);
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Timeline");
            ui.separator();

            // Timeline preview
            ui.horizontal(|ui| {
                if ui.button("▶").clicked() {
                    // TODO: Play timeline
                }
                if ui.button("⏸").clicked() {
                    // TODO: Pause timeline
                }
                if ui.button("⏹").clicked() {
                    // TODO: Stop timeline
                }
                ui.label(format!("Duration: {:.2}s", self.timeline.duration));
            });

            ui.separator();
            ui.label("Clips on timeline:");
            for (i, clip) in self.timeline.clips.iter().enumerate() {
                let selected = self.selected_clip == Some(i);
                if ui.selectable_label(selected, &clip.name) {
                    self.selected_clip = Some(i);
                }
            }

            if let Some(idx) = self.selected_clip {
                ui.separator();
                ui.label("📋 Clip Properties:");
                if idx < self.timeline.clips.len() {
                    let clip = &self.timeline.clips[idx];
                    ui.label(format!("Name: {}", clip.name));
                    ui.label(format!("Duration: {:.2}s", clip.duration));
                    ui.label(format!("Start: {:.2}s", clip.start_time));
                }
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Export Video").clicked() {
                    // TODO: Show export dialog
                }
            });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Ready");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Nexus v0.1.0");
                });
            });
        });
    }
}
