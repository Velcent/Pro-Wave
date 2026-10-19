//! Tracks area (Logic Pro: main window "Tracks" area): local toolbar, ruler with cycle area and
//! playhead, track headers and the region lanes.
//!
//! Works today: select/move regions with Snap, Delete and Repeat regions, Set Locators by
//! Regions, drag in the ruler to set the cycle, click to move the playhead, scroll, zoom
//! (sliders and Ctrl+wheel), Catch playhead, add/duplicate tracks, M/S/R/I, volume, pan.
//! Menu entries that are not implemented yet are shown disabled.
use crate::model::{Project, Region, Track, TrackKind};
use crate::track_icons::{draw_icon, track_icon};
use crate::widgets::{menu_title, popup_button, stroke, todo_item, BLUE, BTN_BG, TEXT, TEXT_DIM};
use crate::{Bottom, UiState};
use egui::{
    pos2, vec2, Align2, Color32, FontId, Key, KeyboardShortcut, Modifiers, Painter, Pos2, Rect,
    Sense, Shape, Stroke, StrokeKind, Ui,
};

const TOOLBAR_H: f32 = 30.0;
const RULER_H: f32 = 26.0;
const HEADER_W: f32 = 340.0;
const NUM_W: f32 = 24.0;

const TOOLBAR_BG: Color32 = Color32::from_rgb(0x3c, 0x3c, 0x3c);
const HEADER_BG: Color32 = Color32::from_rgb(0x41, 0x41, 0x41);
const HEADER_SEL: Color32 = Color32::from_rgb(0x58, 0x58, 0x58);
const LANE_BG: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);
const LANE_SEL: Color32 = Color32::from_rgb(0x30, 0x30, 0x30);
const RULER_BG: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);
const CYCLE: Color32 = Color32::from_rgb(0xd4, 0x9e, 0x2a);
const MUTE_ON: Color32 = Color32::from_rgb(0x3b, 0x8c, 0xe6);
const SOLO_ON: Color32 = Color32::from_rgb(0xe6, 0xc2, 0x2e);
const REC_ON: Color32 = Color32::from_rgb(0xe0, 0x3a, 0x30);
const MON_ON: Color32 = Color32::from_rgb(0xe8, 0x8a, 0x2c);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Snap {
    Smart,
    Bar,
    Beat,
    Division,
    Ticks,
}

impl Snap {
    pub(crate) const ALL: [Snap; 5] = [
        Snap::Smart,
        Snap::Bar,
        Snap::Beat,
        Snap::Division,
        Snap::Ticks,
    ];
    pub(crate) fn label(self) -> &'static str {
        match self {
            Snap::Smart => "Smart",
            Snap::Bar => "Bar",
            Snap::Beat => "Beat",
            Snap::Division => "Division",
            Snap::Ticks => "Ticks",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DragMode {
    Overlap,
    NoOverlap,
    XFade,
    ShuffleR,
    ShuffleL,
}

impl DragMode {
    const ALL: [DragMode; 5] = [
        DragMode::Overlap,
        DragMode::NoOverlap,
        DragMode::XFade,
        DragMode::ShuffleR,
        DragMode::ShuffleL,
    ];
    fn label(self) -> &'static str {
        match self {
            DragMode::Overlap => "Overlap",
            DragMode::NoOverlap => "No Overlap",
            DragMode::XFade => "X-Fade",
            DragMode::ShuffleR => "Shuffle R",
            DragMode::ShuffleL => "Shuffle L",
        }
    }
}

const TOOLS: [&str; 14] = [
    "Pointer Tool",
    "Pencil Tool",
    "Eraser Tool",
    "Text Tool",
    "Scissors Tool",
    "Glue Tool",
    "Solo Tool",
    "Mute Tool",
    "Zoom Tool",
    "Fade Tool",
    "Automation Select Tool",
    "Automation Curve Tool",
    "Marquee Tool",
    "Flex Tool",
];

pub struct TracksView {
    /// Width of one bar in points (horizontal zoom).
    pub bar_w: f32,
    /// Track height in points (vertical zoom).
    pub row_h: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub snap: Snap,
    pub drag_mode: DragMode,
    pub catch: bool,
    /// (track, region)
    pub selected: Option<(usize, usize)>,
    grab_beats: f64,
    cycle_anchor: Option<f64>,
}

impl Default for TracksView {
    fn default() -> Self {
        Self {
            bar_w: 62.0,
            row_h: 56.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            snap: Snap::Smart,
            drag_mode: DragMode::NoOverlap,
            catch: true,
            selected: None,
            grab_beats: 0.0,
            cycle_anchor: None,
        }
    }
}

/// True when another track is soloed and `track` is not: it is silent until solo is released
/// (Logic plays only soloed tracks and greys the others' regions).
pub(crate) fn silenced_by_solo(p: &Project, track: usize) -> bool {
    p.tracks.get(track).is_some_and(|t| !t.solo) && p.tracks.iter().any(|t| t.solo)
}

/// Half the width of the playhead head's grab zone in the ruler.
const PLAYHEAD_GRAB: f32 = 8.0;

/// Beat under screen x `x` while dragging the playhead (never before the start of the song).
fn playhead_beat(x: f32, lanes_left: f32, scroll_x: f32, beat_px: f32) -> f64 {
    ((x - lanes_left + scroll_x) / beat_px.max(1e-3)).max(0.0) as f64
}

/// Half the height of the grab zone on a track's lower edge.
const RESIZE_GRIP: f32 = 3.0;

/// `(top, height)` of every track row in points, from the top of the first track (unscrolled):
/// the global track zoom `row_h` times each track's own height factor.
fn row_layout(p: &Project, row_h: f32) -> Vec<(f32, f32)> {
    let mut top = 0.0;
    p.tracks
        .iter()
        .map(|t| {
            let h = (row_h * t.height).round().max(1.0);
            let row = (top, h);
            top += h;
            row
        })
        .collect()
}

/// Index of the track row containing `y` (same origin as [`row_layout`]).
fn row_at(rows: &[(f32, f32)], y: f32) -> Option<usize> {
    rows.iter().position(|&(top, h)| y >= top && y < top + h)
}

fn id(key: impl std::hash::Hash + std::fmt::Debug) -> egui::Id {
    egui::Id::unique(("tracks_area", key))
}

/// Snap step in beats (quarter notes) for the current setting and zoom. `beat_px` is pixels
/// per quarter note; `beat_unit` is the signature's beat in quarter notes (0.5 in 6/8), so
/// Beat snaps to eighths in x/8 as in Logic. Divisions stay 16th notes.
pub(crate) fn snap_step(snap: Snap, beat_px: f32, beats_per_bar: f64, beat_unit: f64) -> f64 {
    match snap {
        Snap::Smart if beat_px >= 48.0 => 0.25,
        Snap::Smart if beat_px * beat_unit as f32 >= 12.0 => beat_unit,
        Snap::Smart => beats_per_bar,
        Snap::Bar => beats_per_bar,
        Snap::Beat => beat_unit,
        Snap::Division => 0.25,
        Snap::Ticks => 1.0 / 960.0,
    }
}

pub(crate) fn snap_to(b: f64, step: f64) -> f64 {
    (b / step).round() * step
}

// ------------------------------------------------------------------ small widgets

fn icon_button(
    ui: &Ui,
    g: &Painter,
    rect: Rect,
    key: &str,
    on: bool,
    draw: impl FnOnce(&Painter, Rect, Color32),
) -> egui::Response {
    let resp = ui.interact(rect, id(("icon", key)), Sense::click());
    if on {
        g.rect_filled(rect, 4.0, BLUE);
    } else if resp.hovered() {
        g.rect_filled(rect, 4.0, Color32::from_white_alpha(14));
    }
    draw(g, rect, if on { Color32::WHITE } else { TEXT });
    resp
}

/// Horizontal mini slider; returns the new 0..1 value.
fn mini_slider(ui: &Ui, g: &Painter, rect: Rect, key: &str, value: f32) -> f32 {
    let resp = ui.interact(
        rect.expand2(vec2(4.0, 6.0)),
        id(("slider", key)),
        Sense::click_and_drag(),
    );
    let mut v = value;
    if let Some(p) = resp.interact_pointer_pos() {
        v = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    }
    let cy = rect.center().y;
    g.line_segment(
        [pos2(rect.left(), cy), pos2(rect.right(), cy)],
        Stroke::new(2.0, Color32::from_gray(0x22)),
    );
    g.circle_filled(
        pos2(rect.left() + rect.width() * v, cy),
        5.5,
        Color32::from_gray(0xc8),
    );
    v
}

fn header_button_bg(selected_row: bool) -> Color32 {
    if selected_row {
        Color32::from_gray(0x72)
    } else {
        Color32::from_gray(0x5a)
    }
}

#[allow(clippy::too_many_arguments)]
fn header_button(
    ui: &Ui,
    g: &Painter,
    rect: Rect,
    key: (usize, &str),
    label: &str,
    on: bool,
    on_col: Color32,
    selected_row: bool,
) -> bool {
    let resp = ui.interact(rect, id(("hdrbtn", key)), Sense::click());
    g.rect_filled(
        rect,
        3.0,
        if on {
            on_col
        } else {
            header_button_bg(selected_row)
        },
    );
    let txt = if on && on_col == SOLO_ON {
        Color32::BLACK
    } else {
        Color32::from_gray(0xe8)
    };
    g.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.5),
        txt,
    );
    resp.clicked()
}

// ------------------------------------------------------------------ toolbar icons

fn draw_grid_icon(g: &Painter, r: Rect, c: Color32) {
    for (dx, dy) in [(-3.5, -3.5), (3.5, -3.5), (-3.5, 3.5), (3.5, 3.5)] {
        g.rect_filled(
            Rect::from_center_size(r.center() + vec2(dx, dy), vec2(5.0, 5.0)),
            1.0,
            c,
        );
    }
}

fn draw_list_icon(g: &Painter, r: Rect, c: Color32) {
    for i in -1..=1 {
        let y = r.center().y + i as f32 * 4.0;
        g.rect_filled(
            Rect::from_center_size(pos2(r.center().x, y), vec2(13.0, 2.4)),
            1.0,
            c,
        );
    }
}

fn draw_curve_icon(g: &Painter, r: Rect, c: Color32) {
    let p = r.center();
    let pts: Vec<Pos2> = (0..=12)
        .map(|i| {
            let t = i as f32 / 12.0;
            p + vec2(-7.0 + 14.0 * t, 5.0 - 10.0 * (t * t * (3.0 - 2.0 * t)))
        })
        .collect();
    g.add(Shape::line(pts, stroke(c)));
    g.circle_filled(p + vec2(-7.0, 5.0), 1.8, c);
    g.circle_filled(p + vec2(7.0, -5.0), 1.8, c);
}

fn draw_flex_icon(g: &Painter, r: Rect, c: Color32) {
    // Hourglass on its side (Flex).
    let p = r.center();
    g.add(Shape::line(
        vec![
            p + vec2(-7.0, -5.0),
            p + vec2(7.0, 5.0),
            p + vec2(7.0, -5.0),
            p + vec2(-7.0, 5.0),
            p + vec2(-7.0, -5.0),
        ],
        stroke(c),
    ));
}

fn draw_catch_icon(g: &Painter, r: Rect, c: Color32) {
    // Playhead with a follow arrow.
    let p = r.center();
    g.line_segment([p + vec2(-4.0, -7.0), p + vec2(-4.0, 7.0)], stroke(c));
    g.add(Shape::convex_polygon(
        vec![
            p + vec2(-8.0, -7.0),
            p + vec2(0.0, -7.0),
            p + vec2(-4.0, -3.0),
        ],
        c,
        Stroke::NONE,
    ));
    g.line_segment([p + vec2(-1.0, 2.0), p + vec2(6.0, 2.0)], stroke(c));
    g.add(Shape::line(
        vec![p + vec2(3.5, -0.5), p + vec2(6.5, 2.0), p + vec2(3.5, 4.5)],
        stroke(c),
    ));
}

fn draw_pointer(g: &Painter, at: Pos2, c: Color32) {
    let pts = vec![
        at + vec2(-3.0, -7.0),
        at + vec2(5.0, 1.0),
        at + vec2(1.0, 1.5),
        at + vec2(-1.0, 6.0),
        at + vec2(-3.0, -7.0),
    ];
    g.add(Shape::line(pts, stroke(c)));
}

fn draw_crosshair(g: &Painter, at: Pos2, c: Color32) {
    g.line_segment([at + vec2(-6.0, 0.0), at + vec2(6.0, 0.0)], stroke(c));
    g.line_segment([at + vec2(0.0, -6.0), at + vec2(0.0, 6.0)], stroke(c));
}

fn draw_zoom_icon(g: &Painter, at: Pos2, c: Color32, vertical: bool) {
    let (a, b) = if vertical {
        (vec2(0.0, -6.0), vec2(0.0, 6.0))
    } else {
        (vec2(-6.0, 0.0), vec2(6.0, 0.0))
    };
    g.line_segment([at + a, at + b], stroke(c));
    let n = if vertical {
        vec2(2.5, 0.0)
    } else {
        vec2(0.0, 2.5)
    };
    let d = if vertical {
        vec2(0.0, 2.5)
    } else {
        vec2(2.5, 0.0)
    };
    g.add(Shape::line(
        vec![at + a + n + d, at + a, at + a - n + d],
        stroke(c),
    ));
    g.add(Shape::line(
        vec![at + b + n - d, at + b, at + b - n - d],
        stroke(c),
    ));
}

fn shortcut(ui: &Ui, mods: Modifiers, key: Key) -> String {
    ui.ctx().format_shortcut(&KeyboardShortcut::new(mods, key))
}

// ------------------------------------------------------------------ toolbar

fn toolbar(ui: &Ui, p: &mut Project, s: &mut UiState, bar: Rect, lanes_w: f32) {
    let g = ui.painter_at(bar);
    g.rect_filled(bar, 0.0, TOOLBAR_BG);
    g.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(0x24)),
    );
    let cy = bar.center().y;
    let v = &mut s.tracks;

    // Back chevron (dim: no previous view to return to).
    let c = pos2(bar.left() + 16.0, cy);
    g.add(Shape::line(
        vec![c + vec2(3.0, -5.0), c + vec2(-2.0, 0.0), c + vec2(3.0, 5.0)],
        stroke(TEXT_DIM),
    ));

    let mut x = bar.left() + 32.0;
    let sel = v.selected;
    let (edit, w) = menu_title(ui, &g, x, cy, "Edit", id(("menu", "Edit")));
    x += w + 2.0;
    egui::Popup::menu(&edit).show(|ui| {
        ui.set_min_width(260.0);
        todo_item(ui, "Undo");
        todo_item(ui, "Redo");
        ui.separator();
        todo_item(ui, "Cut");
        todo_item(ui, "Copy");
        todo_item(ui, "Paste");
        if ui
            .add_enabled(
                sel.is_some(),
                egui::Button::new("Delete").shortcut_text("Del"),
            )
            .clicked()
        {
            delete_selected(p, v);
            ui.close();
        }
        ui.separator();
        todo_item(ui, "Select All");
        todo_item(ui, "Split Regions at Playhead");
        todo_item(ui, "Join Regions");
        let rep = shortcut(ui, Modifiers::COMMAND, Key::R);
        if ui
            .add_enabled(
                sel.is_some(),
                egui::Button::new("Repeat Regions").shortcut_text(rep),
            )
            .clicked()
        {
            repeat_selected(p, v);
            ui.close();
        }
    });
    let (func, w) = menu_title(ui, &g, x, cy, "Functions", id(("menu", "Functions")));
    x += w + 2.0;
    egui::Popup::menu(&func).show(|ui| {
        ui.set_min_width(260.0);
        if ui
            .add_enabled(sel.is_some(), egui::Button::new("Set Locators by Regions"))
            .clicked()
        {
            set_locators_by_region(p, v);
            ui.close();
        }
        ui.separator();
        todo_item(ui, "Folder");
        todo_item(ui, "Quantize");
        todo_item(ui, "Bounce in Place…");
        todo_item(ui, "Convert to New Sampler Track…");
        todo_item(ui, "Move to Playhead");
    });
    let (view, w) = menu_title(ui, &g, x, cy, "View", id(("menu", "View")));
    x += w + 10.0;
    egui::Popup::menu(&view).show(|ui| {
        ui.set_min_width(260.0);
        if ui.button("Zoom to Fit").clicked() {
            zoom_to_fit(p, v, lanes_w);
            ui.close();
        }
        if ui.button("Zoom In").clicked() {
            v.bar_w = (v.bar_w * 1.25).min(600.0);
            ui.close();
        }
        if ui.button("Zoom Out").clicked() {
            v.bar_w = (v.bar_w / 1.25).max(6.0);
            ui.close();
        }
        ui.separator();
        if ui.checkbox(&mut v.catch, "Catch Playhead").clicked() {
            ui.close();
        }
        todo_item(ui, "Show Global Tracks");
        todo_item(ui, "Configure Track Header…");
    });

    // View buttons: grid (Live Loops) / list (Tracks, active).
    let r = Rect::from_center_size(pos2(x + 13.0, cy), vec2(26.0, 20.0));
    let _ = icon_button(ui, &g, r, "grid", false, draw_grid_icon);
    let r = Rect::from_center_size(pos2(x + 41.0, cy), vec2(26.0, 20.0));
    let _ = icon_button(ui, &g, r, "list", true, draw_list_icon);
    x += 66.0;
    let r = Rect::from_center_size(pos2(x + 13.0, cy), vec2(26.0, 20.0));
    let _ = icon_button(ui, &g, r, "automation", false, draw_curve_icon);
    let r = Rect::from_center_size(pos2(x + 41.0, cy), vec2(26.0, 20.0));
    let _ = icon_button(ui, &g, r, "flex", false, draw_flex_icon);
    let r = Rect::from_center_size(pos2(x + 69.0, cy), vec2(26.0, 20.0));
    if icon_button(ui, &g, r, "catch", v.catch, draw_catch_icon).clicked() {
        v.catch = !v.catch;
    }

    // Right-hand side, laid out from the right edge.
    // Horizontal zoom (log scale, 6..600 points per bar).
    let hz = Rect::from_min_size(pos2(bar.right() - 72.0, cy - 6.0), vec2(64.0, 12.0));
    let (lo, hi) = (6f32.ln(), 600f32.ln());
    let t = ((v.bar_w.ln() - lo) / (hi - lo)).clamp(0.0, 1.0);
    let t2 = mini_slider(ui, &g, hz, "hzoom", t);
    if t2 != t {
        v.bar_w = (lo + t2 * (hi - lo)).exp();
    }
    draw_zoom_icon(&g, pos2(hz.left() - 13.0, cy), TEXT, false);
    // Vertical zoom (track height 36..140).
    let vz = Rect::from_min_size(pos2(hz.left() - 26.0 - 54.0, cy - 6.0), vec2(54.0, 12.0));
    let t = ((v.row_h - 36.0) / (140.0 - 36.0)).clamp(0.0, 1.0);
    let t2 = mini_slider(ui, &g, vz, "vzoom", t);
    if t2 != t {
        v.row_h = 36.0 + t2 * (140.0 - 36.0);
    }
    draw_zoom_icon(&g, pos2(vz.left() - 13.0, cy), TEXT, true);
    // Waveform zoom / fit buttons (not implemented yet: drawn dim).
    let mut rx = vz.left() - 30.0;
    for i in 0..2 {
        let c = pos2(rx - 10.0 - i as f32 * 26.0, cy);
        g.line_segment(
            [c + vec2(-5.0, -6.0), c + vec2(-5.0, 6.0)],
            stroke(TEXT_DIM),
        );
        g.line_segment([c + vec2(5.0, -6.0), c + vec2(5.0, 6.0)], stroke(TEXT_DIM));
        if i == 0 {
            g.line_segment([c + vec2(-3.0, 0.0), c + vec2(3.0, 0.0)], stroke(TEXT_DIM));
        } else {
            g.line_segment([c + vec2(0.0, -4.0), c + vec2(0.0, 4.0)], stroke(TEXT_DIM));
        }
    }
    rx -= 62.0;

    let drag_r = Rect::from_min_size(pos2(rx - 110.0, cy - 10.0), vec2(110.0, 20.0));
    let resp = popup_button(ui, &g, drag_r, id(("popup", "drag")), v.drag_mode.label());
    egui::Popup::menu(&resp).show(|ui| {
        for m in DragMode::ALL {
            if ui.selectable_label(v.drag_mode == m, m.label()).clicked() {
                v.drag_mode = m;
                ui.close();
            }
        }
    });
    g.text(
        pos2(drag_r.left() - 6.0, cy),
        Align2::RIGHT_CENTER,
        "Drag:",
        FontId::proportional(12.5),
        TEXT_DIM,
    );
    rx = drag_r.left() - 50.0;

    let snap_r = Rect::from_min_size(pos2(rx - 150.0, cy - 10.0), vec2(150.0, 20.0));
    let resp = popup_button(ui, &g, snap_r, id(("popup", "snap")), v.snap.label());
    egui::Popup::menu(&resp).show(|ui| {
        for m in Snap::ALL {
            if ui.selectable_label(v.snap == m, m.label()).clicked() {
                v.snap = m;
                ui.close();
            }
        }
    });
    g.text(
        pos2(snap_r.left() - 6.0, cy),
        Align2::RIGHT_CENTER,
        "Snap:",
        FontId::proportional(12.5),
        TEXT_DIM,
    );
    rx = snap_r.left() - 54.0;

    // Left-click and Command-click tools (only the Pointer works so far).
    for (i, key) in ["tool_l", "tool_r"].iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(rx - 40.0 - (1 - i) as f32 * 44.0, cy - 10.0),
            vec2(40.0, 20.0),
        );
        let resp = ui.interact(r, id(*key), Sense::click());
        if resp.hovered() {
            g.rect_filled(r, 4.0, Color32::from_white_alpha(14));
        }
        let icon_at = pos2(r.left() + 13.0, cy);
        if i == 0 {
            draw_pointer(&g, icon_at, TEXT);
        } else {
            draw_crosshair(&g, icon_at, TEXT);
        }
        let c = pos2(r.right() - 9.0, cy + 1.0);
        g.add(Shape::line(
            vec![
                c + vec2(-3.0, -1.5),
                c + vec2(0.0, 1.5),
                c + vec2(3.0, -1.5),
            ],
            stroke(TEXT_DIM),
        ));
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(220.0);
            for (n, t) in TOOLS.iter().enumerate() {
                if n == 0 {
                    let _ = ui.selectable_label(true, *t);
                } else {
                    todo_item(ui, t);
                }
            }
        });
    }
}

// ------------------------------------------------------------------ actions

fn delete_selected(p: &mut Project, v: &mut TracksView) {
    if let Some((t, r)) = v.selected.take() {
        if let Some(track) = p.tracks.get_mut(t) {
            if r < track.regions.len() {
                track.regions.remove(r);
                p.song_rev += 1;
            }
        }
    }
}

fn repeat_selected(p: &mut Project, v: &mut TracksView) {
    let Some((t, r)) = v.selected else { return };
    let Some(track) = p.tracks.get_mut(t) else {
        return;
    };
    let Some(src) = track.regions.get(r).cloned() else {
        return;
    };
    let copy = Region {
        start_bar: src.start_bar + src.len_bars,
        ..src
    };
    track.regions.push(copy);
    v.selected = Some((t, track.regions.len() - 1));
    p.song_rev += 1;
}

fn set_locators_by_region(p: &mut Project, v: &TracksView) {
    let Some((t, r)) = v.selected else { return };
    let Some(reg) = p.tracks.get(t).and_then(|tr| tr.regions.get(r)) else {
        return;
    };
    let bpb = p.beats_per_bar();
    p.cycle_start = (reg.start_bar as f64 - 1.0) * bpb;
    p.cycle_end = p.cycle_start + reg.len_bars as f64 * bpb;
    p.cycle = true;
}

fn zoom_to_fit(p: &Project, v: &mut TracksView, lanes_w: f32) {
    let end = p
        .tracks
        .iter()
        .flat_map(|t| t.regions.iter())
        .map(|r| r.start_bar - 1.0 + r.len_bars)
        .fold(0.0f32, f32::max);
    if end > 0.0 {
        v.bar_w = ((lanes_w - 20.0) / end).clamp(6.0, 600.0);
        v.scroll_x = 0.0;
    }
}

// ------------------------------------------------------------------ drawing

#[allow(clippy::too_many_arguments)]
fn draw_region(
    g: &Painter,
    rr: Rect,
    full: Rect,
    track: &Track,
    region: &Region,
    beat_px: f32,
    selected: bool,
    silenced: bool,
    seed: u32,
) {
    // Muted regions are drawn grey, like Logic.
    let base = if region.params.mute {
        Color32::from_gray(0x6a)
    } else {
        track.color
    };
    let body = if silenced {
        // Another track is soloed: dark grey body, grey content, name in the track colour.
        Color32::from_rgb(0x2e, 0x2e, 0x30)
            .lerp_to_gamma(Color32::WHITE, if selected { 0.06 } else { 0.0 })
    } else if selected {
        base.lerp_to_gamma(Color32::WHITE, 0.18)
    } else {
        base
    };
    g.rect_filled(rr, 3.0, body);
    let head_h = 14.0f32.min(rr.height() * 0.4);
    let head = Rect::from_min_size(rr.min, vec2(rr.width(), head_h));
    g.rect_filled(
        head,
        egui::CornerRadius {
            nw: 3,
            ne: 3,
            sw: 0,
            se: 0,
        },
        body.lerp_to_gamma(Color32::WHITE, if silenced { 0.03 } else { 0.22 }),
    );
    let name_color = if silenced {
        base.lerp_to_gamma(Color32::WHITE, 0.15)
    } else {
        Color32::from_white_alpha(235)
    };
    g.text(
        head.left_center() + vec2(5.0, 0.0),
        Align2::LEFT_CENTER,
        &region.name,
        FontId::proportional(10.5),
        name_color,
    );
    let inner = Rect::from_min_max(
        pos2(rr.left(), head.bottom() + 2.0),
        rr.max - vec2(0.0, 2.0),
    );
    let ink = if silenced {
        Color32::from_gray(0x8c)
    } else {
        Color32::from_white_alpha(if selected { 235 } else { 205 })
    };
    if let Some(peaks) = region.audio.as_ref().and_then(|a| a.peaks.get()) {
        draw_peaks(g, inner, full.left(), full.width(), peaks, ink);
    } else if region.audio.is_some() {
        g.line_segment(
            [
                pos2(inner.left(), inner.center().y),
                pos2(inner.right(), inner.center().y),
            ],
            Stroke::new(1.0, ink),
        );
    } else if track.kind == TrackKind::Audio {
        fake_waveform(g, inner, ink, seed);
    } else if !region.notes.is_empty() {
        let lo = region.notes.iter().map(|n| n.pitch).min().unwrap_or(48) as f32;
        let hi = region.notes.iter().map(|n| n.pitch).max().unwrap_or(72) as f32;
        for n in &region.notes {
            let nx = full.left() + n.start * beat_px;
            if nx > inner.right() || nx + n.len * beat_px < inner.left() {
                continue;
            }
            let ny = inner.bottom()
                - 2.0
                - (n.pitch as f32 - lo) / (hi - lo).max(1.0) * (inner.height() - 4.0);
            let nr = Rect::from_min_max(
                pos2(nx.max(inner.left()), ny - 1.0),
                pos2(
                    (nx + (n.len * beat_px).max(2.0)).min(inner.right()),
                    ny + 1.0,
                ),
            );
            g.rect_filled(nr, 0.0, ink);
        }
    }
    let border = if selected {
        Color32::WHITE
    } else if silenced {
        Color32::from_gray(0x44)
    } else {
        Color32::from_black_alpha(110)
    };
    g.rect_stroke(rr, 3.0, Stroke::new(1.0, border), StrokeKind::Inside);
}

/// Demo audio tracks without a file get a stable pseudo-waveform.
fn fake_waveform(g: &Painter, rect: Rect, col: Color32, seed: u32) {
    let mut s = seed;
    let mid = rect.center().y;
    let mut x = rect.left();
    while x < rect.right() {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        let env = ((x * 0.03).sin().abs() * 0.6 + 0.3) * ((s >> 24) as f32 / 255.0 * 0.6 + 0.4);
        let h = env * rect.height() * 0.45;
        g.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(1.0, col));
        x += 2.0;
    }
}

/// Real waveform from min/max peaks. `x0`/`full_w` describe the unclipped region.
fn draw_peaks(g: &Painter, body: Rect, x0: f32, full_w: f32, peaks: &[(f32, f32)], col: Color32) {
    if peaks.is_empty() || full_w <= 0.0 {
        return;
    }
    let mid = body.center().y;
    let half = body.height() * 0.48;
    let mut x = body.left().floor();
    while x < body.right() {
        let a = (((x - x0) / full_w) * peaks.len() as f32).max(0.0) as usize;
        if a >= peaks.len() {
            break;
        }
        let b =
            ((((x + 1.0 - x0) / full_w) * peaks.len() as f32) as usize).clamp(a + 1, peaks.len());
        let (lo, hi) = peaks[a..b]
            .iter()
            .fold((0f32, 0f32), |(l, h), p| (l.min(p.0), h.max(p.1)));
        g.line_segment(
            [
                pos2(x, mid - hi.min(1.0) * half),
                pos2(x, mid - lo.max(-1.0) * half),
            ],
            Stroke::new(1.0, col),
        );
        x += 1.0;
    }
}

fn header(ui: &Ui, g: &Painter, p: &mut Project, i: usize, hdr: Rect) {
    let selected = p.selected_track == i;
    g.rect_filled(hdr, 0.0, if selected { HEADER_SEL } else { HEADER_BG });
    g.line_segment(
        [hdr.left_bottom(), hdr.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(0x2a)),
    );
    let num = Rect::from_min_size(hdr.min, vec2(NUM_W, hdr.height()));
    g.text(
        num.center(),
        Align2::CENTER_CENTER,
        (i + 1).to_string(),
        FontId::proportional(12.0),
        TEXT_DIM,
    );
    if ui.interact(hdr, id(("hdr", i)), Sense::click()).clicked() {
        p.selected_track = i;
    }

    let h = hdr.height();
    let icon_size = (h - 14.0).clamp(22.0, 40.0);
    let icon = Rect::from_min_size(
        pos2(hdr.left() + NUM_W + 6.0, hdr.center().y - icon_size / 2.0),
        vec2(icon_size, icon_size),
    );
    draw_icon(g, icon, track_icon(&p.tracks[i]));

    let name_x = icon.right() + 10.0;
    let compact = h < 46.0;
    let name_y = if compact {
        hdr.center().y
    } else {
        hdr.center().y - 9.0
    };
    let name_w = hdr.right() - 122.0 - name_x;
    let name_galley = g.layout_no_wrap(p.tracks[i].name.clone(), FontId::proportional(13.0), TEXT);
    let clip = g.with_clip_rect(
        Rect::from_min_size(pos2(name_x, hdr.top()), vec2(name_w.max(20.0), h))
            .intersect(g.clip_rect()),
    );
    clip.galley(
        pos2(name_x, name_y - name_galley.size().y / 2.0),
        name_galley,
        TEXT,
    );

    // M S R (I) buttons under the name (compact tracks: hidden, like Logic at small heights).
    let audio = p.tracks[i].kind == TrackKind::Audio;
    let t = &mut p.tracks[i];
    if !compact {
        let by = hdr.center().y + 3.0;
        let mut bx = name_x;
        let size = vec2(18.0, 15.0);
        if header_button(
            ui,
            g,
            Rect::from_min_size(pos2(bx, by), size),
            (i, "m"),
            "M",
            t.mute,
            MUTE_ON,
            selected,
        ) {
            t.mute = !t.mute;
        }
        bx += 21.0;
        if header_button(
            ui,
            g,
            Rect::from_min_size(pos2(bx, by), size),
            (i, "s"),
            "S",
            t.solo,
            SOLO_ON,
            selected,
        ) {
            t.solo = !t.solo;
        }
        bx += 21.0;
        // Aux and Folder tracks hold no regions, so there is nothing to record into.
        if t.kind.has_regions()
            && header_button(
                ui,
                g,
                Rect::from_min_size(pos2(bx, by), size),
                (i, "r"),
                "R",
                t.rec,
                REC_ON,
                selected,
            )
        {
            t.rec = !t.rec;
        }
        if audio {
            bx += 21.0;
            if header_button(
                ui,
                g,
                Rect::from_min_size(pos2(bx, by), size),
                (i, "i"),
                "I",
                t.monitor,
                MON_ON,
                selected,
            ) {
                t.monitor = !t.monitor;
            }
        }
    }

    // Volume slider (dB, -60..+6; double-click resets to 0 dB) and pan knob.
    let sl = Rect::from_min_size(
        pos2(hdr.right() - 112.0, hdr.center().y - 3.0),
        vec2(70.0, 6.0),
    );
    let resp = ui.interact(
        sl.expand2(vec2(4.0, 8.0)),
        id(("vol", i)),
        Sense::click_and_drag(),
    );
    if let Some(pt) = resp.interact_pointer_pos() {
        t.volume_db = -60.0 + ((pt.x - sl.left()) / sl.width()).clamp(0.0, 1.0) * 66.0;
    }
    if resp.double_clicked() {
        t.volume_db = 0.0;
    }
    let f = ((t.volume_db + 60.0) / 66.0).clamp(0.0, 1.0);
    g.rect_filled(sl, 3.0, Color32::from_gray(0x1e));
    let knob = pos2(sl.left() + sl.width() * f, sl.center().y);
    g.circle_filled(knob, 7.0, Color32::from_gray(0xc6));
    g.circle_stroke(knob, 7.0, Stroke::new(1.0, Color32::from_gray(0x7a)));

    let kc = pos2(hdr.right() - 22.0, hdr.center().y);
    let kresp = ui.interact(
        Rect::from_center_size(kc, vec2(22.0, 22.0)),
        id(("pan", i)),
        Sense::click_and_drag(),
    );
    if kresp.dragged() {
        t.pan = (t.pan - kresp.drag_delta().y * 0.01).clamp(-1.0, 1.0);
    }
    if kresp.double_clicked() {
        t.pan = 0.0;
    }
    crate::widgets::draw_knob(g, kc, 9.0, t.pan);
}

// ------------------------------------------------------------------ main view

pub fn show(ui: &mut Ui, p: &mut Project, s: &mut UiState) {
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let tb = Rect::from_min_size(area.min, vec2(area.width(), TOOLBAR_H));
    let ruler_y = tb.bottom();
    let headers = Rect::from_min_max(
        pos2(area.left(), ruler_y + RULER_H),
        pos2(area.left() + HEADER_W, area.bottom()),
    );
    let lanes = Rect::from_min_max(pos2(area.left() + HEADER_W, ruler_y + RULER_H), area.max);
    let ruler = Rect::from_min_max(
        pos2(lanes.left(), ruler_y),
        pos2(area.right(), ruler_y + RULER_H),
    );
    let corner = Rect::from_min_max(
        pos2(area.left(), ruler_y),
        pos2(headers.right(), ruler.bottom()),
    );

    toolbar(ui, p, s, tb, lanes.width());

    let bpb = p.beats_per_bar();
    let unit = p.beat_unit();
    let v = &mut s.tracks;

    // ---- wheel: scroll; Shift = horizontal; Ctrl/Cmd = horizontal zoom around the pointer
    let hover = ui.input(|i| i.pointer.hover_pos());
    if hover.is_some_and(|pt| lanes.contains(pt) || headers.contains(pt) || ruler.contains(pt)) {
        let (delta, mods) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers));
        if mods.command && delta.y != 0.0 {
            let pointer_x = hover.map_or(lanes.left(), |p| p.x);
            let beat_at = (pointer_x - lanes.left() + v.scroll_x) / (v.bar_w / bpb as f32);
            v.bar_w = (v.bar_w * (1.0 + delta.y * 0.003)).clamp(6.0, 600.0);
            v.scroll_x = beat_at * (v.bar_w / bpb as f32) - (pointer_x - lanes.left());
        } else if mods.shift {
            v.scroll_x -= delta.y + delta.x;
        } else {
            v.scroll_x -= delta.x;
            v.scroll_y -= delta.y;
        }
    }
    let rows = row_layout(p, v.row_h);
    let content_h = rows.last().map_or(0.0, |(top, h)| top + h);
    v.scroll_y = v
        .scroll_y
        .clamp(0.0, (content_h - lanes.height() + 40.0).max(0.0));
    v.scroll_x = v.scroll_x.clamp(0.0, 1000.0 * v.bar_w);

    let beat_px = v.bar_w / bpb as f32;
    // Catch: page the view when the playhead leaves it.
    if v.catch && p.playing {
        let px = p.position_beats as f32 * beat_px - v.scroll_x;
        if px > lanes.width() - 30.0 || px < 0.0 {
            v.scroll_x = (p.position_beats as f32 * beat_px - lanes.width() * 0.1).max(0.0);
        }
    }
    let (scroll_x, scroll_y) = (v.scroll_x, v.scroll_y);
    let x_of = |beat: f64| lanes.left() + beat as f32 * beat_px - scroll_x;
    let beat_of = |x: f32| ((x - lanes.left() + scroll_x) / beat_px).max(0.0) as f64;

    // ---- lanes background + grid
    let g = ui.painter_at(lanes);
    g.rect_filled(lanes, 0.0, LANE_BG);
    let first_bar = (scroll_x / v.bar_w).floor() as i64;
    let last_bar = ((scroll_x + lanes.width()) / v.bar_w).ceil() as i64;
    for (i, &(top, row_h)) in rows.iter().enumerate() {
        let y = lanes.top() + top - scroll_y;
        let row = Rect::from_min_size(pos2(lanes.left(), y), vec2(lanes.width(), row_h));
        if p.selected_track == i {
            g.rect_filled(row, 0.0, LANE_SEL);
        }
        g.line_segment(
            [row.left_bottom(), row.right_bottom()],
            Stroke::new(1.0, Color32::from_gray(0x22)),
        );
    }
    for b in first_bar..=last_bar {
        let x = x_of(b as f64 * bpb);
        g.line_segment(
            [pos2(x, lanes.top()), pos2(x, lanes.bottom())],
            Stroke::new(1.0, Color32::from_gray(0x38)),
        );
        if beat_px * unit as f32 >= 14.0 {
            for k in 1..p.sig.0 {
                let xb = x_of(b as f64 * bpb + k as f64 * unit);
                g.line_segment(
                    [pos2(xb, lanes.top()), pos2(xb, lanes.bottom())],
                    Stroke::new(1.0, Color32::from_gray(0x30)),
                );
            }
        }
    }

    // Click on an empty lane: select that track, deselect regions.
    let lane_resp = ui.interact(lanes, id("lanes"), Sense::click());
    if lane_resp.clicked() {
        if let Some(pt) = lane_resp.interact_pointer_pos() {
            if let Some(row) = row_at(&rows, pt.y - lanes.top() + scroll_y) {
                p.selected_track = row;
            }
        }
        v.selected = None;
    }

    // ---- regions
    let step = snap_step(v.snap, beat_px, bpb, unit);
    let mut moved = false;
    for (ti, &(top, row_h)) in rows.iter().enumerate() {
        let y = lanes.top() + top - scroll_y;
        if y + row_h < lanes.top() || y > lanes.bottom() {
            continue;
        }
        for ri in 0..p.tracks[ti].regions.len() {
            let start_beat = (p.tracks[ti].regions[ri].start_bar as f64 - 1.0) * bpb;
            let len_beats = p.tracks[ti].regions[ri].len_bars as f64 * bpb;
            let full = Rect::from_min_max(
                pos2(x_of(start_beat), y + 1.0),
                pos2(x_of(start_beat + len_beats), y + row_h - 1.0),
            );
            let rr = full.intersect(lanes);
            if rr.width() <= 0.0 || rr.height() <= 0.0 {
                continue;
            }
            let resp = ui.interact(rr, id(("region", ti, ri)), Sense::click_and_drag());
            if resp.clicked() || resp.drag_started() {
                v.selected = Some((ti, ri));
                p.selected_track = ti;
            }
            if resp.double_clicked() && p.tracks[ti].kind != TrackKind::Audio {
                s.bottom = Bottom::PianoRoll;
            }
            if resp.drag_started() {
                if let Some(pt) = resp.interact_pointer_pos() {
                    v.grab_beats = beat_of(pt.x) - start_beat;
                }
            }
            if resp.dragged() {
                if let Some(pt) = resp.interact_pointer_pos() {
                    let nb = snap_to((beat_of(pt.x) - v.grab_beats).max(0.0), step);
                    p.tracks[ti].regions[ri].start_bar = (nb / bpb) as f32 + 1.0;
                }
            }
            if resp.drag_stopped() {
                moved = true;
            }
            let selected = v.selected == Some((ti, ri));
            let tr = &p.tracks[ti];
            draw_region(
                &g,
                rr,
                full,
                tr,
                &tr.regions[ri],
                beat_px,
                selected,
                silenced_by_solo(p, ti),
                ti as u32 * 77 + ri as u32 * 13 + 3,
            );
        }
    }
    if moved {
        p.song_rev += 1;
    }

    // ---- keyboard: Delete, Cmd/Ctrl+R
    if !ui.ctx().egui_wants_keyboard_input() && !s.piano_focus {
        let (del, rep) = ui.input(|i| {
            (
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.modifiers.command && i.key_pressed(Key::R),
            )
        });
        if del {
            delete_selected(p, v);
        }
        if rep {
            repeat_selected(p, v);
        }
    }

    // ---- headers
    let gh = ui.painter_at(headers);
    gh.rect_filled(headers, 0.0, Color32::from_gray(0x3a));
    for (i, &(top, row_h)) in rows.iter().enumerate() {
        let y = headers.top() + top - scroll_y;
        if y + row_h < headers.top() || y > headers.bottom() {
            continue;
        }
        let hdr = Rect::from_min_size(pos2(headers.left(), y), vec2(HEADER_W, row_h));
        header(ui, &gh, p, i, hdr);
    }

    // ---- individual track height: drag the lower edge of a track (header or lane), as in
    // Logic. Registered after headers and regions so the edge wins over them.
    let edges = Rect::from_min_max(pos2(headers.left(), lanes.top()), lanes.right_bottom());
    for (i, &(top, row_h)) in rows.iter().enumerate() {
        let bottom = lanes.top() + top + row_h - scroll_y;
        if bottom < edges.top() || bottom > edges.bottom() {
            continue;
        }
        let edge = Rect::from_min_max(
            pos2(edges.left(), bottom - RESIZE_GRIP),
            pos2(edges.right(), bottom + RESIZE_GRIP),
        )
        .intersect(edges);
        let resp = ui
            .interact(edge, id(("track_edge", i)), Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeVertical);
        if resp.dragged() {
            if let (Some(pt), Some(t)) = (resp.interact_pointer_pos(), p.tracks.get_mut(i)) {
                let wanted = pt.y - (bottom - row_h);
                t.height = (wanted / v.row_h).clamp(Track::MIN_HEIGHT, Track::MAX_HEIGHT);
            }
        }
        if resp.double_clicked() {
            if let Some(t) = p.tracks.get_mut(i) {
                t.height = 1.0;
            }
        }
    }
    gh.line_segment(
        [headers.right_top(), headers.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(0x22)),
    );

    // ---- corner above the headers: "+" (new track), duplicate track, global tracks
    let gc = ui.painter_at(corner);
    gc.rect_filled(corner, 0.0, Color32::from_gray(0x3a));
    gc.line_segment(
        [corner.left_bottom(), corner.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(0x24)),
    );
    let plus = Rect::from_center_size(
        pos2(corner.left() + 18.0, corner.center().y),
        vec2(24.0, 18.0),
    );
    let pr = ui.interact(plus, id("plus"), Sense::click());
    gc.rect_filled(
        plus,
        4.0,
        if pr.hovered() {
            Color32::from_gray(0x66)
        } else {
            BTN_BG
        },
    );
    gc.line_segment(
        [
            plus.center() + vec2(-5.0, 0.0),
            plus.center() + vec2(5.0, 0.0),
        ],
        stroke(TEXT),
    );
    gc.line_segment(
        [
            plus.center() + vec2(0.0, -5.0),
            plus.center() + vec2(0.0, 5.0),
        ],
        stroke(TEXT),
    );
    if pr.clicked() {
        p.add_track(TrackKind::Instrument);
    }
    let dup = Rect::from_center_size(
        pos2(plus.right() + 18.0, corner.center().y),
        vec2(24.0, 18.0),
    );
    let dr = ui.interact(dup, id("dup"), Sense::click());
    let dup_bg = if dr.hovered() {
        Color32::from_gray(0x66)
    } else {
        BTN_BG
    };
    gc.rect_filled(dup, 4.0, dup_bg);
    gc.rect_stroke(
        Rect::from_center_size(dup.center() + vec2(1.5, -1.5), vec2(8.0, 7.0)),
        1.0,
        stroke(TEXT),
        StrokeKind::Middle,
    );
    gc.rect_filled(
        Rect::from_center_size(dup.center() + vec2(-1.5, 1.5), vec2(8.0, 7.0)),
        1.0,
        dup_bg,
    );
    gc.rect_stroke(
        Rect::from_center_size(dup.center() + vec2(-1.5, 1.5), vec2(8.0, 7.0)),
        1.0,
        stroke(TEXT),
        StrokeKind::Middle,
    );
    if dr.clicked() {
        if let Some(t) = p.tracks.get(p.selected_track).cloned() {
            p.tracks.insert(p.selected_track + 1, t);
            p.selected_track += 1;
            p.song_rev += 1;
        }
    }
    let gt = Rect::from_center_size(
        pos2(corner.right() - 18.0, corner.center().y),
        vec2(24.0, 18.0),
    );
    gc.rect_stroke(
        Rect::from_center_size(gt.center(), vec2(14.0, 10.0)),
        1.5,
        stroke(TEXT_DIM),
        StrokeKind::Middle,
    );
    gc.line_segment(
        [
            gt.center() + vec2(-7.0, -1.5),
            gt.center() + vec2(7.0, -1.5),
        ],
        stroke(TEXT_DIM),
    );

    // ---- ruler
    let gr = ui.painter_at(ruler);
    gr.rect_filled(ruler, 0.0, RULER_BG);
    gr.line_segment(
        [ruler.left_bottom(), ruler.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(0x24)),
    );
    if p.cycle {
        let band = Rect::from_min_max(
            pos2(x_of(p.cycle_start), ruler.top() + 2.0),
            pos2(x_of(p.cycle_end), ruler.bottom() - 1.0),
        );
        gr.rect_filled(band, 2.0, CYCLE);
    }
    let label_every = [1i64, 2, 4, 8, 16, 32]
        .into_iter()
        .find(|n| *n as f32 * v.bar_w >= 34.0)
        .unwrap_or(64);
    for b in first_bar..=last_bar {
        let x = x_of(b as f64 * bpb);
        let in_cycle =
            p.cycle && (b as f64 * bpb) >= p.cycle_start && (b as f64 * bpb) < p.cycle_end;
        let col = if in_cycle {
            Color32::from_gray(0x2a)
        } else {
            TEXT
        };
        if b % label_every == 0 {
            gr.line_segment(
                [pos2(x, ruler.top() + 4.0), pos2(x, ruler.bottom())],
                Stroke::new(1.0, col.gamma_multiply(0.7)),
            );
            gr.text(
                pos2(x + 3.0, ruler.top() + 4.0),
                Align2::LEFT_TOP,
                (b + 1).to_string(),
                FontId::proportional(11.5),
                col,
            );
        } else {
            gr.line_segment(
                [pos2(x, ruler.bottom() - 7.0), pos2(x, ruler.bottom())],
                Stroke::new(1.0, col.gamma_multiply(0.6)),
            );
        }
        if beat_px * unit as f32 >= 10.0 {
            for k in 1..p.sig.0 {
                let xb = x_of(b as f64 * bpb + k as f64 * unit);
                gr.line_segment(
                    [pos2(xb, ruler.bottom() - 4.0), pos2(xb, ruler.bottom())],
                    Stroke::new(1.0, col.gamma_multiply(0.45)),
                );
            }
        }
    }
    // Upper half: click toggles cycle, drag sets it (snapped to bars). Lower half: locate.
    let upper = Rect::from_min_max(ruler.min, pos2(ruler.right(), ruler.center().y));
    let lower = Rect::from_min_max(pos2(ruler.left(), ruler.center().y), ruler.max);
    let up = ui.interact(upper, id("ruler_cycle"), Sense::click_and_drag());
    if up.drag_started() {
        v.cycle_anchor = up
            .interact_pointer_pos()
            .map(|pt| snap_to(beat_of(pt.x), bpb));
    }
    if let (Some(anchor), true, Some(pt)) =
        (v.cycle_anchor, up.dragged(), up.interact_pointer_pos())
    {
        let other = snap_to(beat_of(pt.x), bpb);
        if (other - anchor).abs() >= bpb {
            p.cycle_start = anchor.min(other);
            p.cycle_end = anchor.max(other);
            p.cycle = true;
        }
    }
    if up.drag_stopped() {
        v.cycle_anchor = None;
    }
    if up.clicked() {
        p.cycle = !p.cycle;
    }
    let low = ui.interact(lower, id("ruler_locate"), Sense::click_and_drag());
    if let Some(pt) = low.interact_pointer_pos() {
        if low.clicked() || low.dragged() {
            p.position_beats = beat_of(pt.x);
        }
    }

    // ---- playhead
    let px = x_of(p.position_beats);
    // Grab the playhead's head in the ruler and drag it anywhere in the song (back to the
    // start, past the end of the view, ...). Registered after the ruler so the head wins over
    // the cycle area; kept alive while dragging even when the head leaves the view.
    let head_id = id("playhead_head");
    let visible = px >= lanes.left() && px <= lanes.right();
    if visible || ui.ctx().is_being_dragged(head_id) {
        let grab = Rect::from_min_max(
            pos2(px - PLAYHEAD_GRAB, ruler.top()),
            pos2(px + PLAYHEAD_GRAB, ruler.bottom()),
        );
        let resp = ui
            .interact(grab, head_id, Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if resp.dragged() {
            if let Some(pt) = resp.interact_pointer_pos() {
                p.position_beats = playhead_beat(pt.x, lanes.left(), scroll_x, beat_px);
            }
        }
    }
    if visible {
        let gp = ui.painter_at(Rect::from_min_max(ruler.min, area.max));
        gp.line_segment(
            [pos2(px, ruler.top() + 6.0), pos2(px, area.bottom())],
            Stroke::new(1.0, Color32::from_gray(0xd8)),
        );
        let head = vec![
            pos2(px - 6.0, ruler.top() + 2.0),
            pos2(px + 6.0, ruler.top() + 2.0),
            pos2(px + 6.0, ruler.top() + 9.0),
            pos2(px, ruler.top() + 15.0),
            pos2(px - 6.0, ruler.top() + 9.0),
        ];
        gp.add(Shape::convex_polygon(
            head,
            Color32::from_gray(0xc8),
            Stroke::new(1.0, Color32::from_gray(0x60)),
        ));
    }
}

#[cfg(test)]
mod row_tests {
    use super::*;

    #[test]
    fn rows_follow_each_tracks_height() {
        let mut p = Project::demo();
        p.tracks.truncate(3);
        p.tracks[1].height = 2.0;
        p.tracks[2].height = 0.5;
        let rows = row_layout(&p, 56.0);
        assert_eq!(rows, vec![(0.0, 56.0), (56.0, 112.0), (168.0, 28.0)]);
        assert_eq!(row_at(&rows, 0.0), Some(0));
        assert_eq!(row_at(&rows, 167.9), Some(1));
        assert_eq!(row_at(&rows, 168.0), Some(2));
        assert_eq!(row_at(&rows, 196.0), None);
        assert_eq!(row_at(&rows, -1.0), None);
    }

    #[test]
    fn dragging_the_playhead_follows_the_pointer_and_stops_at_the_start() {
        // Lanes start at x = 100, scrolled by 50 px, 20 px per beat.
        assert_eq!(playhead_beat(150.0, 100.0, 50.0, 20.0), 5.0);
        assert_eq!(playhead_beat(100.0, 100.0, 0.0, 20.0), 0.0);
        // Dragged left of the lanes: pinned to the beginning of the song.
        assert_eq!(playhead_beat(20.0, 100.0, 0.0, 20.0), 0.0);
    }

    #[test]
    fn solo_silences_every_other_track_until_released() {
        let mut p = Project::demo();
        assert!((0..p.tracks.len()).all(|i| !silenced_by_solo(&p, i)));
        p.tracks[2].solo = true;
        assert!(!silenced_by_solo(&p, 2));
        assert!((0..p.tracks.len())
            .filter(|&i| i != 2)
            .all(|i| silenced_by_solo(&p, i)));
        // Soloing a second track adds it to what plays.
        p.tracks[4].solo = true;
        assert!(!silenced_by_solo(&p, 4) && silenced_by_solo(&p, 0));
        p.tracks[2].solo = false;
        p.tracks[4].solo = false;
        assert!((0..p.tracks.len()).all(|i| !silenced_by_solo(&p, i)));
    }

    #[test]
    fn beat_snap_follows_the_signature_beat() {
        // 4/4: a beat is a quarter note, as before.
        assert_eq!(snap_step(Snap::Beat, 20.0, 4.0, 1.0), 1.0);
        assert_eq!(snap_step(Snap::Smart, 20.0, 4.0, 1.0), 1.0);
        assert_eq!(snap_step(Snap::Bar, 20.0, 4.0, 1.0), 4.0);
        // 6/8: a beat is an eighth (0.5 quarter notes), a bar 3 quarter notes.
        assert_eq!(snap_step(Snap::Beat, 20.0, 3.0, 0.5), 0.5);
        assert_eq!(snap_step(Snap::Bar, 20.0, 3.0, 0.5), 3.0);
        // Smart snaps to eighths once an eighth is at least 12 px wide, else to bars.
        assert_eq!(snap_step(Snap::Smart, 24.0, 3.0, 0.5), 0.5);
        assert_eq!(snap_step(Snap::Smart, 20.0, 3.0, 0.5), 3.0);
        // Divisions are 16th notes in any signature.
        assert_eq!(snap_step(Snap::Division, 20.0, 3.0, 0.5), 0.25);
        assert_eq!(snap_step(Snap::Smart, 60.0, 3.0, 0.5), 0.25);
    }
}
