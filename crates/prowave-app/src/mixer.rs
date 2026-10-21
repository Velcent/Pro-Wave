//! Mixer (Logic Pro: Mixer in the main window): local toolbar, row labels and channel strips
//! with Setting, Gain Reduction, EQ, Input, Audio FX, Sends, Output, Group, Automation, Pan,
//! dB readouts, fader with scale, stereo meter, M/S and the coloured name label.
//!
//! Works today: fader, pan, mute, solo (all reach the engine), real post-fader meters with
//! peak hold, strip selection, Single/Tracks/All, the strip-type filters, scrolling.
//! Plug-in slots, sends and groups are drawn but not functional yet.
//!
//! [`channel_strip`] is also used by the Inspector's two strips.
use crate::i18n::{tr, trf};
use crate::model::{Project, TrackKind};
use crate::widgets::{
    back_chevron, menu_title, popup_button, stroke, todo_item, BLUE, TEXT, TEXT_DIM,
};
use crate::UiState;
use egui::{
    pos2, vec2, Align2, Color32, FontId, Painter, Rect, Sense, Shape, Stroke, StrokeKind, Ui,
};
use std::collections::HashMap;

const TOOLBAR_H: f32 = 30.0;
const LABEL_W: f32 = 88.0;
const GAP: f32 = 2.0;

const TOOLBAR_BG: Color32 = Color32::from_rgb(0x3c, 0x3c, 0x3c);
const MIXER_BG: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
const STRIP_BG: Color32 = Color32::from_rgb(0x3e, 0x3e, 0x3e);
const STRIP_SEL: Color32 = Color32::from_rgb(0x4d, 0x4d, 0x4d);
const SLOT_EMPTY: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2c);
const SLOT_GRAY: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
const PLUGIN_BLUE: Color32 = Color32::from_rgb(0x36, 0x6f, 0xd2);
/// A strip that makes no sound (muted, or another track is soloed): instruments, plug-ins
/// and sends turn grey, and so do the meters.
const PLUGIN_OFF: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x4a);
const METER_OFF: Color32 = Color32::from_rgb(0x70, 0x70, 0x70);
const READ_GREEN: Color32 = Color32::from_rgb(0x6c, 0xd0, 0x5a);
const MUTE_ON: Color32 = Color32::from_rgb(0x3b, 0x8c, 0xe6);
const SOLO_ON: Color32 = Color32::from_rgb(0xe6, 0xc2, 0x2e);
const OUT_PURPLE: Color32 = Color32::from_rgb(0x8a, 0x3f, 0xc8);
const MASTER_PINK: Color32 = Color32::from_rgb(0xb0, 0x3c, 0x9a);

/// Fader/meter scale marks in dB, top to bottom (Logic's channel strip scale).
const SCALE: [f32; 11] = [
    6.0, 3.0, 0.0, -3.0, -6.0, -10.0, -15.0, -20.0, -30.0, -40.0, -60.0,
];
pub(crate) const FILTERS: [&str; 8] = [
    "Audio",
    "Inst",
    "Aux",
    "Bus",
    "Input",
    "Output",
    "Master/VCA",
    "MIDI",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MixerMode {
    Single,
    Tracks,
    All,
}

pub struct MixerView {
    pub mode: MixerMode,
    /// Visibility per strip type, in `FILTERS` order.
    pub filters: [bool; 8],
    pub wide: bool,
    pub scroll_x: f32,
    /// Peak hold per strip (linear), shared with the Inspector strips.
    pub peak_hold: HashMap<StripKey, f32>,
}

impl Default for MixerView {
    fn default() -> Self {
        Self {
            mode: MixerMode::Tracks,
            filters: [true; 8],
            wide: false,
            scroll_x: 0.0,
            peak_hold: HashMap::new(),
        }
    }
}

/// Live meter values handed in by the app each frame.
#[derive(Default)]
pub struct Meters {
    pub tracks: Vec<(f32, f32)>,
    pub master: (f32, f32),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StripKey {
    Track(usize),
    StereoOut,
    Master,
}

fn id(ns: &'static str, key: impl std::hash::Hash + std::fmt::Debug) -> egui::Id {
    egui::Id::unique((ns, key))
}

/// 0 (bottom, -60 dB and below) .. 1 (top, +6 dB).
fn db_to_frac(db: f32) -> f32 {
    if db >= SCALE[0] {
        return 1.0;
    }
    let n = SCALE.len() - 1;
    for i in 0..n {
        let (hi, lo) = (SCALE[i], SCALE[i + 1]);
        if db <= hi && db >= lo {
            let idx = i as f32 + (hi - db) / (hi - lo);
            return 1.0 - idx / n as f32;
        }
    }
    0.0
}

fn frac_to_db(f: f32) -> f32 {
    let n = SCALE.len() - 1;
    let idx = ((1.0 - f.clamp(0.0, 1.0)) * n as f32).min(n as f32 - 1e-4);
    let i = idx.floor() as usize;
    SCALE[i] + (SCALE[i + 1] - SCALE[i]) * (idx - i as f32)
}

pub(crate) fn lin_to_db(x: f32) -> f32 {
    if x <= 1e-6 {
        -120.0
    } else {
        20.0 * x.log10()
    }
}

/// Shortens `text` with a trailing "…" so it fits in `max_w` (Logic truncates strip names).
pub fn elide(g: &Painter, text: &str, font: &FontId, max_w: f32) -> String {
    let width = |t: &str| {
        g.layout_no_wrap(t.to_string(), font.clone(), Color32::WHITE)
            .size()
            .x
    };
    if width(text) <= max_w {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect::<String>() + "…";
        if width(&candidate) <= max_w {
            return candidate;
        }
    }
    String::new()
}

fn slot(g: &Painter, r: Rect, text: &str, fill: Color32, color: Color32) {
    g.rect_filled(r, 3.0, fill);
    if !text.is_empty() {
        let font = FontId::proportional(10.5);
        let shown = elide(g, text, &font, r.width() - 6.0);
        g.text(r.center(), Align2::CENTER_CENTER, shown, font, color);
    }
}

fn small_button(
    ui: &Ui,
    g: &Painter,
    r: Rect,
    id: egui::Id,
    label: &str,
    on: bool,
    on_col: Color32,
) -> bool {
    let resp = ui.interact(r, id, Sense::click());
    g.rect_filled(r, 3.0, if on { on_col } else { SLOT_GRAY });
    let txt = if on && on_col == SOLO_ON {
        Color32::BLACK
    } else {
        Color32::from_gray(0xe8)
    };
    g.text(
        r.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.5),
        txt,
    );
    resp.clicked()
}

fn eq_thumb(g: &Painter, r: Rect, active: bool) {
    g.rect_filled(r, 3.0, Color32::from_gray(0x24));
    g.rect_stroke(
        r,
        3.0,
        Stroke::new(1.0, Color32::from_gray(0x1a)),
        StrokeKind::Inside,
    );
    if active {
        let pts: Vec<_> = (0..=24)
            .map(|i| {
                let t = i as f32 / 24.0;
                let y =
                    (t * 6.0 - 1.5).sin() * 0.25 + (-(t - 0.75) * (t - 0.75) * 40.0).exp() * 0.35;
                pos2(
                    r.left() + 3.0 + t * (r.width() - 6.0),
                    r.center().y - y * r.height(),
                )
            })
            .collect();
        g.add(Shape::line(
            pts,
            Stroke::new(1.2, Color32::from_rgb(0x8a, 0xb8, 0xf0)),
        ));
    } else {
        g.text(
            r.center(),
            Align2::CENTER_CENTER,
            "EQ",
            FontId::proportional(10.0),
            TEXT_DIM,
        );
    }
}

fn pan_knob(ui: &Ui, g: &Painter, c: egui::Pos2, id: egui::Id, pan: Option<&mut f32>) {
    let r = Rect::from_center_size(c, vec2(26.0, 26.0));
    let value = match pan {
        Some(p) => {
            let resp = ui.interact(r, id, Sense::click_and_drag());
            if resp.dragged() {
                *p = (*p - resp.drag_delta().y * 0.01).clamp(-1.0, 1.0);
            }
            if resp.double_clicked() {
                *p = 0.0;
            }
            *p
        }
        None => 0.0,
    };
    crate::widgets::draw_knob(g, c, 12.0, value);
}

/// Logic-style meter colour for a level in dB: green, yellow from -6 dB, red above 0 dB.
pub(crate) fn meter_colour(db: f32) -> Color32 {
    if db > 0.0 {
        Color32::from_rgb(0xe8, 0x3a, 0x2c)
    } else if db > -6.0 {
        Color32::from_rgb(0xe6, 0xc8, 0x2a)
    } else {
        Color32::from_rgb(0x34, 0xc2, 0x3e)
    }
}

/// Fader (scale + slot + cap) and stereo meter with peak-hold lines, laid out like Logic's
/// channel strip: fader scale on the left, the cap, the meter scale numbers, then L/R bars.
/// Returns the new fader value in dB when it was dragged or double-clicked.
#[allow(clippy::too_many_arguments)]
fn fader_and_meter(
    ui: &Ui,
    g: &Painter,
    area: Rect,
    id: egui::Id,
    db: f32,
    editable: bool,
    peaks: (f32, f32),
    inactive: bool,
) -> Option<f32> {
    let top = area.top() + 8.0;
    let bottom = area.bottom() - 6.0;
    let h = (bottom - top).max(1.0);
    let y_of = |f: f32| bottom - f * h;
    let w = area.width();
    let scale_col = Color32::from_gray(0x8e);
    let tiny = FontId::proportional(6.5);

    // Columns, proportional to the strip width (Logic: fader left of centre, meter right).
    let fader_x = area.left() + (w * 0.30).max(22.0);
    let num_x = area.left() + w * 0.56;
    let bar_w = 5.0;
    let meter_left = (area.left() + w * 0.66).min(area.right() - 2.0 * bar_w - 6.0);

    // Fader scale: tick marks with small numbers, left of the slot.
    for mark in SCALE {
        let y = y_of(db_to_frac(mark));
        let tick_len = if mark == 0.0 { 6.0 } else { 4.0 };
        let tx = fader_x - 13.0;
        g.line_segment(
            [pos2(tx - tick_len, y), pos2(tx, y)],
            Stroke::new(1.0, scale_col),
        );
        if w >= 70.0 {
            g.text(
                pos2(tx - tick_len - 2.0, y),
                Align2::RIGHT_CENTER,
                format!("{}", mark.abs() as i32),
                tiny.clone(),
                scale_col,
            );
        }
        // Meter scale numbers between the fader and the meter.
        g.text(
            pos2(num_x, y),
            Align2::CENTER_CENTER,
            format!("{}", mark.abs() as i32),
            tiny.clone(),
            scale_col,
        );
    }

    // Fader slot.
    let slot_r = Rect::from_min_max(pos2(fader_x - 2.0, top), pos2(fader_x + 2.0, bottom));
    g.rect_filled(slot_r, 2.0, Color32::from_gray(0x14));
    g.line_segment(
        [slot_r.right_top(), slot_r.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(0x50)),
    );

    // Meter bars with peak-hold lines (hold falls back at 12 dB/s).
    let hold_id = id.with("hold");
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    let mut hold: (f32, f32) = ui
        .ctx()
        .data(|d| d.get_temp(hold_id).unwrap_or((-120.0, -120.0)));
    let levels = (lin_to_db(peaks.0), lin_to_db(peaks.1));
    hold.0 = levels.0.max(hold.0 - 12.0 * dt);
    hold.1 = levels.1.max(hold.1 - 12.0 * dt);
    ui.ctx().data_mut(|d| d.insert_temp(hold_id, hold));
    // One dark housing holds both bars (Logic draws L and R side by side in one well).
    let housing = Rect::from_min_max(
        pos2(meter_left - 2.0, top - 1.0),
        pos2(meter_left + 2.0 * bar_w + 3.0, bottom + 1.0),
    );
    g.rect_filled(housing, 2.5, Color32::from_gray(0x10));
    g.rect_stroke(
        housing,
        2.5,
        Stroke::new(1.0, Color32::from_gray(0x2e)),
        StrokeKind::Inside,
    );
    for (i, (lvl, hd)) in [(levels.0, hold.0), (levels.1, hold.1)]
        .into_iter()
        .enumerate()
    {
        let x0 = meter_left + i as f32 * (bar_w + 1.0);
        let bar = Rect::from_min_max(pos2(x0, top), pos2(x0 + bar_w, bottom));
        g.rect_filled(bar, 0.5, Color32::from_gray(0x08));
        let f = db_to_frac(lvl);
        if f > 0.0 {
            let bands = [(-120.0f32, -6.0f32), (-6.0, 0.0), (0.0, 12.0)];
            for (lo, hi) in bands {
                let (fa, fb) = (db_to_frac(lo), db_to_frac(hi).min(f));
                if fb > fa {
                    let seg =
                        Rect::from_min_max(pos2(bar.left(), y_of(fb)), pos2(bar.right(), y_of(fa)));
                    let col = if inactive {
                        METER_OFF
                    } else {
                        meter_colour((lo + hi) * 0.5)
                    };
                    g.rect_filled(seg, 0.0, col);
                    // Lit left edge, like the LED column in Logic.
                    g.rect_filled(
                        Rect::from_min_max(seg.min, pos2(seg.left() + bar_w * 0.4, seg.bottom())),
                        0.0,
                        col.lerp_to_gamma(Color32::WHITE, 0.28),
                    );
                }
            }
            // Thin dark gaps every few pixels make the bar look segmented.
            let mut y = bottom - 2.0;
            while y > y_of(f) {
                g.line_segment(
                    [pos2(bar.left(), y), pos2(bar.right(), y)],
                    Stroke::new(0.8, Color32::from_black_alpha(110)),
                );
                y -= 2.5;
            }
        }
        if hd > -60.0 {
            let y = y_of(db_to_frac(hd));
            g.line_segment(
                [pos2(bar.left(), y), pos2(bar.right(), y)],
                Stroke::new(
                    1.5,
                    if inactive {
                        METER_OFF
                    } else {
                        meter_colour(hd)
                    }
                    .lerp_to_gamma(Color32::WHITE, 0.4),
                ),
            );
        }
    }
    if levels.0 > -60.0 || levels.1 > -60.0 || hold.0 > -60.0 || hold.1 > -60.0 {
        ui.ctx().request_repaint();
    }

    // Fader cap: light grey block with a dark centre groove and fine ridges.
    let cap_y = y_of(db_to_frac(db));
    let cap = Rect::from_center_size(pos2(fader_x, cap_y), vec2(16.0, 27.0));
    let mut out = None;
    if editable {
        let hit = Rect::from_min_max(
            pos2(fader_x - 14.0, top - 17.0),
            pos2(fader_x + 14.0, bottom + 17.0),
        );
        let resp = ui.interact(hit, id, Sense::click_and_drag());
        if resp.dragged() {
            let f = db_to_frac(db) - resp.drag_delta().y / h;
            out = Some(frac_to_db(f).clamp(-60.0, 6.0));
        }
        if resp.double_clicked() {
            out = Some(0.0);
        }
    }
    // Logic's cap: a narrow metal block. Soft shadow, silver body that is a little darker at the
    // edges, a dark groove across the middle with a bright line under it, and engraved ridges
    // (a dark line with a light line under it) above and below.
    g.rect_filled(
        cap.translate(vec2(0.5, 1.5)),
        2.0,
        Color32::from_black_alpha(110),
    );
    g.rect_filled(cap, 2.0, Color32::from_gray(0x9c));
    g.rect_filled(
        Rect::from_min_max(
            pos2(cap.left() + 1.5, cap.top() + 1.0),
            pos2(cap.right() - 1.5, cap.center().y),
        ),
        egui::CornerRadius {
            nw: 1,
            ne: 1,
            sw: 0,
            se: 0,
        },
        Color32::from_gray(0xd4),
    );
    g.rect_filled(
        Rect::from_min_max(
            pos2(cap.left() + 1.5, cap.center().y),
            pos2(cap.right() - 1.5, cap.bottom() - 1.0),
        ),
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: 1,
            se: 1,
        },
        Color32::from_gray(0xb2),
    );
    g.rect_stroke(
        cap,
        2.0,
        Stroke::new(1.0, Color32::from_gray(0x44)),
        StrokeKind::Inside,
    );
    for dy in [-9.0f32, -6.0, -3.0, 4.5, 7.5, 10.5] {
        g.line_segment(
            [
                pos2(cap.left() + 2.5, cap_y + dy),
                pos2(cap.right() - 2.5, cap_y + dy),
            ],
            Stroke::new(0.8, Color32::from_gray(0x80)),
        );
        g.line_segment(
            [
                pos2(cap.left() + 2.5, cap_y + dy + 1.0),
                pos2(cap.right() - 2.5, cap_y + dy + 1.0),
            ],
            Stroke::new(0.6, Color32::from_gray(0xe4)),
        );
    }
    g.line_segment(
        [
            pos2(cap.left() + 1.0, cap_y),
            pos2(cap.right() - 1.0, cap_y),
        ],
        Stroke::new(1.8, Color32::from_gray(0x24)),
    );
    g.line_segment(
        [
            pos2(cap.left() + 1.0, cap_y + 1.4),
            pos2(cap.right() - 1.0, cap_y + 1.4),
        ],
        Stroke::new(0.8, Color32::from_gray(0xee)),
    );
    out
}

/// Vertical positions of the strip rows (shared by every strip so rows line up).
pub struct Rows {
    setting: f32,
    gain_red: f32,
    eq: f32,
    input: f32,
    fx: f32,
    fx_rows: usize,
    sends: f32,
    output: f32,
    group: f32,
    automation: f32,
    pan: f32,
    db: f32,
    fader_top: f32,
}

pub fn rows(top: f32, fx_rows: usize) -> Rows {
    let fx_rows = fx_rows.max(2);
    let setting = top + 6.0;
    let gain_red = setting + 20.0;
    let eq = gain_red + 12.0;
    let input = eq + 28.0;
    let fx = input + 22.0;
    let sends = fx + fx_rows as f32 * 17.0 + 4.0;
    let output = sends + 20.0;
    let group = output + 20.0;
    let automation = group + 20.0;
    let pan = automation + 22.0;
    let db = pan + 30.0;
    Rows {
        setting,
        gain_red,
        eq,
        input,
        fx,
        fx_rows,
        sends,
        output,
        group,
        automation,
        pan,
        db,
        fader_top: db + 18.0,
    }
}

/// Audio FX rows needed so every strip in `keys` fits (inserts + one empty slot).
pub fn fx_rows_for(p: &Project, keys: &[StripKey]) -> usize {
    keys.iter()
        .map(|k| match k {
            StripKey::Track(i) => p.tracks.get(*i).map_or(0, |t| t.inserts.len()),
            StripKey::StereoOut => 2,
            StripKey::Master => 0,
        })
        .max()
        .unwrap_or(0)
        + 1
}

/// Logic's send slot: a blue "Bus N" button with a round level knob beside it. Click the slot
/// to pick a bus; the eye marks a pre-fader send. Nothing is routed in the engine yet.
fn send_slot(
    ui: &Ui,
    g: &Painter,
    r: Rect,
    send: &mut crate::model::SendSlot,
    uid: egui::Id,
    inactive: bool,
) {
    let Some(bus) = send.bus else {
        let resp = ui.interact(r, uid, Sense::click());
        slot(g, r, "", SLOT_EMPTY, TEXT);
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(110.0);
            for b in 1..=8u8 {
                if ui.button(trf("Bus {}", &[&b])).clicked() {
                    send.bus = Some(b);
                    ui.close();
                }
            }
        });
        return;
    };
    let knob_c = pos2(r.right() - 8.0, r.center().y);
    let slot_r = Rect::from_min_max(r.min, pos2(r.right() - 19.0, r.bottom()));
    let resp = ui.interact(slot_r, uid, Sense::click());
    slot(
        g,
        slot_r,
        &trf("Bus {}", &[&bus]),
        if inactive { PLUGIN_OFF } else { PLUGIN_BLUE },
        if inactive { TEXT_DIM } else { Color32::WHITE },
    );
    if send.pre {
        // The eye icon Logic shows on pre-fader sends.
        let c = pos2(slot_r.left() + 9.0, slot_r.center().y);
        g.circle_stroke(c, 4.5, Stroke::new(1.0, Color32::WHITE));
        g.circle_filled(c, 1.6, Color32::WHITE);
    }
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(130.0);
        for b in 1..=8u8 {
            if ui
                .selectable_label(send.bus == Some(b), trf("Bus {}", &[&b]))
                .clicked()
            {
                send.bus = Some(b);
                ui.close();
            }
        }
        ui.separator();
        if ui.selectable_label(send.pre, tr("Pre Fader")).clicked() {
            send.pre = !send.pre;
            ui.close();
        }
        if ui.button(tr("No Send")).clicked() {
            send.bus = None;
            ui.close();
        }
    });
    // Level knob: drag vertically; the indicator sweeps like a pan knob.
    let kresp = ui.interact(
        Rect::from_center_size(knob_c, vec2(16.0, 16.0)),
        uid.with("level"),
        Sense::click_and_drag(),
    );
    if kresp.dragged() {
        send.level = (send.level - kresp.drag_delta().y * 0.01).clamp(0.0, 1.0);
    }
    if kresp.double_clicked() {
        send.level = 0.0;
    }
    g.circle_filled(knob_c, 6.5, Color32::from_gray(0x5e));
    g.circle_stroke(knob_c, 6.5, Stroke::new(1.0, Color32::from_gray(0x26)));
    let a = -std::f32::consts::FRAC_PI_2 - 2.3 + send.level * 4.6;
    g.line_segment(
        [
            knob_c + vec2(a.cos(), a.sin()) * 2.0,
            knob_c + vec2(a.cos(), a.sin()) * 5.5,
        ],
        Stroke::new(1.4, Color32::WHITE),
    );
}

/// How the strip's name is shown at the bottom.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NameStyle {
    /// Coloured label (Mixer).
    Coloured,
    /// Plain text (Inspector).
    Plain,
}

/// Draws one channel strip in `strip` and applies its edits to the project.
/// `ns` keeps widget ids unique when the same strip is shown in two places.
#[allow(clippy::too_many_arguments)]
pub fn channel_strip(
    ui: &Ui,
    g: &Painter,
    strip: Rect,
    key: StripKey,
    p: &mut Project,
    peak_hold: &mut HashMap<StripKey, f32>,
    meters: &Meters,
    r: &Rows,
    ns: &'static str,
    name_style: NameStyle,
) {
    let selected = matches!(key, StripKey::Track(i) if i == p.selected_track);
    let inactive = matches!(key, StripKey::Track(i)
        if p.is_muted(i) || crate::tracks_area::silenced_by_solo(p, i));
    let (plug, plug_text) = if inactive {
        (PLUGIN_OFF, TEXT_DIM)
    } else {
        (PLUGIN_BLUE, Color32::WHITE)
    };
    // Whole-strip click selects (registered first so the controls on top win).
    if let StripKey::Track(i) = key {
        if ui
            .interact(strip, id(ns, (key, "strip")), Sense::click())
            .clicked()
        {
            p.selected_track = i;
        }
    }
    let bg = if selected && name_style == NameStyle::Coloured {
        STRIP_SEL
    } else {
        STRIP_BG
    };
    g.rect_filled(strip, 2.0, bg);
    let inner_w = strip.width() - 8.0;
    let sx = strip.left() + 4.0;
    let row = |y: f32, h: f32| Rect::from_min_size(pos2(sx, y), vec2(inner_w, h));

    let (name, color, inserts, kind, instrument) = match key {
        StripKey::Track(i) => {
            let t = &p.tracks[i];
            (
                t.name.clone(),
                t.color,
                t.inserts.clone(),
                Some(t.kind),
                t.instrument,
            )
        }
        StripKey::StereoOut => (
            tr("Stereo Out").to_string(),
            OUT_PURPLE,
            vec!["Compressor", "MultiMeter"],
            None,
            None,
        ),
        StripKey::Master => (tr("Master").to_string(), MASTER_PINK, vec![], None, None),
    };

    // A VCA strip is a fader with Mute and Solo: none of the channel strip's slots.
    let vca_strip = kind == Some(TrackKind::Vca);
    if vca_strip {
        g.text(
            pos2(strip.center().x, r.setting + 8.0),
            Align2::CENTER_CENTER,
            tr("VCA"),
            FontId::proportional(12.0),
            TEXT_DIM,
        );
    }
    if !vca_strip {
        slot(g, row(r.setting, 16.0), tr("Setting"), SLOT_GRAY, TEXT);
        g.rect_filled(row(r.gain_red + 1.0, 6.0), 1.0, SLOT_EMPTY);
        eq_thumb(g, row(r.eq, 24.0), inserts.iter().any(|x| x.contains("EQ")));
        let input_r = row(r.input, 16.0);
        match (kind, instrument) {
            (Some(TrackKind::Instrument | TrackKind::Drummer), Some(inst)) => {
                slot(g, input_r, inst, plug, plug_text)
            }
            // An aux strip's input is the bus it listens to: Aux 1 takes Bus 1, and so on. (The
            // engine does not route buses yet, so this is the strip's setting, not a signal path.)
            (Some(TrackKind::Aux), _) => {
                let n = match key {
                    StripKey::Track(i) => p.tracks[..=i]
                        .iter()
                        .filter(|t| t.kind == TrackKind::Aux)
                        .count(),
                    _ => 1,
                };
                slot(g, input_r, &trf("Bus {}", &[&n]), plug, plug_text);
            }
            // An external MIDI strip shows where its notes go: the port and the channel.
            (Some(TrackKind::ExternalMidi), _) => {
                let text = match key {
                    StripKey::Track(i) => {
                        let t = &p.tracks[i];
                        let port = t
                            .midi_port
                            .clone()
                            .unwrap_or_else(|| tr("No Port").to_string());
                        format!("{port} · {}", t.midi_channel)
                    }
                    _ => String::new(),
                };
                let fitted = elide(g, &text, &FontId::proportional(10.5), input_r.width() - 8.0);
                slot(g, input_r, &fitted, plug, plug_text);
            }
            (Some(TrackKind::Audio), _) => {
                slot(g, input_r, &trf("Input {}", &[&1]), SLOT_GRAY, TEXT);
                g.circle_stroke(
                    pos2(input_r.left() + 8.0, input_r.center().y),
                    3.5,
                    stroke(TEXT),
                );
            }
            _ => slot(g, input_r, "", SLOT_EMPTY, TEXT),
        }
        for k in 0..r.fx_rows {
            let rr = row(r.fx + k as f32 * 17.0, 15.0);
            match inserts.get(k) {
                Some(fx) => slot(g, rr, fx, plug, plug_text),
                None if k == inserts.len() => slot(g, rr, "", SLOT_EMPTY, TEXT),
                None => {}
            }
        }
        match key {
            StripKey::Track(i) => send_slot(
                ui,
                g,
                row(r.sends, 16.0),
                &mut p.tracks[i].send,
                id(ns, (key, "send")),
                inactive,
            ),
            StripKey::StereoOut => slot(g, row(r.sends, 16.0), "", SLOT_EMPTY, TEXT),
            StripKey::Master => {}
        }
        if let StripKey::Track(_) = key {
            let out = if name_style == NameStyle::Plain {
                tr("Stereo Out")
            } else {
                tr("St Out")
            };
            slot(g, row(r.output, 16.0), out, SLOT_GRAY, TEXT);
            if let StripKey::Track(i) = key {
                vca_slot(ui, g, row(r.group, 16.0), p, i, id(ns, (key, "vca")));
            }
        }
        if matches!(key, StripKey::StereoOut | StripKey::Master) {
            let rr = row(r.automation, 16.0);
            slot(
                g,
                rr,
                tr("Read"),
                SLOT_EMPTY,
                Color32::from_rgb(0xa8, 0x9c, 0xf2),
            );
            g.rect_stroke(
                rr,
                3.0,
                Stroke::new(1.0, Color32::from_rgb(0x6a, 0x5a, 0xc0)),
                StrokeKind::Inside,
            );
        } else {
            slot(
                g,
                row(r.automation, 16.0),
                tr("Read"),
                SLOT_EMPTY,
                READ_GREEN,
            );
        }
    }
    let pan_c = pos2(strip.center().x, r.pan + 12.0);
    match key {
        _ if vca_strip => {}
        StripKey::Track(i) => pan_knob(
            ui,
            g,
            pan_c,
            id(ns, (key, "pan")),
            Some(&mut p.tracks[i].pan),
        ),
        _ => pan_knob(ui, g, pan_c, id(ns, (key, "pan")), None),
    }

    // Volume (dB) and peak-hold readouts.
    let vol_db = match key {
        StripKey::Track(i) => p.tracks[i].volume_db,
        StripKey::StereoOut => lin_to_db(p.master_vol).max(-60.0),
        StripKey::Master => 0.0,
    };
    let peaks = match key {
        StripKey::Track(i) => meters.tracks.get(i).copied().unwrap_or((0.0, 0.0)),
        _ => meters.master,
    };
    let hold = peak_hold.entry(key).or_insert(0.0);
    *hold = hold.max(peaks.0.max(peaks.1));
    let half = (inner_w - 4.0) / 2.0;
    let vol_r = Rect::from_min_size(pos2(sx, r.db), vec2(half, 14.0));
    let pk_r = Rect::from_min_size(pos2(sx + half + 4.0, r.db), vec2(half, 14.0));
    slot(
        g,
        vol_r,
        &format!("{vol_db:.1}"),
        Color32::from_gray(0x24),
        TEXT,
    );
    let hold_db = lin_to_db(*hold);
    let pk_text = if *hold <= 1e-5 {
        String::new()
    } else {
        format!("{hold_db:.1}")
    };
    let pk_col = if hold_db > 0.0 {
        Color32::from_rgb(0xff, 0x5a, 0x4a)
    } else {
        READ_GREEN
    };
    slot(g, pk_r, &pk_text, Color32::from_gray(0x24), pk_col);
    if ui
        .interact(pk_r, id(ns, (key, "peak")), Sense::click())
        .clicked()
    {
        *hold = 0.0;
    }

    // Fader + scale + meter.
    let name_h = 22.0;
    let fader_area = Rect::from_min_max(
        pos2(sx, r.fader_top),
        pos2(strip.right() - 4.0, strip.bottom() - 26.0 - name_h),
    );
    if let Some(db) = fader_and_meter(
        ui,
        g,
        fader_area,
        id(ns, (key, "fader")),
        vol_db,
        key != StripKey::Master,
        peaks,
        inactive,
    ) {
        match key {
            StripKey::Track(i) => p.tracks[i].volume_db = db,
            StripKey::StereoOut => p.master_vol = 10f32.powf(db / 20.0).min(2.0),
            StripKey::Master => {}
        }
    }

    // M / S
    let by = strip.bottom() - 22.0 - name_h;
    let bw = (inner_w - 6.0) / 2.0;
    let m_r = Rect::from_min_size(pos2(sx, by), vec2(bw, 16.0));
    let s_r = Rect::from_min_size(pos2(sx + bw + 6.0, by), vec2(bw, 16.0));
    match key {
        StripKey::Track(i) => {
            let t = &mut p.tracks[i];
            if small_button(ui, g, m_r, id(ns, (key, "m")), "M", t.mute, MUTE_ON) {
                t.mute = !t.mute;
            }
            if small_button(ui, g, s_r, id(ns, (key, "s")), "S", t.solo, SOLO_ON) {
                t.solo = !t.solo;
            }
        }
        _ => {
            slot(g, m_r, "M", SLOT_GRAY, TEXT_DIM);
            if key == StripKey::StereoOut {
                slot(g, s_r, tr("Bnc"), SLOT_GRAY, TEXT_DIM);
            }
        }
    }

    // Name.
    let name_r = Rect::from_min_max(pos2(strip.left(), strip.bottom() - name_h), strip.max);
    let font = FontId::proportional(10.5);
    let shown = elide(g, &name, &font, name_r.width() - 6.0);
    match name_style {
        NameStyle::Coloured => {
            g.rect_filled(name_r, 2.0, color);
            g.text(
                name_r.center(),
                Align2::CENTER_CENTER,
                shown,
                font,
                Color32::WHITE,
            );
        }
        NameStyle::Plain => {
            g.text(name_r.center(), Align2::CENTER_CENTER, shown, font, TEXT);
        }
    }
    if key == StripKey::StereoOut && name_style == NameStyle::Coloured {
        g.rect_stroke(
            strip,
            2.0,
            Stroke::new(1.5, Color32::from_rgb(0xb8, 0x3c, 0xc8)),
            StrokeKind::Inside,
        );
    }
    if selected && name_style == NameStyle::Coloured {
        g.rect_stroke(
            strip,
            2.0,
            Stroke::new(1.0, Color32::from_gray(0x8a)),
            StrokeKind::Inside,
        );
    }
}

/// The strip's Group slot: which VCA track moves this track's level (empty for none). Click
/// to pick one of the project's VCA tracks.
fn vca_slot(ui: &Ui, g: &Painter, r: Rect, p: &mut Project, i: usize, id: egui::Id) {
    let name = p.vca_of(i).map(|v| v.name.clone()).unwrap_or_default();
    let fill = if name.is_empty() {
        SLOT_EMPTY
    } else {
        SLOT_GRAY
    };
    slot(
        g,
        r,
        &elide(g, &name, &FontId::proportional(10.5), r.width() - 8.0),
        fill,
        TEXT,
    );
    let vcas: Vec<(usize, String)> = p
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.kind == TrackKind::Vca)
        .map(|(j, t)| (j, t.name.clone()))
        .collect();
    if vcas.is_empty() {
        return;
    }
    let resp = ui.interact(r, id, Sense::click());
    let current = p.tracks[i].vca;
    let mut picked = None;
    egui::Popup::menu(&resp).show(|ui| {
        if ui
            .selectable_label(current.is_none(), tr("No VCA"))
            .clicked()
        {
            picked = Some(None);
            ui.close();
        }
        for (j, name) in &vcas {
            if ui.selectable_label(current == Some(*j), name).clicked() {
                picked = Some(Some(*j));
                ui.close();
            }
        }
    });
    if let Some(vca) = picked {
        p.assign_vca(i, vca);
    }
}

fn toolbar(ui: &Ui, s: &mut UiState, bar: Rect) {
    const NS: &str = "mixer_toolbar";
    let g = ui.painter_at(bar);
    g.rect_filled(bar, 0.0, TOOLBAR_BG);
    g.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(0x24)),
    );
    let cy = bar.center().y;
    back_chevron(&g, pos2(bar.left() + 16.0, cy));
    let (can_undo, can_redo) = (s.can_undo, s.can_redo);
    let mut undo_pick = None;
    let v = &mut s.mixer;

    let mut x = bar.left() + 32.0;
    let (edit, w) = menu_title(ui, &g, x, cy, tr("Edit"), id(NS, "Edit"));
    x += w + 2.0;
    egui::Popup::menu(&edit).show(|ui| {
        ui.set_min_width(220.0);
        crate::widgets::undo_items(ui, can_undo, can_redo, &mut undo_pick);
        for item in [
            tr("Copy Channel Strip"),
            tr("Paste Channel Strip"),
            tr("Select Audio Channel Strips"),
        ] {
            todo_item(ui, item);
        }
    });
    if undo_pick.is_some() {
        s.undo_cmd = undo_pick;
    }
    let (options, w) = menu_title(ui, &g, x, cy, tr("Options"), id(NS, "Options"));
    x += w + 2.0;
    egui::Popup::menu(&options).show(|ui| {
        ui.set_min_width(220.0);
        for item in [
            tr("Create New Auxiliary Channel Strip"),
            tr("Create New Output Channel Strip"),
            tr("Create New VCA"),
            tr("Create Group…"),
        ] {
            todo_item(ui, item);
        }
    });
    let (view, w) = menu_title(ui, &g, x, cy, tr("View"), id(NS, "View"));
    x += w + 12.0;
    egui::Popup::menu(&view).show(|ui| {
        ui.set_min_width(220.0);
        if ui
            .selectable_label(v.wide, tr("Wide Channel Strips"))
            .clicked()
        {
            v.wide = !v.wide;
            ui.close();
        }
        todo_item(ui, tr("Show Signal Flow Channel Strips"));
    });

    let menus_end = x;
    // Right-hand side first (strip width buttons, then filters), so the rest can be fitted
    // into what is left.
    let mut rx = bar.right() - 8.0;
    for (wide, key) in [(true, "wide"), (false, "narrow")] {
        let r = Rect::from_min_size(pos2(rx - 26.0, cy - 10.0), vec2(26.0, 20.0));
        let resp = ui.interact(r, id(NS, key), Sense::click());
        if v.wide == wide {
            g.rect_filled(r, 4.0, Color32::from_gray(0x5a));
        }
        let c = r.center();
        let iw = if wide { 13.0 } else { 9.0 };
        g.rect_stroke(
            Rect::from_center_size(c, vec2(iw, 11.0)),
            1.5,
            stroke(TEXT),
            StrokeKind::Middle,
        );
        g.line_segment([c + vec2(0.0, -5.5), c + vec2(0.0, 5.5)], stroke(TEXT));
        if resp.clicked() {
            v.wide = wide;
        }
        rx = r.left() - 4.0;
    }
    rx -= 10.0;
    let filter_widths: Vec<f32> = FILTERS
        .iter()
        .map(|label| {
            g.layout_no_wrap(tr(label).into(), FontId::proportional(12.5), TEXT)
                .size()
                .x
                + 16.0
        })
        .collect();
    let filters_w = filter_widths.iter().map(|w| w + 3.0).sum::<f32>();
    let fit = fit_toolbar(menus_end, bar.center().x, rx, filters_w);

    if fit.filters_inline {
        for (i, label) in FILTERS.iter().enumerate().rev() {
            let w = filter_widths[i];
            let r = Rect::from_min_size(pos2(rx - w, cy - 10.0), vec2(w, 20.0));
            let resp = ui.interact(r, id(NS, ("filter", i)), Sense::click());
            let on = v.filters[i];
            g.rect_filled(r, 4.0, if on { BLUE } else { Color32::from_gray(0x50) });
            g.text(
                r.center(),
                Align2::CENTER_CENTER,
                tr(label),
                FontId::proportional(12.5),
                if on { Color32::WHITE } else { TEXT_DIM },
            );
            if resp.clicked() {
                v.filters[i] = !v.filters[i];
            }
            rx = r.left() - 3.0;
        }
    } else {
        // Not enough room for a button per strip type: one menu with a check for each.
        let r = Rect::from_min_size(
            pos2(rx - FILTER_MENU_W, cy - 10.0),
            vec2(FILTER_MENU_W, 20.0),
        );
        let resp = popup_button(ui, &g, r, id(NS, "filters_menu"), tr("Show"));
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(160.0);
            for (i, label) in FILTERS.iter().enumerate() {
                if ui.checkbox(&mut v.filters[i], tr(label)).clicked() {
                    ui.close();
                }
            }
        });
    }

    if fit.show_sends {
        draw_sends_on_faders(ui, &g, menus_end, cy);
    }
    let left = fit.modes_left;

    // Single | Tracks | All
    let modes = [
        (MixerMode::Single, tr("Single")),
        (MixerMode::Tracks, tr("Tracks")),
        (MixerMode::All, tr("All")),
    ];
    let seg_w = MODE_W / 3.0;
    for (i, (m, label)) in modes.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(left + i as f32 * seg_w, cy - 10.0),
            vec2(seg_w - 2.0, 20.0),
        );
        let resp = ui.interact(r, id(NS, ("mode", *label)), Sense::click());
        let on = v.mode == *m;
        if on {
            g.rect_filled(r, 4.0, BLUE);
        } else if resp.hovered() {
            g.rect_filled(r, 4.0, Color32::from_white_alpha(14));
        }
        g.text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            FontId::proportional(12.5),
            if on { Color32::WHITE } else { TEXT },
        );
        if resp.clicked() {
            v.mode = *m;
        }
    }
}

/// Width of the Single | Tracks | All switch, of the "Sends on Faders" group and of the
/// "Show" menu that replaces the filter buttons when they do not fit.
const MODE_W: f32 = 174.0;
const SENDS_W: f32 = 236.0;
const FILTER_MENU_W: f32 = 70.0;
const TOOLBAR_GAP: f32 = 10.0;

/// Where the Mixer toolbar's groups go so that none overlaps another: the mode switch at the
/// centre when there is room, else just left of the filters; the filters as buttons when they
/// fit, else as one menu; "Sends on Faders" only in the room left over.
#[derive(Clone, Copy, PartialEq, Debug)]
struct ToolbarFit {
    modes_left: f32,
    filters_inline: bool,
    show_sends: bool,
}

fn fit_toolbar(menus_end: f32, center: f32, right_end: f32, filters_w: f32) -> ToolbarFit {
    let place = |filters_left: f32| {
        let centred = center - MODE_W / 2.0 + 60.0;
        centred.min(filters_left - TOOLBAR_GAP - MODE_W)
    };
    let mut filters_inline = true;
    let mut modes_left = place(right_end - filters_w);
    if modes_left < menus_end + TOOLBAR_GAP {
        filters_inline = false;
        modes_left = place(right_end - FILTER_MENU_W);
    }
    // When even that is too tight, the modes stay just after the menus (the panel has a
    // minimum width, so this only happens in very narrow windows).
    modes_left = modes_left.max(menus_end + TOOLBAR_GAP);
    let show_sends = menus_end + SENDS_W + TOOLBAR_GAP <= modes_left;
    ToolbarFit {
        modes_left,
        filters_inline,
        show_sends,
    }
}

fn draw_sends_on_faders(ui: &Ui, g: &egui::Painter, x: f32, cy: f32) {
    const NS: &str = "mixer_toolbar";
    // Sends on Faders (no sends yet: only "Off").
    g.text(
        pos2(x, cy),
        Align2::LEFT_CENTER,
        tr("Sends on Faders:"),
        FontId::proportional(12.5),
        TEXT_DIM,
    );
    let pw = pos2(x + 112.0, cy + 1.0);
    g.circle_stroke(pw, 5.5, stroke(TEXT_DIM));
    g.line_segment(
        [pw + vec2(0.0, -7.5), pw + vec2(0.0, -1.5)],
        Stroke::new(3.0, TOOLBAR_BG),
    );
    g.line_segment(
        [pw + vec2(0.0, -7.0), pw + vec2(0.0, -2.0)],
        stroke(TEXT_DIM),
    );
    let sof = Rect::from_min_size(pos2(x + 126.0, cy - 10.0), vec2(110.0, 20.0));
    let resp = popup_button(ui, g, sof, id(NS, "sends_on_faders"), tr("Off"));
    egui::Popup::menu(&resp).show(|ui| {
        let _ = ui.selectable_label(true, tr("Off"));
        todo_item(ui, &trf("Bus {}", &[&1]));
    });
}

/// Whether a strip of `kind` is shown with the toolbar's filter buttons set as `filters`
/// (Audio, Inst, Aux, Bus, Input, Output, Master/VCA, MIDI).
fn strip_visible(kind: TrackKind, filters: &[bool; 8]) -> bool {
    match kind {
        TrackKind::Audio => filters[0],
        TrackKind::Instrument | TrackKind::Drummer => filters[1],
        TrackKind::Aux => filters[2],
        TrackKind::Bus => filters[3],
        TrackKind::Output => filters[5],
        TrackKind::Vca => filters[6],
        TrackKind::ExternalMidi => filters[7],
        // A folder is only a way to group tracks; it has no channel strip.
        TrackKind::Folder => false,
    }
}

pub fn show(ui: &mut Ui, p: &mut Project, s: &mut UiState, meters: &Meters) {
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let bar = Rect::from_min_size(area.min, vec2(area.width(), TOOLBAR_H));
    toolbar(ui, s, bar);
    let v = &mut s.mixer;

    let body = Rect::from_min_max(pos2(area.left(), bar.bottom()), area.max);
    let labels = Rect::from_min_max(body.min, pos2(body.left() + LABEL_W, body.bottom()));
    let strips_area = Rect::from_min_max(pos2(labels.right(), body.top()), body.max);
    let g = ui.painter_at(body);
    g.rect_filled(body, 0.0, MIXER_BG);

    // Which strips, in order.
    let filters = v.filters;
    let mut keys: Vec<StripKey> = if p.tracks.is_empty() {
        Vec::new()
    } else {
        match v.mode {
            MixerMode::Single => vec![StripKey::Track(p.selected_track.min(p.tracks.len() - 1))],
            MixerMode::Tracks | MixerMode::All => (0..p.tracks.len())
                .filter(|i| strip_visible(p.tracks[*i].kind, &filters))
                .map(StripKey::Track)
                .collect(),
        }
    };
    if filters[5] {
        keys.push(StripKey::StereoOut);
    }
    if filters[6] && v.mode != MixerMode::Single {
        keys.push(StripKey::Master);
    }

    let strip_w = if v.wide { 96.0 } else { 80.0 };
    let total_w = keys.len() as f32 * (strip_w + GAP);
    if ui
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|pt| strips_area.contains(pt))
    {
        let d = ui.input(|i| i.smooth_scroll_delta);
        v.scroll_x -= d.x + d.y;
    }
    v.scroll_x = v
        .scroll_x
        .clamp(0.0, (total_w - strips_area.width() + 8.0).max(0.0));
    let r = rows(body.top(), fx_rows_for(p, &keys));

    // Row labels.
    let gl = ui.painter_at(labels);
    let lx = labels.right() - 6.0;
    let label_at = |y: f32, h: f32, t: &str| {
        gl.text(
            pos2(lx, y + h / 2.0),
            Align2::RIGHT_CENTER,
            t,
            FontId::proportional(10.5),
            TEXT_DIM,
        );
    };
    label_at(r.setting, 16.0, tr("Setting"));
    label_at(r.gain_red, 8.0, tr("Gain Reduction"));
    label_at(r.eq, 24.0, "EQ");
    label_at(r.input, 16.0, tr("Input"));
    label_at(r.fx, 15.0, tr("Audio FX"));
    label_at(r.sends, 16.0, tr("Sends"));
    label_at(r.output, 16.0, tr("Output"));
    label_at(r.group, 16.0, tr("Group"));
    label_at(r.automation, 16.0, tr("Automation"));
    label_at(r.pan, 24.0, tr("Pan"));
    label_at(r.db, 14.0, "dB");

    let gs = ui.painter_at(strips_area);
    for (n, key) in keys.iter().enumerate() {
        let x = strips_area.left() + 4.0 + n as f32 * (strip_w + GAP) - v.scroll_x;
        let strip = Rect::from_min_max(
            pos2(x, body.top() + 2.0),
            pos2(x + strip_w, body.bottom() - 2.0),
        );
        if strip.right() < strips_area.left() || strip.left() > strips_area.right() {
            continue;
        }
        channel_strip(
            ui,
            &gs,
            strip,
            *key,
            p,
            &mut v.peak_hold,
            meters,
            &r,
            "mixer",
            NameStyle::Coloured,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_groups_never_overlap_and_give_way_in_order() {
        // Wide panel: modes centred, every filter a button, Sends on Faders shown.
        let f = fit_toolbar(200.0, 1200.0, 2300.0, 520.0);
        assert!(f.filters_inline && f.show_sends);
        assert_eq!(f.modes_left, 1200.0 - MODE_W / 2.0 + 60.0);

        // Narrower: the modes move left of the filters, Sends on Faders goes first.
        let f = fit_toolbar(200.0, 700.0, 1100.0, 520.0);
        assert!(f.filters_inline && !f.show_sends);
        assert!(f.modes_left + MODE_W + TOOLBAR_GAP <= 1100.0 - 520.0);
        assert!(f.modes_left >= 200.0 + TOOLBAR_GAP);

        // Narrower still: the filters fold into one menu.
        let f = fit_toolbar(200.0, 500.0, 700.0, 520.0);
        assert!(!f.filters_inline);
        assert!(f.modes_left + MODE_W + TOOLBAR_GAP <= 700.0 - FILTER_MENU_W);
    }

    #[test]
    fn strips_follow_the_filter_buttons_and_folders_have_none() {
        let all = [true; 8];
        let mut only_aux = [false; 8];
        only_aux[2] = true;
        assert!(strip_visible(TrackKind::Aux, &all));
        assert!(strip_visible(TrackKind::Aux, &only_aux));
        assert!(!strip_visible(TrackKind::Audio, &only_aux));
        assert!(!strip_visible(TrackKind::Instrument, &only_aux));
        assert!(!strip_visible(TrackKind::Aux, &[false; 8]));
        assert!(
            !strip_visible(TrackKind::Folder, &all),
            "a folder has no channel strip"
        );
    }
}
