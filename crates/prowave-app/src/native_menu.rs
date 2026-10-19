//! macOS: the menus live in the system menu bar at the top of the screen, as in Logic, instead
//! of a bar inside the window (`menus::IN_WINDOW`). Built with `muda` from the same menu lists
//! as the in-window bar, and run through the same `menus::apply`.
//!
//! What each entry becomes is decided by [`kind`], which is plain data so it is tested on every
//! platform; only [`NativeMenu`] itself is macOS-only.
// Off macOS only the tests use the entry mapping.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]
use crate::file_commands::FileCmd;
use egui::Key;

/// What a menu-list entry becomes in the system menu bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// One of macOS's own items, which the system runs (minimize, zoom, full screen…).
    System(SystemItem),
    /// An entry with a check mark that follows `menus::checked`.
    Check,
    /// A plain entry, with Logic's key command if it has one with Cmd. Single-key commands
    /// (Y, I, X) are left to the app, so they never steal typing from a text field.
    Plain(Option<Accel>),
    /// Shown in the app menu on macOS instead (About Pro Wave).
    Skip,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemItem {
    Minimize,
    Zoom,
    FullScreen,
    BringAllToFront,
}

/// A key command with Cmd: the letter key and whether Shift is held too.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Accel {
    pub key: Key,
    pub shift: bool,
}

pub fn kind(label: &str) -> Kind {
    match label {
        "Minimize" => return Kind::System(SystemItem::Minimize),
        "Zoom" => return Kind::System(SystemItem::Zoom),
        "Enter Full Screen" => return Kind::System(SystemItem::FullScreen),
        "Bring All to Front" => return Kind::System(SystemItem::BringAllToFront),
        "About Pro Wave" => return Kind::Skip,
        _ => {}
    }
    if crate::menus::CHECKABLE.contains(&label) {
        return Kind::Check;
    }
    let accel = FileCmd::from_menu(label)
        .and_then(FileCmd::shortcut)
        .filter(|s| s.modifiers.command && !s.modifiers.alt)
        .map(|s| Accel {
            key: s.logical_key,
            shift: s.modifiers.shift,
        });
    Kind::Plain(accel)
}

#[cfg(target_os = "macos")]
pub use mac::NativeMenu;

/// Elsewhere the menus are drawn in the window, so there is nothing to install.
#[cfg(not(target_os = "macos"))]
pub struct NativeMenu;

#[cfg(not(target_os = "macos"))]
impl NativeMenu {
    pub fn update(
        &mut self,
        _: &egui::Context,
        _: &mut crate::model::Project,
        _: &mut crate::UiState,
    ) {
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{kind, Accel, Kind, SystemItem};
    use crate::model::Project;
    use crate::{menus, UiState};
    use muda::accelerator::{Accelerator, Code, Modifiers};
    use muda::{
        AboutMetadata, CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem,
        Submenu,
    };
    use std::path::PathBuf;
    use std::sync::mpsc;

    const QUIT: &str = "app:quit";
    const RECENT_CLEAR: &str = "recent:clear";
    const RECENT: &str = "recent:";

    /// The installed menu bar. Keep it alive for as long as the window is open.
    pub struct NativeMenu {
        _menu: Menu,
        events: mpsc::Receiver<String>,
        checks: Vec<(&'static str, CheckMenuItem, bool)>,
        recent_menu: Submenu,
        recent_shown: Vec<PathBuf>,
    }

    fn accelerator(a: Accel) -> Option<Accelerator> {
        let code = match a.key {
            egui::Key::N => Code::KeyN,
            egui::Key::O => Code::KeyO,
            egui::Key::S => Code::KeyS,
            _ => return None,
        };
        let mods = if a.shift {
            Modifiers::META | Modifiers::SHIFT
        } else {
            Modifiers::META
        };
        Some(Accelerator::new(mods, code))
    }

    impl NativeMenu {
        /// Puts Pro Wave's menus in the system menu bar. Menu clicks wake the app up.
        pub fn install(ctx: &egui::Context) -> Self {
            let (tx, events) = mpsc::channel();
            let wake = ctx.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let _ = tx.send(e.id.0);
                wake.request_repaint();
            }));

            let menu = Menu::new();
            let about = AboutMetadata {
                name: Some("Pro Wave".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                license: Some("GPL-3.0-or-later".into()),
                ..Default::default()
            };
            let app_menu = Submenu::new("Pro Wave", true);
            let quit = MenuItem::with_id(
                QUIT,
                "Quit Pro Wave",
                true,
                Some(Accelerator::new(Modifiers::META, Code::KeyQ)),
            );
            let _ = app_menu.append_items(&[
                &PredefinedMenuItem::about(Some("About Pro Wave"), Some(about)),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(Some("Hide Pro Wave")),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &quit,
            ]);
            let _ = menu.append(&app_menu);

            let mut checks = Vec::new();
            let recent_menu = Submenu::new("Open Recent", true);
            for (title, items) in menus::MENUS {
                let sub = Submenu::new(title, true);
                for &label in items {
                    if label == "-" {
                        let _ = sub.append(&PredefinedMenuItem::separator());
                        continue;
                    }
                    if label == "Open Recent" {
                        let _ = sub.append(&recent_menu);
                        continue;
                    }
                    let enabled = !menus::GREYED.contains(&label);
                    let item: Box<dyn IsMenuItem> = match kind(label) {
                        Kind::Skip => continue,
                        Kind::System(SystemItem::Minimize) => {
                            Box::new(PredefinedMenuItem::minimize(None))
                        }
                        Kind::System(SystemItem::Zoom) => {
                            Box::new(PredefinedMenuItem::maximize(Some("Zoom")))
                        }
                        Kind::System(SystemItem::FullScreen) => {
                            Box::new(PredefinedMenuItem::fullscreen(None))
                        }
                        Kind::System(SystemItem::BringAllToFront) => {
                            Box::new(PredefinedMenuItem::bring_all_to_front(None))
                        }
                        Kind::Check => {
                            let c = CheckMenuItem::with_id(label, label, enabled, false, None);
                            checks.push((label, c.clone(), false));
                            Box::new(c)
                        }
                        Kind::Plain(accel) => Box::new(MenuItem::with_id(
                            label,
                            label,
                            enabled,
                            accel.and_then(accelerator),
                        )),
                    };
                    let _ = sub.append(item.as_ref());
                }
                let _ = menu.append(&sub);
                match title {
                    "Window" => sub.set_as_windows_menu_for_nsapp(),
                    "Help" => sub.set_as_help_menu_for_nsapp(),
                    _ => {}
                }
            }
            menu.init_for_nsapp();
            let mut native = Self {
                _menu: menu,
                events,
                checks,
                recent_menu,
                // Differs from any real list, so the first `update` fills Open Recent.
                recent_shown: vec![PathBuf::from("\0")],
            };
            native.refresh_recent(&[]);
            native
        }

        /// Rebuilds File > Open Recent when the list of projects changed.
        fn refresh_recent(&mut self, paths: &[PathBuf]) {
            if self.recent_shown == paths {
                return;
            }
            while self.recent_menu.remove_at(0).is_some() {}
            for (k, path) in paths.iter().enumerate() {
                let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
                let item = MenuItem::with_id(format!("{RECENT}{k}"), name, true, None);
                let _ = self.recent_menu.append(&item);
            }
            if !paths.is_empty() {
                let _ = self.recent_menu.append(&PredefinedMenuItem::separator());
            }
            let clear = MenuItem::with_id(RECENT_CLEAR, "Clear Menu", !paths.is_empty(), None);
            let _ = self.recent_menu.append(&clear);
            self.recent_shown = paths.to_vec();
        }

        /// Runs the entries picked since the last frame and brings check marks and Open Recent
        /// up to date. Call once per frame, before the panels are drawn.
        pub fn update(&mut self, ctx: &egui::Context, p: &mut Project, s: &mut UiState) {
            while let Ok(id) = self.events.try_recv() {
                if id == QUIT {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else if id == RECENT_CLEAR {
                    s.recent.clear();
                } else if let Some(k) = id.strip_prefix(RECENT) {
                    let path = k
                        .parse::<usize>()
                        .ok()
                        .and_then(|k| s.recent.paths().get(k));
                    if let Some(path) = path.cloned() {
                        s.recent_pick = Some(path);
                        s.file_cmd = Some(crate::file_commands::FileCmd::OpenRecent);
                    }
                } else {
                    menus::apply(&id, p, s);
                }
            }
            for (label, item, shown) in &mut self.checks {
                let now = menus::checked(label, s);
                if now != *shown {
                    item.set_checked(now);
                    *shown = now;
                }
            }
            let paths = s.recent.paths().to_vec();
            self.refresh_recent(&paths);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_entries_become_the_systems_own() {
        assert_eq!(kind("Minimize"), Kind::System(SystemItem::Minimize));
        assert_eq!(kind("Zoom"), Kind::System(SystemItem::Zoom));
        assert_eq!(
            kind("Enter Full Screen"),
            Kind::System(SystemItem::FullScreen)
        );
        assert_eq!(
            kind("About Pro Wave"),
            Kind::Skip,
            "it lives in the app menu"
        );
    }

    #[test]
    fn only_cmd_key_commands_are_given_to_the_menu_bar() {
        let save_as = Accel {
            key: Key::S,
            shift: true,
        };
        assert_eq!(kind("Save As…"), Kind::Plain(Some(save_as)));
        assert_eq!(
            kind("Open…"),
            Kind::Plain(Some(Accel {
                key: Key::O,
                shift: false
            }))
        );
        // Y / I / X stay with the app, so typing in a text field still works.
        assert_eq!(kind("Show Library"), Kind::Check);
        assert_eq!(kind("Bounce"), Kind::Plain(None));
    }

    #[test]
    fn every_checkable_entry_is_in_a_menu() {
        for label in crate::menus::CHECKABLE {
            assert!(
                crate::menus::MENUS
                    .iter()
                    .any(|(_, items)| items.contains(label)),
                "{label}"
            );
        }
    }
}
