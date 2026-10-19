//! File > New… / Open… / Save / Save As…: native dialogs (rfd), key commands, the "save
//! changes?" prompt and the window title ("<project> - Tracks", as in Logic).
use crate::model::Project;
use crate::{project_io, App};
use egui::{Key, KeyboardShortcut, Modifiers};
use prowave_project::ProjectDoc;
use rfd::FileDialog;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileCmd {
    New,
    Open,
    Save,
    SaveAs,
}

impl FileCmd {
    /// Save As before Save: egui ignores an extra Shift when matching Cmd+S.
    const ALL: [FileCmd; 4] = [FileCmd::SaveAs, FileCmd::Save, FileCmd::Open, FileCmd::New];

    /// Logic's key commands: Cmd+N, Cmd+O, Cmd+S, Shift+Cmd+S (Ctrl on Windows/Linux).
    pub fn shortcut(self) -> KeyboardShortcut {
        match self {
            FileCmd::New => KeyboardShortcut::new(Modifiers::COMMAND, Key::N),
            FileCmd::Open => KeyboardShortcut::new(Modifiers::COMMAND, Key::O),
            FileCmd::Save => KeyboardShortcut::new(Modifiers::COMMAND, Key::S),
            FileCmd::SaveAs => KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::S),
        }
    }

    /// The File menu entry that runs this command.
    pub fn from_menu(label: &str) -> Option<Self> {
        match label {
            "New…" => Some(FileCmd::New),
            "Open…" => Some(FileCmd::Open),
            "Save" => Some(FileCmd::Save),
            "Save As…" => Some(FileCmd::SaveAs),
            _ => None,
        }
    }
}

/// The file command whose key command was pressed this frame, if any (consumes the keys).
pub fn pressed(ctx: &egui::Context) -> Option<FileCmd> {
    FileCmd::ALL
        .into_iter()
        .find(|c| ctx.input_mut(|i| i.consume_shortcut(&c.shortcut())))
}

/// What gets saved, minus state that changes on its own (playhead, selected track), so that
/// playing or clicking around does not count as an unsaved change.
pub fn content(p: &Project) -> ProjectDoc {
    let mut doc = project_io::to_doc(p).0;
    doc.position_beats = 0.0;
    doc.selected_track = 0;
    doc
}

fn dialog() -> FileDialog {
    FileDialog::new().add_filter("Pro Wave Project", &[prowave_project::EXTENSION])
}

/// The answer to the "save changes?" prompt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Answer {
    Save,
    DontSave,
    Cancel,
}

impl App {
    /// Runs a File menu command. New / Open first ask what to do with unsaved changes.
    pub fn run_file_cmd(&mut self, cmd: FileCmd, ctx: &egui::Context) {
        let discards = matches!(cmd, FileCmd::New | FileCmd::Open);
        if discards && content(&self.project) != self.saved {
            self.ui.ask_save = Some(cmd);
        } else {
            self.do_file_cmd(cmd, ctx);
        }
        ctx.request_repaint();
    }

    fn do_file_cmd(&mut self, cmd: FileCmd, ctx: &egui::Context) {
        match cmd {
            FileCmd::New => self.replace(Project::empty(), None),
            FileCmd::Open => {
                if let Some(path) = dialog().set_title("Open Project").pick_file() {
                    self.open(&path, ctx);
                }
            }
            FileCmd::Save => {
                self.save();
            }
            FileCmd::SaveAs => {
                self.save_as();
            }
        }
    }

    /// The "Do you want to save the changes…?" prompt. Drawn by the app itself (not a system
    /// message box), so it looks the same everywhere and needs no helper program on Linux.
    pub fn show_save_prompt(&mut self, ctx: &egui::Context) {
        let Some(cmd) = self.ui.ask_save else {
            return;
        };
        let mut answer = None;
        let modal = egui::Modal::new(egui::Id::unique(("file", "save_prompt"))).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Pro Wave");
            ui.add_space(6.0);
            ui.label(format!(
                "Do you want to save the changes made to the project \"{}\"?",
                self.project.name
            ));
            ui.label("Your changes will be lost if you don't save them.");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Don't Save").clicked() {
                    answer = Some(Answer::DontSave);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Save").clicked() {
                        answer = Some(Answer::Save);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(Answer::Cancel);
                    }
                });
            });
        });
        if modal.should_close() && answer.is_none() {
            answer = Some(Answer::Cancel);
        }
        let Some(answer) = answer else {
            return;
        };
        self.ui.ask_save = None;
        let proceed = match answer {
            Answer::Save => self.save(),
            Answer::DontSave => true,
            Answer::Cancel => false,
        };
        if proceed {
            self.do_file_cmd(cmd, ctx);
        }
        ctx.request_repaint();
    }

    /// Opens `path`, replacing the current project (no prompt; callers ask first).
    pub fn open(&mut self, path: &Path, ctx: &egui::Context) {
        match project_io::open_project(path, ctx) {
            Ok((p, warnings)) => {
                self.replace(p, Some(path.to_path_buf()));
                if !warnings.is_empty() {
                    self.ui.status = Some(format!("missing media: {}", warnings.join(", ")));
                }
            }
            Err(e) => self.ui.status = Some(format!("cannot open {}: {e}", path.display())),
        }
    }

    /// Save to the current file, or ask for one. Returns false if cancelled or failed.
    fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(path) => self.save_to(&path),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) -> bool {
        let name = format!("{}.{}", self.project.name, prowave_project::EXTENSION);
        match dialog()
            .set_title("Save As")
            .set_file_name(name)
            .save_file()
        {
            Some(path) => self.save_to(&path),
            None => false,
        }
    }

    pub fn save_to(&mut self, path: &Path) -> bool {
        match project_io::save_project(&self.project, path) {
            Ok(path) => {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    self.project.name = stem.to_string();
                }
                self.path = Some(path);
                self.saved = content(&self.project);
                self.ui.status = None;
                true
            }
            Err(e) => {
                self.ui.status = Some(format!("cannot save {}: {e}", path.display()));
                false
            }
        }
    }

    /// Swaps in a whole new project and resets editor state that indexes into the old one.
    fn replace(&mut self, mut p: Project, path: Option<PathBuf>) {
        // A new song revision makes `EngineSync` rebuild and swap the engine's song.
        p.song_rev = self.project.song_rev.wrapping_add(1);
        self.project = p;
        self.path = path;
        self.saved = content(&self.project);
        self.ui.reset_views();
    }

    /// Keeps the window title in step with the project name.
    pub fn update_title(&mut self, ctx: &egui::Context) {
        let title = format!("Pro Wave — {} - Tracks", self.project.name);
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn app() -> App {
        let project = Project::demo();
        App {
            saved: content(&project),
            project,
            ui: crate::UiState::new(),
            sync: None,
            path: None,
            title: String::new(),
        }
    }

    #[test]
    fn playhead_and_selection_are_not_unsaved_changes() {
        let mut p = Project::demo();
        let before = content(&p);
        p.position_beats = 12.0;
        p.selected_track = 2;
        assert_eq!(content(&p), before);
        p.tempo += 1.0;
        assert_ne!(content(&p), before);
    }

    #[test]
    fn menu_entries_and_key_commands() {
        assert_eq!(FileCmd::from_menu("Save As…"), Some(FileCmd::SaveAs));
        assert_eq!(FileCmd::from_menu("Bounce"), None);
        let save_as = FileCmd::SaveAs.shortcut();
        assert!(save_as.modifiers.shift && save_as.logical_key == Key::S);
        assert!(!FileCmd::Save.shortcut().modifiers.shift);
    }

    #[test]
    fn save_then_open_tracks_path_name_and_clean_state() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut a = app();
        a.project.tempo = 97.0;
        assert_ne!(content(&a.project), a.saved);

        assert!(a.save_to(&dir.path().join("Night Drive")));
        let file = dir.path().join("Night Drive.prowave");
        assert_eq!(a.path.as_deref(), Some(file.as_path()));
        assert_eq!(a.project.name, "Night Drive");
        assert_eq!(content(&a.project), a.saved);

        let rev = a.project.song_rev;
        a.project.tempo = 120.0;
        a.open(&file, &ctx);
        assert_eq!(a.project.tempo, 97.0);
        assert_ne!(a.project.song_rev, rev, "engine must rebuild the song");
        assert_eq!(content(&a.project), a.saved);
        a.update_title(&ctx);
        assert_eq!(a.title, "Pro Wave — Night Drive - Tracks");
    }

    #[test]
    fn new_with_unsaved_changes_asks_first() {
        let ctx = egui::Context::default();

        let mut clean = app();
        let rev = clean.project.song_rev;
        clean.run_file_cmd(FileCmd::New, &ctx);
        assert!(clean.ui.ask_save.is_none(), "nothing to lose, no prompt");
        assert_ne!(clean.project.song_rev, rev, "New replaced the project");

        let mut dirty = app();
        dirty.project.tempo += 1.0;
        let tracks = dirty.project.tracks.len();
        dirty.run_file_cmd(FileCmd::New, &ctx);
        assert_eq!(dirty.ui.ask_save, Some(FileCmd::New));
        assert_eq!(
            dirty.project.tracks.len(),
            tracks,
            "not replaced until answered"
        );

        // Drawing the prompt (nobody clicks) keeps it open.
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            dirty.show_save_prompt(ui.ctx())
        });
        out.textures_delta.clear();
        assert_eq!(dirty.ui.ask_save, Some(FileCmd::New));
    }

    #[test]
    fn save_commands_never_prompt() {
        let ctx = egui::Context::default();
        let dir = tempfile::tempdir().unwrap();
        let mut a = app();
        a.project.tempo += 1.0;
        a.save_to(&dir.path().join("x"));
        a.project.tempo += 1.0;
        a.run_file_cmd(FileCmd::Save, &ctx);
        assert!(a.ui.ask_save.is_none());
        assert_eq!(content(&a.project), a.saved);
    }

    #[test]
    fn open_failure_keeps_the_project_and_reports() {
        let mut a = app();
        let tracks = a.project.tracks.len();
        a.open(
            Path::new("does-not-exist.prowave"),
            &egui::Context::default(),
        );
        assert_eq!(a.project.tracks.len(), tracks);
        assert!(a.ui.status.is_some());
    }
}
