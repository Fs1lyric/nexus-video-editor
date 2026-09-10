//! egui front-end. A thin editor over the `nexus` library: import clips, set
//! in/out points, reorder, preview the playhead frame, export.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;

use nexus::media::{ffmpeg_bin, probe, MediaInfo};
use nexus::project::Project;

pub fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("Nexus"),
        ..Default::default()
    };
    eframe::run_native(
        "Nexus",
        options,
        Box::new(|_cc| Ok(Box::new(NexusApp::new()) as Box<dyn eframe::App>)),
    )
    .map_err(|e| anyhow::anyhow!("gui error: {e}"))
}

type Frame = (u32, u32, Vec<u8>);
type FrameKey = (u64, i64);
type PendingFrame = (Receiver<anyhow::Result<Frame>>, FrameKey);

enum ExportMsg {
    Progress(f32, String),
    Done(PathBuf),
    Failed(String),
}

struct NexusApp {
    project: Project,
    project_path: Option<PathBuf>,
    info: HashMap<PathBuf, MediaInfo>,
    selected: Option<usize>,

    playhead: f64,
    playing: bool,
    last_seek: f64,

    preview: Option<egui::TextureHandle>,
    preview_key: Option<FrameKey>,
    pending: Option<PendingFrame>,

    export: Option<Receiver<ExportMsg>>,
    export_progress: f32,
    status: String,
    ffmpeg_ok: bool,
}

impl NexusApp {
    fn new() -> Self {
        let ffmpeg_ok = Command::new(ffmpeg_bin())
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        Self {
            project: Project::default(),
            project_path: None,
            info: HashMap::new(),
            selected: None,
            playhead: 0.0,
            playing: false,
            last_seek: 0.0,
            preview: None,
            preview_key: None,
            pending: None,
            export: None,
            export_progress: 0.0,
            status: if ffmpeg_ok {
                "Ready. File → Import clips to begin.".into()
            } else {
                "ffmpeg was not found on PATH — export and preview will not work.".into()
            },
            ffmpeg_ok,
        }
    }

    fn busy(&self) -> bool {
        self.export.is_some()
    }

    fn source_len(&self, p: &Path) -> f64 {
        self.info.get(p).map(|m| m.duration).unwrap_or(0.0)
    }

    fn import(&mut self, paths: Vec<PathBuf>) {
        for p in paths {
            match probe(&p) {
                Ok(m) => {
                    let end = m.duration;
                    self.info.insert(p.clone(), m);
                    self.project.push_clip(p, 0.0, end);
                    self.selected = Some(self.project.timeline.clips.len() - 1);
                }
                Err(e) => self.status = format!("Could not import {}: {e}", p.display()),
            }
        }
    }

    fn new_project(&mut self) {
        self.project = Project::default();
        self.project_path = None;
        self.info.clear();
        self.selected = None;
        self.playhead = 0.0;
        self.preview = None;
        self.preview_key = None;
        self.status = "New project.".into();
    }

    fn open_project(&mut self, path: PathBuf) {
        match Project::load(&path) {
            Ok(p) => {
                self.info.clear();
                for c in &p.timeline.clips {
                    if let Ok(m) = probe(&c.source) {
                        self.info.insert(c.source.clone(), m);
                    }
                }
                self.project = p;
                self.project_path = Some(path);
                self.selected = (!self.project.timeline.clips.is_empty()).then_some(0);
                self.playhead = 0.0;
                self.preview_key = None;
                self.status = "Project opened.".into();
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    fn save_project(&mut self, path: PathBuf) {
        match self.project.save(&path) {
            Ok(()) => {
                self.project_path = Some(path);
                self.status = "Saved.".into();
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    fn start_export(&mut self, out: PathBuf) {
        let project = self.project.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut cb = |f: f32, m: &str| {
                let _ = tx.send(ExportMsg::Progress(f, m.to_string()));
            };
            match nexus::export_project(&project, &out, &mut cb) {
                Ok(()) => {
                    let _ = tx.send(ExportMsg::Done(out));
                }
                Err(e) => {
                    let _ = tx.send(ExportMsg::Failed(format!("{e}")));
                }
            }
        });
        self.export = Some(rx);
        self.export_progress = 0.0;
        self.status = "Exporting…".into();
    }

    fn poll_export(&mut self) {
        let Some(rx) = &self.export else { return };
        let mut finished = None;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ExportMsg::Progress(f, m) => {
                    self.export_progress = f;
                    self.status = m;
                }
                ExportMsg::Done(p) => finished = Some(format!("Exported → {}", p.display())),
                ExportMsg::Failed(e) => finished = Some(format!("Export failed: {e}")),
            }
        }
        if let Some(s) = finished {
            self.status = s;
            self.export = None;
            self.export_progress = 0.0;
        }
    }

    /// Kick off a preview-frame render for the current playhead if needed.
    fn refresh_preview(&mut self, ctx: &egui::Context) {
        if !self.ffmpeg_ok || self.busy() {
            return;
        }
        // Resolve pending render.
        if let Some((rx, key)) = &self.pending {
            if let Ok(result) = rx.try_recv() {
                if let Ok((w, h, rgba)) = result {
                    let img =
                        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                    self.preview =
                        Some(ctx.load_texture("preview", img, egui::TextureOptions::LINEAR));
                    self.preview_key = Some(*key);
                }
                self.pending = None;
            } else {
                return; // still rendering
            }
        }

        let Some((idx, into_src)) = self.project.locate(self.playhead) else {
            return;
        };
        let clip = &self.project.timeline.clips[idx];
        let key = (clip.id, (into_src * 1000.0) as i64);
        if Some(key) == self.preview_key {
            return;
        }
        // debounce scrubbing
        if ctx.input(|i| i.time) - self.last_seek < 0.10 {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
            return;
        }

        let src = clip.source.clone();
        let t = into_src;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(render_frame(&src, t));
        });
        self.pending = Some((rx, key));
        ctx.request_repaint_after(std::time::Duration::from_millis(80));
    }

    fn move_selected(&mut self, delta: i64) {
        let Some(i) = self.selected else { return };
        let n = self.project.timeline.clips.len() as i64;
        let j = i as i64 + delta;
        if j < 0 || j >= n {
            return;
        }
        self.project.timeline.clips.swap(i, j as usize);
        self.selected = Some(j as usize);
    }

    fn delete_selected(&mut self) {
        let Some(i) = self.selected else { return };
        self.project.timeline.clips.remove(i);
        self.selected = if self.project.timeline.clips.is_empty() {
            None
        } else {
            Some(i.min(self.project.timeline.clips.len() - 1))
        };
        self.preview_key = None;
    }
}

impl eframe::App for NexusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_export();

        let dur = self.project.duration();
        if self.playing {
            if dur <= 0.0 {
                self.playing = false;
            } else {
                self.playhead += ctx.input(|i| i.stable_dt) as f64;
                if self.playhead >= dur {
                    self.playhead = dur;
                    self.playing = false;
                }
                self.last_seek = 0.0; // don't debounce during playback
                ctx.request_repaint();
            }
        }

        menu_bar(self, ctx);

        if !self.ffmpeg_ok {
            egui::TopBottomPanel::top("warn").show(ctx, |ui| {
                ui.colored_label(egui::Color32::from_rgb(220, 90, 80), &self.status);
            });
        }

        inspector(self, ctx);
        timeline(self, ctx, dur);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);
            match &self.preview {
                Some(tex) => {
                    let avail = ui.available_size() - egui::vec2(0.0, 8.0);
                    ui.centered_and_justified(|ui| {
                        ui.add(
                            egui::Image::new(tex)
                                .max_size(avail)
                                .maintain_aspect_ratio(true),
                        );
                    });
                }
                None => {
                    ui.centered_and_justified(|ui| {
                        ui.weak(if self.project.timeline.clips.is_empty() {
                            "No clips. File → Import clips…"
                        } else {
                            "Scrub the timeline to preview a frame."
                        });
                    });
                }
            }
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.busy() {
                    ui.add(egui::ProgressBar::new(self.export_progress).desired_width(160.0));
                }
                ui.label(&self.status);
            });
        });

        self.refresh_preview(ctx);
    }
}

fn menu_bar(app: &mut NexusApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("menu").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New project").clicked() {
                    app.new_project();
                    ui.close_menu();
                }
                if ui.button("Open project…").clicked() {
                    ui.close_menu();
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Nexus project", &["json"])
                        .pick_file()
                    {
                        app.open_project(p);
                    }
                }
                if ui.button("Save").clicked() {
                    ui.close_menu();
                    match app.project_path.clone() {
                        Some(p) => app.save_project(p),
                        None => {
                            if let Some(p) = rfd::FileDialog::new()
                                .set_file_name("project.json")
                                .add_filter("Nexus project", &["json"])
                                .save_file()
                            {
                                app.save_project(p);
                            }
                        }
                    }
                }
                if ui.button("Save as…").clicked() {
                    ui.close_menu();
                    if let Some(p) = rfd::FileDialog::new()
                        .set_file_name("project.json")
                        .add_filter("Nexus project", &["json"])
                        .save_file()
                    {
                        app.save_project(p);
                    }
                }
                ui.separator();
                if ui.button("Import clips…").clicked() {
                    ui.close_menu();
                    if let Some(paths) = rfd::FileDialog::new()
                        .add_filter("Video", &["mp4", "mov", "mkv", "webm", "avi", "m4v"])
                        .pick_files()
                    {
                        app.import(paths);
                    }
                }
                let can_export =
                    !app.project.timeline.clips.is_empty() && !app.busy() && app.ffmpeg_ok;
                if ui
                    .add_enabled(can_export, egui::Button::new("Export…"))
                    .clicked()
                {
                    ui.close_menu();
                    if let Some(p) = rfd::FileDialog::new()
                        .set_file_name("nexus-export.mp4")
                        .add_filter("MP4", &["mp4"])
                        .save_file()
                    {
                        app.start_export(p);
                    }
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.separator();
            ui.add_enabled_ui(!app.project.timeline.clips.is_empty(), |ui| {
                if ui
                    .button(if app.playing { "⏸ Pause" } else { "▶ Play" })
                    .clicked()
                {
                    app.playing = !app.playing;
                }
                if ui.button("⏮").clicked() {
                    app.playhead = 0.0;
                    app.playing = false;
                }
            });
        });
    });
}

fn inspector(app: &mut NexusApp, ctx: &egui::Context) {
    egui::SidePanel::right("inspector")
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("Project");
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut app.project.name);
            });
            ui.horizontal(|ui| {
                ui.label("Size");
                ui.add(
                    egui::DragValue::new(&mut app.project.width)
                        .range(16..=7680)
                        .suffix(" w"),
                );
                ui.add(
                    egui::DragValue::new(&mut app.project.height)
                        .range(16..=4320)
                        .suffix(" h"),
                );
            });
            ui.horizontal(|ui| {
                ui.label("FPS");
                ui.add(
                    egui::DragValue::new(&mut app.project.fps)
                        .range(1.0..=240.0)
                        .speed(0.1),
                );
            });

            ui.separator();
            ui.heading("Clip");

            let Some(i) = app.selected else {
                ui.weak("Select a clip on the timeline.");
                return;
            };
            if i >= app.project.timeline.clips.len() {
                app.selected = None;
                return;
            }

            let src_len = app.source_len(&app.project.timeline.clips[i].source.clone());
            let clip = &mut app.project.timeline.clips[i];
            ui.label(
                clip.source
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            );
            ui.horizontal(|ui| {
                ui.label("In ");
                ui.add(
                    egui::DragValue::new(&mut clip.src_in)
                        .range(0.0..=src_len.max(0.0))
                        .speed(0.05)
                        .suffix(" s"),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Out");
                ui.add(
                    egui::DragValue::new(&mut clip.src_out)
                        .range(0.0..=src_len.max(0.0))
                        .speed(0.05)
                        .suffix(" s"),
                );
            });
            if clip.src_out < clip.src_in {
                clip.src_out = clip.src_in;
            }
            ui.label(format!("Length: {:.2} s", clip.duration()));

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("◀ move").clicked() {
                    app.move_selected(-1);
                }
                if ui.button("move ▶").clicked() {
                    app.move_selected(1);
                }
            });
            if ui.button("🗑 Delete clip").clicked() {
                app.delete_selected();
            }

            ui.separator();
            if app.busy() {
                ui.add(egui::ProgressBar::new(app.export_progress).show_percentage());
            }
        });
}

fn timeline(app: &mut NexusApp, ctx: &egui::Context, dur: f64) {
    egui::TopBottomPanel::bottom("timeline")
        .resizable(false)
        .min_height(96.0)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.strong("Timeline");
                ui.label(format!("{:.2}s / {:.2}s", app.playhead.min(dur), dur));
            });

            if dur > 0.0 {
                let mut ph = app.playhead;
                if ui
                    .add(
                        egui::Slider::new(&mut ph, 0.0..=dur)
                            .show_value(false)
                            .text("playhead"),
                    )
                    .changed()
                {
                    app.playhead = ph;
                    app.playing = false;
                    app.last_seek = ctx.input(|i| i.time);
                }
            }

            ui.add_space(4.0);
            let total = dur.max(0.001);
            let full_w = ui.available_width();
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let clips = app.project.timeline.clips.clone();
                    for (idx, c) in clips.iter().enumerate() {
                        let w = ((c.duration() / total) as f32 * full_w).max(24.0);
                        let label = c
                            .source
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| format!("clip {}", idx + 1));
                        let selected = app.selected == Some(idx);
                        let resp = ui.add_sized(
                            [w, 44.0],
                            egui::SelectableLabel::new(
                                selected,
                                format!("{label}\n{:.1}s", c.duration()),
                            ),
                        );
                        if resp.clicked() {
                            app.selected = Some(idx);
                            // jump playhead to this clip's start
                            let start: f64 = clips[..idx].iter().map(|c| c.duration()).sum();
                            app.playhead = start;
                            app.playing = false;
                            app.last_seek = ctx.input(|i| i.time);
                        }
                    }
                });
            });
        });
}

fn render_frame(src: &Path, t: f64) -> anyhow::Result<Frame> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let out = std::env::temp_dir().join(format!(
        "nexus-preview-{}-{}.png",
        std::process::id(),
        nonce
    ));

    let status = Command::new(ffmpeg_bin())
        .args(["-y", "-hide_banner", "-loglevel", "error"])
        .args(["-ss", &format!("{t:.3}")])
        .arg("-i")
        .arg(src)
        .args(["-frames:v", "1", "-vf", "scale=720:-2", "-f", "image2"])
        .arg(&out)
        .status()?;
    if !status.success() {
        anyhow::bail!("ffmpeg could not extract a frame");
    }
    let img = image::open(&out)?.to_rgba8();
    let (w, h) = img.dimensions();
    let _ = std::fs::remove_file(&out);
    Ok((w, h, img.into_raw()))
}
