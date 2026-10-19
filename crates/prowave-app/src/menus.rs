//! Top menu bar with the same menus and entries as Logic Pro. Each entry is drawn like a macOS
//! menu item: check-mark column, title, key command on the right; entries that do not apply
//! are greyed out. The Smart Controls button in the control bar switches the bar and its
//! menus between the dark look and the light (white) macOS look (`UiState::light_menus`).
use crate::file_commands::FileCmd;
use crate::model::{Project, TrackKind};
use crate::Bottom;
use egui::{pos2, vec2, Align2, Color32, FontId, Key, KeyboardShortcut, Modifiers, Sense, Stroke};

const FILE: &[&str] = &[
    "New…",
    "New from Template…",
    "Open…",
    "Open Recent",
    "-",
    "Close",
    "Save",
    "Save As…",
    "Save A Copy As…",
    "Save as Template…",
    "Revert to Saved",
    "-",
    "Import",
    "Export",
    "Bounce",
    "Share",
];
const EDIT: &[&str] = &[
    "Undo",
    "Redo",
    "Undo History…",
    "Delete Undo History",
    "-",
    "Cut",
    "Copy",
    "Paste",
    "Paste Replace",
    "Delete",
    "-",
    "Select All",
    "Deselect All",
    "-",
    "Split at Playhead",
    "Join",
    "Bounce and Join",
];
const TRACK: &[&str] = &[
    "New",
    "New Aux Track",
    "Rename Track",
    "Delete Track",
    "-",
    "Create Track Stack…",
    "Show Master Track",
    "Show Tempo Track",
    "Show Chord Track",
    "Show Marker Track",
    "Show Arrangement Track",
    "-",
    "Assign Track Color…",
    "Assign Track Icon…",
    "Configure Track Header…",
];
const NAVIGATE: &[&str] = &[
    "Go to Beginning",
    "Go to End",
    "Go to Position…",
    "-",
    "Go to Left Locator",
    "Go to Right Locator",
    "-",
    "Go to Next Marker",
    "Go to Previous Marker",
    "-",
    "Go to Next Region",
    "Go to Previous Region",
    "-",
    "Zoom to Fit",
    "Zoom In",
    "Zoom Out",
];
const RECORD: &[&str] = &[
    "Record",
    "Record / Record Repeat",
    "Record Toggle",
    "Record Enable",
    "-",
    "Capture Recording",
    "-",
    "New Take",
    "Delete Take",
    "Bounce In Place",
    "-",
    "Record Project Settings",
    "Project Settings",
];
const MIX: &[&str] = &[
    "Show Automation",
    "Create Automation",
    "Convert Automation",
    "Delete Automation",
    "-",
    "Group",
    "Edit Groups…",
    "-",
    "Pre-Fader Metering",
    "Mastering",
    "Create Mastering Track",
    "Stem Splitter…",
    "-",
    "Bounce",
    "Export",
    "Import",
];
/// Logic Pro 11's View menu.
const VIEW: &[&str] = &[
    "Hide All Automation",
    "Show Automation",
    "Create Track Automation",
    "Convert Automation",
    "-",
    "Show Region Automation",
    "Show Track Automation",
    "-",
    "Show Advanced Tools",
    "Show Smart Controls",
    "-",
    "Show Piano Roll",
    "Show Score Editor",
    "Show Step Sequencer",
    "Show Session Player",
    "Show Flex Editor",
    "-",
    "Show Mixer",
    "Show Library",
    "Show Quick Help",
    "Show Inspector",
    "Show Performers",
    "Show Project Notes",
    "Show Movie",
    "Show Media Browser",
    "-",
    "Show Toolbar",
    "Customize Toolbar…",
    "-",
    "Show Track Header",
    "Show Ruler",
    "Show Global Tracks",
    "Show Colors",
    "Show Markers",
    "Show Grid",
    "Show Smart Tempo",
    "-",
    "Enter Full Screen",
];
const WINDOW: &[&str] = &[
    "Minimize",
    "Zoom",
    "-",
    "Record",
    "Score Editor",
    "Step Sequencer",
    "Smart Tempo",
    "Piano Roll",
    "Mixer",
    "Plugin Manager",
    "Video",
    "-",
    "Bring All to Front",
];
const HELP: &[&str] = &["Quick Help", "Logic-style Key Commands…", "About Pro Wave"];

/// View entries Logic greys out while there is nothing to act on.
const GREYED: &[&str] = &["Create Track Automation", "Convert Automation"];

/// Colours of one menu look.
struct Look {
    bar: Color32,
    bar_text: Color32,
    menu: Color32,
    text: Color32,
    dim: Color32,
    hover: Color32,
    hover_text: Color32,
    separator: Color32,
}

const DARK: Look = Look {
    bar: Color32::from_rgb(0x3a, 0x3a, 0x3c),
    bar_text: Color32::from_rgb(0xdc, 0xdc, 0xdc),
    menu: Color32::from_rgb(0x4a, 0x4a, 0x4c),
    text: Color32::from_rgb(0xe6, 0xe6, 0xe6),
    dim: Color32::from_rgb(0x8a, 0x8a, 0x8c),
    hover: Color32::from_rgb(0x2f, 0x6f, 0xd8),
    hover_text: Color32::WHITE,
    separator: Color32::from_rgb(0x5e, 0x5e, 0x60),
};

/// macOS light menu bar and menus, as in Logic with the system in light mode.
const LIGHT: Look = Look {
    bar: Color32::from_rgb(0xf5, 0xf5, 0xf5),
    bar_text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
    menu: Color32::from_rgb(0xec, 0xec, 0xec),
    text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
    dim: Color32::from_rgb(0xa8, 0xa8, 0xaa),
    hover: Color32::from_rgb(0x2f, 0x7b, 0xf6),
    hover_text: Color32::WHITE,
    separator: Color32::from_rgb(0xd2, 0xd2, 0xd4),
};

fn look(light: bool) -> &'static Look {
    if light {
        &LIGHT
    } else {
        &DARK
    }
}

/// Fill for the panel behind the menu bar.
pub fn bar_fill(light: bool) -> Color32 {
    look(light).bar
}

/// Logic's default key commands for the entries this app implements.
fn shortcut(label: &str) -> Option<KeyboardShortcut> {
    if let Some(cmd) = FileCmd::from_menu(label) {
        return cmd.shortcut();
    }
    let key = match label {
        "Show Library" => Key::Y,
        "Show Inspector" => Key::I,
        "Show Mixer" => Key::X,
        _ => return None,
    };
    Some(KeyboardShortcut::new(Modifiers::NONE, key))
}

/// Whether `label` shows a check mark (what it shows is visible).
fn checked(label: &str, s: &crate::UiState) -> bool {
    match label {
        "Show Piano Roll" => s.bottom == Bottom::PianoRoll,
        "Show Mixer" => s.show_mixer && s.bottom == Bottom::Mixer,
        "Show Library" => s.show_library,
        "Show Inspector" => s.show_inspector,
        // Always visible in this app.
        "Show Toolbar" | "Show Track Header" | "Show Ruler" | "Show Colors" | "Show Grid" => true,
        _ => false,
    }
}

/// Runs a menu entry (File entries are queued in `UiState::file_cmd` for `App`).
fn apply(label: &str, p: &mut Project, s: &mut crate::UiState) {
    match label {
        "Show Mixer" | "Mixer" => {
            s.show_mixer = !(s.show_mixer && s.bottom == Bottom::Mixer);
            s.bottom = Bottom::Mixer;
        }
        "Show Library" => s.show_library = !s.show_library,
        "Show Inspector" => s.show_inspector = !s.show_inspector,
        "Show Piano Roll" | "Piano Roll" => {
            s.bottom = if s.bottom == Bottom::PianoRoll {
                Bottom::Mixer
            } else {
                Bottom::PianoRoll
            }
        }
        "New Aux Track" => {
            p.add_track(TrackKind::Aux);
        }
        "Go to Beginning" => p.position_beats = 0.0,
        "Record" => p.recording = !p.recording,
        _ => {
            if let Some(cmd) = FileCmd::from_menu(label) {
                s.file_cmd = Some(cmd);
            }
        }
    }
}

/// One menu entry: check column, title, key command. Returns true when chosen.
fn item(ui: &mut egui::Ui, label: &str, check: bool, key: Option<String>, look: &Look) -> bool {
    item_with(ui, label, !GREYED.contains(&label), check, key, look)
}

/// Like [`item`], with the greyed-out state given by the caller.
fn item_with(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    check: bool,
    key: Option<String>,
    look: &Look,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
    let hot = enabled && resp.hovered();
    let g = ui.painter();
    if hot {
        g.rect_filled(rect.shrink2(vec2(0.0, 1.0)), 4.0, look.hover);
    }
    let color = match (enabled, hot) {
        (false, _) => look.dim,
        (true, true) => look.hover_text,
        (true, false) => look.text,
    };
    let font = FontId::proportional(13.0);
    if check {
        g.text(
            pos2(rect.left() + 8.0, rect.center().y),
            Align2::LEFT_CENTER,
            "✔",
            FontId::proportional(11.0),
            color,
        );
    }
    g.text(
        pos2(rect.left() + 24.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font.clone(),
        color,
    );
    if let Some(key) = key {
        let key_color = if hot { look.hover_text } else { look.dim };
        g.text(
            pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            key,
            font,
            key_color,
        );
    }
    enabled && resp.clicked()
}

fn separator(ui: &mut egui::Ui, look: &Look) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    let y = rect.center().y;
    ui.painter().line_segment(
        [pos2(rect.left() + 10.0, y), pos2(rect.right() - 10.0, y)],
        Stroke::new(1.0, look.separator),
    );
}

/// How a remembered project is listed: "Night Drive  (Music)", so equal names in different
/// folders can be told apart.
fn recent_label(path: &std::path::Path) -> String {
    let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
    match path
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|d| d.to_str())
    {
        Some(folder) => format!("    {name}  ({folder})"),
        None => format!("    {name}"),
    }
}

/// "Open Recent" with the remembered projects under it. They are listed flat, because this
/// menu has no submenus.
fn recent_items(ui: &mut egui::Ui, s: &mut crate::UiState, look: &Look) {
    item_with(ui, "Open Recent", false, false, None, look);
    if s.recent.paths().is_empty() {
        item_with(ui, "    (none)", false, false, None, look);
        return;
    }
    let mut picked = None;
    for path in s.recent.paths() {
        if item(ui, &recent_label(path), false, None, look) {
            picked = Some(path.clone());
        }
    }
    if let Some(path) = picked {
        s.recent_pick = Some(path);
        s.file_cmd = Some(FileCmd::OpenRecent);
        ui.close();
    } else if item(ui, "    Clear Menu", false, None, look) {
        s.recent.clear();
        ui.close();
    }
}

fn menu(ui: &mut egui::Ui, title: &str, items: &[&str], p: &mut Project, s: &mut crate::UiState) {
    let look = look(s.light_menus);
    let title = egui::RichText::new(title).color(look.bar_text);
    ui.menu_button(title, |ui| {
        ui.set_min_width(260.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        for it in items {
            if *it == "-" {
                separator(ui, look);
                continue;
            }
            if *it == "Open Recent" {
                recent_items(ui, s, look);
                continue;
            }
            let key = shortcut(it).map(|k| ui.ctx().format_shortcut(&k));
            if item(ui, it, checked(it, s), key, look) {
                apply(it, p, s);
                ui.close();
            }
        }
    });
}

/// Style of the bar buttons and of the drop-down menus for the current look.
fn menu_style(light: bool) -> impl Fn(&mut egui::Style) + Send + Sync + 'static {
    move |style: &mut egui::Style| {
        egui::containers::menu::menu_style(style);
        let l = look(light);
        let hover_fill = if light {
            Color32::from_rgb(0xdc, 0xdc, 0xdc)
        } else {
            Color32::from_rgb(0x55, 0x55, 0x58)
        };
        let v = &mut style.visuals;
        v.window_fill = l.menu;
        v.window_stroke = Stroke::new(1.0, l.separator);
        v.menu_corner_radius = egui::CornerRadius::same(6);
        v.override_text_color = None;
        v.widgets.noninteractive.fg_stroke.color = l.text;
        v.widgets.inactive.fg_stroke.color = l.bar_text;
        v.widgets.hovered.weak_bg_fill = hover_fill;
        v.widgets.hovered.fg_stroke.color = l.bar_text;
        v.widgets.open.weak_bg_fill = hover_fill;
        v.widgets.open.fg_stroke.color = l.bar_text;
        v.widgets.active.fg_stroke.color = l.bar_text;
    }
}

/// Key commands of the View entries above (Y, I, X), unless a text field has focus.
pub fn handle_keys(ctx: &egui::Context, p: &mut Project, s: &mut crate::UiState) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    for label in ["Show Library", "Show Inspector", "Show Mixer"] {
        if let Some(k) = shortcut(label) {
            if ctx.input_mut(|i| i.consume_shortcut(&k)) {
                apply(label, p, s);
            }
        }
    }
}

pub fn menu_bar(ui: &mut egui::Ui, p: &mut Project, s: &mut crate::UiState) {
    let light = s.light_menus;
    let bar = egui::MenuBar::new()
        .style(menu_style(light))
        .config(egui::containers::menu::MenuConfig::new().style(menu_style(light)));
    bar.ui(ui, |ui| {
        ui.label(
            egui::RichText::new("Pro Wave")
                .strong()
                .color(look(light).bar_text),
        );
        menu(ui, "File", FILE, p, s);
        menu(ui, "Edit", EDIT, p, s);
        menu(ui, "Track", TRACK, p, s);
        menu(ui, "Navigate", NAVIGATE, p, s);
        menu(ui, "Record", RECORD, p, s);
        menu(ui, "Mix", MIX, p, s);
        menu(ui, "View", VIEW, p, s);
        menu(ui, "Window", WINDOW, p, s);
        menu(ui, "Help", HELP, p, s);
        let warn = if light {
            Color32::from_rgb(0xb0, 0x5a, 0x00)
        } else {
            Color32::from_rgb(0xff, 0xb0, 0x40)
        };
        if let Some(msg) = &s.status {
            ui.colored_label(warn, msg);
        }
        // The engine plays only its first MAX_TRACKS tracks; say so instead of failing silently.
        let max = prowave_engine::MAX_TRACKS;
        if p.tracks.len() > max {
            ui.colored_label(warn, format!("Tracks {}–{} are silent (limit {max})", max + 1, p.tracks.len()))
                .on_hover_text("The audio engine plays up to 64 tracks. Tracks after that are kept in the project but not played.");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_menu_checks_follow_the_panels() {
        let mut s = crate::UiState::new();
        let mut p = Project::demo();
        assert!(checked("Show Library", &s) && checked("Show Mixer", &s));
        apply("Show Library", &mut p, &mut s);
        assert!(!s.show_library && !checked("Show Library", &s));
        apply("Show Piano Roll", &mut p, &mut s);
        assert!(checked("Show Piano Roll", &s) && !checked("Show Mixer", &s));
        apply("Show Mixer", &mut p, &mut s);
        assert!(checked("Show Mixer", &s));
    }

    #[test]
    fn file_entries_queue_their_command() {
        let mut s = crate::UiState::new();
        apply("Save As…", &mut Project::demo(), &mut s);
        assert_eq!(s.file_cmd, Some(FileCmd::SaveAs));
        assert!(shortcut("Show Library").is_some() && shortcut("Bounce").is_none());
        assert!(GREYED.iter().all(|g| VIEW.contains(g)));

        for (label, cmd) in [
            ("Save A Copy As…", FileCmd::SaveCopy),
            ("Revert to Saved", FileCmd::Revert),
        ] {
            assert!(FILE.contains(&label), "{label} is in the File menu");
            apply(label, &mut Project::demo(), &mut s);
            assert_eq!(s.file_cmd.take(), Some(cmd));
        }
    }

    #[test]
    fn new_aux_track_adds_an_aux_track() {
        let (mut p, mut s) = (Project::demo(), crate::UiState::new());
        assert!(TRACK.contains(&"New Aux Track"));
        let n = p.tracks.len();
        apply("New Aux Track", &mut p, &mut s);
        assert_eq!(p.tracks.len(), n + 1);
        assert_eq!(p.tracks[n].kind, TrackKind::Aux);
        assert_eq!(p.selected_track, n);
    }

    #[test]
    fn recent_projects_are_listed_with_their_folder() {
        let label = recent_label(std::path::Path::new("/home/me/Music/Night Drive.prowave"));
        assert_eq!(label, "    Night Drive  (Music)");
        assert_eq!(
            recent_label(std::path::Path::new("Solo.prowave")),
            "    Solo"
        );
    }
}
