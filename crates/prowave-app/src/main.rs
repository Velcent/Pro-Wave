#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Pro Wave desktop app. Pure Rust (egui/eframe) for Windows, Linux and macOS.
// Debug builds abort if the audio callback allocates (see `prowave-engine` output.rs).
#[cfg(debug_assertions)]
#[global_allocator]
static ALLOC_GUARD: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

mod engine_sync;
mod file_commands;
mod import;
mod inspector;
mod library;
mod menus;
mod mixer;
mod model;
mod native_menu;
mod note_edit;
mod patches;
mod piano_roll;
mod project_io;
mod recent_files;
mod start_screen;
mod theme;
mod track_icons;
mod tracks_area;
mod transport;
mod widgets;

use model::Project;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Bottom {
    Mixer,
    PianoRoll,
}

pub struct UiState {
    pub show_mixer: bool,
    pub show_library: bool,
    pub show_inspector: bool,
    pub bottom: Bottom,
    /// Last import error or engine warning, shown in the menu bar.
    pub status: Option<String>,
    pub tracks: tracks_area::TracksView,
    pub mixer: mixer::MixerView,
    pub inspector: inspector::InspectorView,
    pub library: library::LibraryView,
    pub piano: piano_roll::PianoView,
    /// True while the pointer is over the Piano Roll (it owns Delete/Cmd+R then).
    pub piano_focus: bool,
    /// Live notes from the Piano Roll, forwarded to the engine every frame.
    pub audition: Vec<piano_roll::Audition>,
    /// File menu command picked this frame; run by `App` once the panels are drawn.
    pub file_cmd: Option<file_commands::FileCmd>,
    /// A New / Open waiting for the answer to the "save changes?" prompt.
    pub ask_save: Option<file_commands::FileCmd>,
    /// True while the "revert to the last saved version?" prompt is showing.
    pub ask_revert: bool,
    /// The project picked in File > Open Recent, until `App` has opened it.
    pub recent_pick: Option<std::path::PathBuf>,
    /// File > Open Recent. In memory only until `main` loads the saved list.
    pub recent: recent_files::Recent,
    /// DSP load shown by the LCD's CPU meter (1.0 = full budget); peak-held, falls slowly.
    pub cpu_load: f32,
    /// Light (white) menu bar and menus, toggled by the Smart Controls button.
    pub light_menus: bool,
    /// The Project Chooser, while it covers the window (File > New from Template…).
    pub chooser: Option<start_screen::Chooser>,
}

impl UiState {
    fn new() -> Self {
        Self {
            show_mixer: true,
            show_library: true,
            show_inspector: true,
            bottom: Bottom::Mixer,
            status: None,
            tracks: tracks_area::TracksView::default(),
            mixer: mixer::MixerView::default(),
            inspector: inspector::InspectorView::default(),
            library: library::LibraryView::default(),
            piano: piano_roll::PianoView::default(),
            piano_focus: false,
            audition: Vec::new(),
            file_cmd: None,
            ask_save: None,
            ask_revert: false,
            recent_pick: None,
            recent: recent_files::Recent::default(),
            cpu_load: 0.0,
            light_menus: false,
            chooser: None,
        }
    }

    /// After a project is replaced: drop editor state (selections, open regions) that indexes
    /// into the old project, keeping which panels are shown.
    fn reset_views(&mut self) {
        self.status = None;
        self.tracks = tracks_area::TracksView::default();
        self.mixer = mixer::MixerView::default();
        self.inspector = inspector::InspectorView::default();
        self.piano = piano_roll::PianoView::default();
        self.piano_focus = false;
        self.audition.clear();
    }
}

struct App {
    project: Project,
    ui: UiState,
    sync: Option<engine_sync::EngineSync>,
    /// The `.prowave` file the project was opened from or last saved to.
    path: Option<std::path::PathBuf>,
    /// Content at the last open/save, to detect unsaved changes (see `file_commands`).
    saved: prowave_project::ProjectDoc,
    /// Window title last sent to the viewport.
    title: String,
    /// macOS: the menus in the system menu bar (elsewhere they are drawn in the window).
    native_menu: Option<native_menu::NativeMenu>,
}

impl App {
    fn import(&mut self, path: &std::path::Path, ctx: &egui::Context) {
        if !import::is_audio_path(path) {
            self.ui.status = Some(format!("not an audio file: {}", path.display()));
            return;
        }
        if let Err(e) = import::import_audio(&mut self.project, path, ctx) {
            self.ui.status = Some(format!("cannot import {}: {e}", path.display()));
        }
    }
}

impl App {
    /// The window while the Project Chooser is up: the menu bar and the chooser, with the
    /// File key commands and prompts still working.
    fn chooser_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(menu) = self.native_menu.as_mut() {
            menu.update(ctx, &mut self.project, &mut self.ui);
        }
        if let Some(cmd) = file_commands::pressed(ctx) {
            self.ui.file_cmd = Some(cmd);
        }
        if let Some(sync) = self.sync.as_mut() {
            sync.sync(&mut self.project);
        }
        // Audio files dropped on the chooser go into the open project.
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            self.ui.chooser = None;
            for path in dropped {
                self.import(&path, ctx);
            }
        }
        top_bar(ui, &mut self.project, &mut self.ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| start_screen::show(ui, &mut self.ui));
        if let Some(cmd) = self.ui.file_cmd.take() {
            self.run_file_cmd(cmd, ctx);
        }
        self.show_save_prompt(ctx);
        self.update_title(ctx);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt) as f64;
        if self.ui.chooser.is_some() {
            self.chooser_ui(ui, &ctx);
            return;
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.project.playing = !self.project.playing;
        }
        if let Some(menu) = self.native_menu.as_mut() {
            menu.update(&ctx, &mut self.project, &mut self.ui);
        }
        if let Some(cmd) = file_commands::pressed(&ctx) {
            self.ui.file_cmd = Some(cmd);
        }
        menus::handle_keys(&ctx, &mut self.project, &mut self.ui);
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        for path in dropped {
            self.import(&path, &ctx);
        }
        if let Some(sync) = self.sync.as_mut() {
            sync.sync(&mut self.project);
            if sync.needs_retry() {
                ctx.request_repaint();
            }
            let load = sync.take_dsp_load();
            self.ui.cpu_load = load.max(self.ui.cpu_load - dt as f32 * 0.8);
            if sync.underruns() > 0 {
                self.ui.status = Some(format!("disk too slow: {} underruns", sync.underruns()));
            }
        } else if self.project.playing {
            // No audio device: fall back to a UI-driven clock.
            self.project.position_beats += dt * self.project.tempo as f64 / 60.0;
        }
        if self.project.playing {
            ctx.request_repaint();
        }
        let master_peaks = self.sync.as_ref().map(|s| s.master_peaks());
        let meters = match self.sync.as_ref() {
            Some(sync) => mixer::Meters {
                tracks: sync.track_meters(self.project.tracks.len()),
                master: sync.master_peaks(),
            },
            None => mixer::Meters::default(),
        };
        let p = &mut self.project;
        let s = &mut self.ui;

        top_bar(ui, p, s);
        egui::Panel::top("control")
            .exact_size(58.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_gray(0x47)))
            .show(ui, |ui| transport::control_bar(ui, p, s, master_peaks));
        if s.show_library {
            egui::Panel::left("library")
                .exact_size(290.0)
                .frame(egui::Frame::new())
                .show(ui, |ui| library::show(ui, p, s));
        }
        if s.show_inspector {
            egui::Panel::left("inspector")
                .exact_size(244.0)
                .frame(egui::Frame::new())
                .show(ui, |ui| inspector::show(ui, p, s, &meters));
        }
        if s.show_mixer || s.bottom == Bottom::PianoRoll {
            egui::Panel::bottom("bottom")
                .resizable(true)
                .default_size(500.0)
                .frame(egui::Frame::new())
                .show(ui, |ui| match s.bottom {
                    Bottom::Mixer => mixer::show(ui, p, s, &meters),
                    Bottom::PianoRoll => piano_roll::show(ui, p, s),
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| tracks_area::show(ui, p, s));

        // Forward live notes (piano keys, note auditioning) to the engine.
        // Notes on tracks past the engine's limit are silent, like the rest of those tracks.
        // A rejected event stays queued (with everything after it, to keep On/Off order)
        // and is retried next frame, so a Note Off is never lost.
        if let Some(sync) = self.sync.as_mut() {
            let mut sent = 0;
            for ev in &self.ui.audition {
                let cmd = match *ev {
                    piano_roll::Audition::On { track, .. }
                        if usize::from(track) >= prowave_engine::MAX_TRACKS =>
                    {
                        None
                    }
                    piano_roll::Audition::On { pitch, vel, track } => {
                        Some(prowave_engine::Command::NoteOn { pitch, vel, track })
                    }
                    piano_roll::Audition::Off { pitch } => {
                        Some(prowave_engine::Command::NoteOff { pitch })
                    }
                };
                if cmd.is_some_and(|cmd| !sync.engine.send(cmd)) {
                    break;
                }
                sent += 1;
            }
            self.ui.audition.drain(..sent);
            if !self.ui.audition.is_empty() {
                ctx.request_repaint();
            }
        } else {
            self.ui.audition.clear();
        }
        if let Some(cmd) = self.ui.file_cmd.take() {
            self.run_file_cmd(cmd, &ctx);
        }
        self.show_save_prompt(&ctx);
        self.show_revert_prompt(&ctx);
        self.update_title(&ctx);
    }
}

/// The in-window menu bar; on macOS, where the menus are in the system menu bar, only a line
/// for errors and warnings while there is one.
fn top_bar(ui: &mut egui::Ui, p: &mut Project, s: &mut UiState) {
    let frame = egui::Frame::side_top_panel(ui.style()).fill(menus::bar_fill(s.light_menus));
    if menus::IN_WINDOW {
        egui::Panel::top("menu")
            .frame(frame)
            .show(ui, |ui| menus::menu_bar(ui, p, s));
    } else if menus::has_status(p, s) {
        egui::Panel::top("status")
            .frame(frame)
            .show(ui, |ui| ui.horizontal(|ui| menus::status_line(ui, p, s)));
    }
}

/// Log to stderr; `RUST_LOG` overrides the default (warnings, plus info from ProWave; the
/// Vulkan loader's harmless layer-manifest warnings from `wgpu_hal` are hidden).
fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,wgpu_hal=error,prowave=info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// The Pro Wave logo, 256×256 with transparency (generated from dist/icons/prowave-1024.png).
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icon-256.png");

fn main() -> eframe::Result {
    // Release builds: on a crash, write a report to the temp folder instead of a raw panic.
    human_panic::setup_panic!(human_panic::Metadata::new(
        "Pro Wave",
        env!("CARGO_PKG_VERSION")
    ));
    init_logging();
    project_io::remove_stale_media(std::time::Duration::from_secs(7 * 24 * 3600));
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1600.0, 980.0])
        .with_title("Pro Wave — Untitled - Tracks")
        // Window class: lets Linux desktops match the window to its .desktop entry and icon.
        .with_app_id("prowave");
    // Title bar / taskbar / dock icon. A broken icon must not stop the app from starting.
    match eframe::icon_data::from_png_bytes(APP_ICON_PNG) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => tracing::warn!("app icon could not be decoded: {e}"),
    }
    let opts = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let result = eframe::run_native(
        "Pro Wave",
        opts,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            // The very first start shows the demo song; later starts begin clean.
            let project = if recent_files::take_first_launch() {
                Project::demo()
            } else {
                Project::empty()
            };
            let mut audio_error = None;
            let sync = match prowave_engine::Engine::start() {
                Ok(e) => Some(engine_sync::EngineSync::new(e, &project)),
                Err(err) => {
                    tracing::warn!("audio disabled: {err}");
                    audio_error = Some(format!("No sound: {err}. Check your audio device."));
                    None
                }
            };
            let mut app = App {
                saved: file_commands::content(&project),
                project,
                ui: UiState::new(),
                sync,
                path: None,
                title: String::new(),
                native_menu: None,
            };
            #[cfg(target_os = "macos")]
            {
                app.native_menu = Some(native_menu::NativeMenu::install(&cc.egui_ctx));
            }
            app.ui.status = audio_error;
            app.ui.recent = recent_files::Recent::load();
            // `prowave song.prowave` opens a project; `prowave song.wav drums.aiff` imports each
            // file as an audio track (into the opened project, if any).
            // `--play` starts the transport on launch; `--piano` opens the Piano Roll.
            let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
            let is_project = |a: &std::ffi::OsString| {
                std::path::Path::new(a)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(prowave_project::EXTENSION))
            };
            if let Some(file) = args.iter().find(|a| is_project(a)) {
                app.open(std::path::Path::new(file), &cc.egui_ctx);
            }
            for arg in args.iter().filter(|a| !is_project(a)) {
                if arg == "--play" {
                    app.project.playing = true;
                } else if arg == "--piano" {
                    app.ui.bottom = Bottom::PianoRoll;
                } else {
                    app.import(std::path::Path::new(arg), &cc.egui_ctx);
                }
            }
            Ok(Box::new(app))
        }),
    );
    // The app (and with it the engine's open media files) is gone by now.
    project_io::remove_session_media();
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_icon_decodes_as_a_square_rgba_image() {
        let icon = eframe::icon_data::from_png_bytes(super::APP_ICON_PNG).unwrap();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }
}
