//! Control bar (Logic Pro: "Control Bar"): view buttons, transport, LCD, count-in/metronome,
//! master volume, and the editor/browser buttons on the right.
//!
//! Every icon is drawn with vector shapes here (no bitmap or font glyphs), so it renders the
//! same on Windows, Linux and macOS.
use crate::{model::Project, Bottom, UiState};
use egui::epaint::text::{LayoutJob, TextFormat};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui,
};

const BAR_BG: Color32 = Color32::from_rgb(0x47, 0x47, 0x47);
const SEG_BG: Color32 = Color32::from_rgb(0x56, 0x56, 0x56);
const SEG_BORDER: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x3a);
const SEG_ON: Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2e);
const ICON: Color32 = Color32::from_rgb(0xb4, 0xb4, 0xb4);
const ICON_ON: Color32 = Color32::from_rgb(0xec, 0xec, 0xec);
const ICON_DIM: Color32 = Color32::from_rgb(0x7a, 0x7a, 0x7a);
const LCD_BG: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1e);
const LCD_TEXT: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
const LCD_MID: Color32 = Color32::from_rgb(0x9c, 0x9c, 0x9c);
const LCD_DIM: Color32 = Color32::from_rgb(0x4e, 0x4e, 0x50);
const PLAY_GREEN: Color32 = Color32::from_rgb(0x30, 0xa8, 0x4b);
const REC_RED: Color32 = Color32::from_rgb(0xe6, 0x34, 0x2c);
const CYCLE_YELLOW: Color32 = Color32::from_rgb(0xe8, 0xa6, 0x2a);
const PURPLE: Color32 = Color32::from_rgb(0x9b, 0x3f, 0xd6);

fn stroke(c: Color32) -> Stroke {
    Stroke::new(1.4, c)
}

fn clicked(ui: &Ui, rect: Rect, key: &str) -> bool {
    ui.interact(rect, egui::Id::unique(("control_bar", key)), Sense::click())
        .clicked()
}

// ---------------------------------------------------------------- icons

type IconFn = fn(&Painter, Rect, Color32);

fn icon_library(g: &Painter, r: Rect, c: Color32) {
    // Inbox tray.
    let b = Rect::from_center_size(r.center(), vec2(14.0, 11.0));
    g.rect_stroke(b, 2.0, stroke(c), StrokeKind::Middle);
    let y = b.top() + 6.0;
    g.add(Shape::line(
        vec![
            pos2(b.left(), y),
            pos2(b.left() + 4.0, y),
            pos2(b.left() + 5.0, y + 2.0),
            pos2(b.right() - 5.0, y + 2.0),
            pos2(b.right() - 4.0, y),
            pos2(b.right(), y),
        ],
        stroke(c),
    ));
}

fn icon_inspector(g: &Painter, r: Rect, c: Color32) {
    let ctr = r.center();
    g.circle_stroke(ctr, 7.0, stroke(c));
    g.circle_filled(ctr + vec2(0.0, -3.4), 1.1, c);
    g.line_segment(
        [ctr + vec2(0.0, -1.0), ctr + vec2(0.0, 4.0)],
        Stroke::new(1.6, c),
    );
}

fn icon_help(g: &Painter, r: Rect, c: Color32) {
    let ctr = r.center();
    g.circle_stroke(ctr, 7.0, stroke(c));
    g.text(
        ctr + vec2(0.0, 0.5),
        Align2::CENTER_CENTER,
        "?",
        FontId::proportional(10.5),
        c,
    );
}

fn icon_toolbar(g: &Painter, r: Rect, c: Color32) {
    let b = Rect::from_center_size(r.center(), vec2(15.0, 11.0));
    g.rect_stroke(b, 1.5, stroke(c), StrokeKind::Middle);
    g.line_segment(
        [
            pos2(b.left(), b.top() + 3.5),
            pos2(b.right(), b.top() + 3.5),
        ],
        stroke(c),
    );
    g.line_segment(
        [
            pos2(b.left() + 4.0, b.top() + 6.5),
            pos2(b.right() - 4.0, b.top() + 6.5),
        ],
        Stroke::new(1.0, c),
    );
}

fn icon_smart_controls(g: &Painter, r: Rect, c: Color32) {
    let ctr = r.center();
    g.circle_stroke(ctr, 3.6, stroke(c));
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::FRAC_PI_4;
        let d = vec2(a.cos(), a.sin());
        g.line_segment([ctr + d * 5.6, ctr + d * 7.6], stroke(c));
    }
}

fn icon_mixer(g: &Painter, r: Rect, c: Color32) {
    let ctr = r.center();
    for (i, knob) in [2.5f32, -3.0, 1.0].iter().enumerate() {
        let x = ctr.x + (i as f32 - 1.0) * 4.5;
        g.line_segment(
            [pos2(x, ctr.y - 7.0), pos2(x, ctr.y + 7.0)],
            Stroke::new(1.2, c),
        );
        g.rect_filled(
            Rect::from_center_size(pos2(x, ctr.y + knob), vec2(4.0, 2.4)),
            0.5,
            c,
        );
    }
}

fn icon_editors(g: &Painter, r: Rect, c: Color32) {
    // Scissors.
    let ctr = r.center();
    g.circle_stroke(ctr + vec2(-3.5, 4.5), 2.2, stroke(c));
    g.circle_stroke(ctr + vec2(3.5, 4.5), 2.2, stroke(c));
    g.line_segment([ctr + vec2(-2.4, 2.6), ctr + vec2(3.0, -7.0)], stroke(c));
    g.line_segment([ctr + vec2(2.4, 2.6), ctr + vec2(-3.0, -7.0)], stroke(c));
}

fn tri(g: &Painter, pts: [Pos2; 3], c: Color32) {
    g.add(Shape::convex_polygon(pts.to_vec(), c, Stroke::NONE));
}

fn icon_rewind(g: &Painter, r: Rect, c: Color32) {
    let p = r.center();
    tri(
        g,
        [p + vec2(-8.0, 0.0), p + vec2(0.0, -6.0), p + vec2(0.0, 6.0)],
        c,
    );
    tri(
        g,
        [p + vec2(0.0, 0.0), p + vec2(8.0, -6.0), p + vec2(8.0, 6.0)],
        c,
    );
}

fn icon_forward(g: &Painter, r: Rect, c: Color32) {
    let p = r.center();
    tri(
        g,
        [
            p + vec2(-8.0, -6.0),
            p + vec2(0.0, 0.0),
            p + vec2(-8.0, 6.0),
        ],
        c,
    );
    tri(
        g,
        [p + vec2(0.0, -6.0), p + vec2(8.0, 0.0), p + vec2(0.0, 6.0)],
        c,
    );
}

fn icon_to_start(g: &Painter, r: Rect, c: Color32) {
    let p = r.center();
    g.rect_filled(
        Rect::from_center_size(p + vec2(-5.5, 0.0), vec2(2.4, 12.0)),
        0.5,
        c,
    );
    tri(
        g,
        [p + vec2(-4.0, 0.0), p + vec2(5.5, -6.0), p + vec2(5.5, 6.0)],
        c,
    );
}

fn icon_play(g: &Painter, r: Rect, c: Color32) {
    let p = r.center() + vec2(1.0, 0.0);
    tri(
        g,
        [
            p + vec2(-5.0, -7.0),
            p + vec2(7.0, 0.0),
            p + vec2(-5.0, 7.0),
        ],
        c,
    );
}

fn icon_cycle(g: &Painter, r: Rect, c: Color32) {
    // Two half-loops with arrow heads (loop/cycle symbol).
    let p = r.center();
    let (rx, ry) = (7.5f32, 4.5f32);
    let arc = |from: f32, to: f32| -> Vec<Pos2> {
        (0..=12)
            .map(|i| {
                let a = from + (to - from) * i as f32 / 12.0;
                p + vec2(a.cos() * rx, a.sin() * ry)
            })
            .collect()
    };
    use std::f32::consts::PI;
    g.add(Shape::line(arc(PI * 1.05, PI * 1.95), Stroke::new(1.6, c)));
    g.add(Shape::line(arc(PI * 0.05, PI * 0.95), Stroke::new(1.6, c)));
    // heads: top-right pointing right, bottom-left pointing left
    let tr = p + vec2(rx * 0.95, -ry * 0.3);
    tri(
        g,
        [
            tr + vec2(-3.5, -3.0),
            tr + vec2(1.8, -0.2),
            tr + vec2(-2.0, 2.5),
        ],
        c,
    );
    let bl = p + vec2(-rx * 0.95, ry * 0.3);
    tri(
        g,
        [
            bl + vec2(3.5, 3.0),
            bl + vec2(-1.8, 0.2),
            bl + vec2(2.0, -2.5),
        ],
        c,
    );
}

fn icon_low_latency(g: &Painter, r: Rect, c: Color32) {
    // Shield with a check mark.
    let p = r.center();
    let pts = vec![
        p + vec2(-6.0, -6.5),
        p + vec2(6.0, -6.5),
        p + vec2(6.0, 0.0),
        p + vec2(0.0, 7.0),
        p + vec2(-6.0, 0.0),
        p + vec2(-6.0, -6.5),
    ];
    g.add(Shape::line(pts, stroke(c)));
    g.add(Shape::line(
        vec![
            p + vec2(-3.0, -0.5),
            p + vec2(-0.5, 2.0),
            p + vec2(3.5, -3.0),
        ],
        stroke(c),
    ));
}

fn icon_tuner(g: &Painter, r: Rect, c: Color32) {
    // Tuning fork.
    let p = r.center();
    g.add(Shape::line(
        vec![
            p + vec2(-3.0, -7.0),
            p + vec2(-3.0, 0.0),
            p + vec2(0.0, 2.5),
            p + vec2(3.0, 0.0),
            p + vec2(3.0, -7.0),
        ],
        stroke(c),
    ));
    g.line_segment([p + vec2(0.0, 2.5), p + vec2(0.0, 7.5)], stroke(c));
}

fn icon_metronome(g: &Painter, r: Rect, c: Color32) {
    let p = r.center();
    let body = vec![
        p + vec2(-6.0, 7.0),
        p + vec2(-2.5, -7.0),
        p + vec2(2.5, -7.0),
        p + vec2(6.0, 7.0),
        p + vec2(-6.0, 7.0),
    ];
    g.add(Shape::line(body, stroke(c)));
    g.line_segment([p + vec2(-4.8, 3.0), p + vec2(4.8, 3.0)], stroke(c));
    g.line_segment([p + vec2(0.0, 3.0), p + vec2(4.5, -6.0)], stroke(c));
}

fn icon_list_editors(g: &Painter, r: Rect, c: Color32) {
    let p = r.center();
    for i in -1..=1 {
        let y = p.y + i as f32 * 4.5;
        g.circle_filled(pos2(p.x - 6.0, y), 1.1, c);
        g.line_segment([pos2(p.x - 3.0, y), pos2(p.x + 7.0, y)], stroke(c));
    }
}

fn icon_note_pad(g: &Painter, r: Rect, c: Color32) {
    let b = Rect::from_center_size(r.center(), vec2(13.0, 13.0));
    g.rect_stroke(b, 2.0, stroke(c), StrokeKind::Middle);
    g.line_segment(
        [
            b.left_top() + vec2(0.0, 4.0),
            b.right_top() + vec2(0.0, 4.0),
        ],
        stroke(c),
    );
    g.line_segment(
        [
            b.left_top() + vec2(3.0, 8.0),
            b.right_top() + vec2(-3.0, 8.0),
        ],
        Stroke::new(1.0, c),
    );
}

fn icon_loops(g: &Painter, r: Rect, c: Color32) {
    // Speech-bubble-like loop (Apple Loops browser).
    let p = r.center() + vec2(0.0, -1.0);
    g.circle_stroke(p, 6.5, stroke(c));
    g.add(Shape::line(
        vec![
            p + vec2(-4.0, 5.0),
            p + vec2(-6.5, 8.5),
            p + vec2(-1.0, 6.3),
        ],
        stroke(c),
    ));
}

fn icon_browsers(g: &Painter, r: Rect, c: Color32) {
    let p = r.center();
    let back = Rect::from_center_size(p + vec2(2.5, -2.0), vec2(11.0, 9.0));
    let front = Rect::from_center_size(p + vec2(-2.0, 2.5), vec2(11.0, 9.0));
    g.rect_stroke(back, 1.5, stroke(c), StrokeKind::Middle);
    g.rect_filled(front, 1.5, BAR_BG);
    g.rect_stroke(front, 1.5, stroke(c), StrokeKind::Middle);
}

// ---------------------------------------------------------------- widgets

/// A joined segmented control (Logic's grouped toolbar buttons). Returns the clicked index.
fn segmented(
    ui: &Ui,
    g: &Painter,
    left: f32,
    cy: f32,
    key: &str,
    items: &[(IconFn, bool)],
) -> Option<usize> {
    let seg = vec2(30.0, 24.0);
    let full = Rect::from_min_size(
        pos2(left, cy - seg.y / 2.0),
        vec2(seg.x * items.len() as f32, seg.y),
    );
    g.rect_filled(full, 5.0, SEG_BG);
    let mut hit = None;
    for (i, (draw, on)) in items.iter().enumerate() {
        let r = Rect::from_min_size(pos2(left + i as f32 * seg.x, full.top()), seg);
        if *on {
            let cr = CornerRadius {
                nw: if i == 0 { 5 } else { 0 },
                sw: if i == 0 { 5 } else { 0 },
                ne: if i + 1 == items.len() { 5 } else { 0 },
                se: if i + 1 == items.len() { 5 } else { 0 },
            };
            g.rect_filled(r, cr, SEG_ON);
        }
        if i > 0 {
            g.line_segment(
                [
                    pos2(r.left(), r.top() + 4.0),
                    pos2(r.left(), r.bottom() - 4.0),
                ],
                Stroke::new(1.0, SEG_BORDER),
            );
        }
        draw(g, r, if *on { ICON_ON } else { ICON });
        if clicked(ui, r, &format!("{key}{i}")) {
            hit = Some(i);
        }
    }
    g.rect_stroke(full, 5.0, Stroke::new(1.0, SEG_BORDER), StrokeKind::Inside);
    hit
}

/// Transport-style button: icon only, or icon on a coloured rounded square when active.
fn tbutton(
    ui: &Ui,
    g: &Painter,
    center: Pos2,
    key: &str,
    draw: IconFn,
    active_bg: Option<Color32>,
    icon: Color32,
) -> bool {
    let r = Rect::from_center_size(center, vec2(30.0, 26.0));
    let resp = ui.interact(r, egui::Id::unique(("control_bar", key)), Sense::click());
    if let Some(bg) = active_bg {
        g.rect_filled(r, 6.0, bg);
    } else if resp.hovered() {
        g.rect_filled(r, 6.0, Color32::from_white_alpha(14));
    }
    draw(g, r, icon);
    resp.clicked()
}

/// Appends `text`, drawing the first `dim` characters in the dim LCD colour.
fn lcd_seg(job: &mut LayoutJob, text: &str, dim: usize, size: f32, color: Color32) {
    let split = text.char_indices().nth(dim).map_or(text.len(), |(i, _)| i);
    let font = FontId::proportional(size);
    if split > 0 {
        job.append(
            &text[..split],
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: LCD_DIM,
                ..Default::default()
            },
        );
    }
    job.append(
        &text[split..],
        0.0,
        TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
}

/// "009 1 1 001"-style position with dim leading zeros.
fn position_job(bbdt: (u32, u32, u32, u32), size: f32, color: Color32) -> LayoutJob {
    let (bar, beat, div, tick) = bbdt;
    let mut job = LayoutJob::default();
    let bar_s = format!("{bar:03}");
    let bar_dim = bar_s.len() - bar.to_string().len();
    lcd_seg(&mut job, &bar_s, bar_dim, size, color);
    lcd_seg(&mut job, &format!("  {beat}  {div}  "), 0, size, color);
    let tick_s = format!("{tick:03}");
    let tick_dim = tick_s.len() - tick.to_string().len();
    lcd_seg(&mut job, &tick_s, tick_dim, size, color);
    job
}

fn lcd(ui: &Ui, g: &Painter, p: &mut Project, rect: Rect, cpu_load: f32) {
    g.rect_filled(rect, 6.0, LCD_BG);
    g.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, Color32::from_gray(0x10)),
        StrokeKind::Inside,
    );
    let top = rect.top() + 4.0;
    let bot = rect.center().y + 1.0;
    let x0 = rect.left();

    // Column 1: SMPTE over bars/beats.
    let mut job = LayoutJob::default();
    lcd_seg(&mut job, &p.timecode(), 0, 18.0, LCD_TEXT);
    g.galley(pos2(x0 + 8.0, top), g.layout_job(job), LCD_TEXT);
    let pos = p.bbdt(p.position_beats);
    g.galley(
        pos2(x0 + 14.0, bot),
        g.layout_job(position_job(pos, 18.0, LCD_TEXT)),
        LCD_TEXT,
    );

    // Column 2: left/right locators (dimmer).
    let l = p.bbdt(p.cycle_start);
    let r = p.bbdt(p.cycle_end);
    g.galley(
        pos2(x0 + 158.0, top + 1.0),
        g.layout_job(position_job(l, 15.0, LCD_MID)),
        LCD_MID,
    );
    g.galley(
        pos2(x0 + 158.0, bot + 1.0),
        g.layout_job(position_job(r, 15.0, LCD_MID)),
        LCD_MID,
    );

    // Column 3: tempo (drag vertically to change it, like Logic).
    let tempo_rect = Rect::from_min_size(pos2(x0 + 252.0, rect.top()), vec2(84.0, rect.height()));
    let resp = ui.interact(
        tempo_rect,
        egui::Id::unique(("control_bar", "tempo")),
        Sense::drag(),
    );
    if resp.dragged() {
        p.tempo = (p.tempo - resp.drag_delta().y * 0.5).clamp(5.0, 990.0);
    }
    g.text(
        pos2(tempo_rect.center().x, top),
        Align2::CENTER_TOP,
        format!("{:.4}", p.tempo),
        FontId::proportional(18.0),
        LCD_TEXT,
    );
    g.text(
        pos2(tempo_rect.center().x, bot + 2.0),
        Align2::CENTER_TOP,
        "Keep Tempo",
        FontId::proportional(13.5),
        LCD_TEXT,
    );

    // Column 4: time signature / division.
    let sig_x = x0 + 360.0;
    g.text(
        pos2(sig_x, top),
        Align2::CENTER_TOP,
        format!("{}/{}", p.sig.0, p.sig.1),
        FontId::proportional(18.0),
        LCD_TEXT,
    );
    g.text(
        pos2(sig_x + 3.0, bot + 1.0),
        Align2::CENTER_TOP,
        "/16",
        FontId::proportional(16.0),
        LCD_TEXT,
    );

    // Column 5: MIDI in/out activity.
    g.text(
        pos2(x0 + 386.0, top + 3.0),
        Align2::LEFT_TOP,
        "No In",
        FontId::proportional(13.5),
        LCD_TEXT,
    );
    g.text(
        pos2(x0 + 386.0, bot + 2.0),
        Align2::LEFT_TOP,
        "No Out",
        FontId::proportional(13.5),
        LCD_TEXT,
    );

    // Column 6: CPU (audio-callback DSP load) / HD (disk; not measured yet) meters.
    for (i, label) in ["CPU", "HD"].iter().enumerate() {
        let y = rect.top() + 12.0 + i as f32 * 18.0;
        g.text(
            pos2(x0 + 474.0, y),
            Align2::LEFT_CENTER,
            *label,
            FontId::proportional(7.0),
            LCD_DIM,
        );
        let m = Rect::from_min_size(pos2(x0 + 490.0, y - 2.0), vec2(40.0, 4.0));
        g.rect_filled(m, 1.0, Color32::from_gray(0x2c));
        if i == 0 {
            let load = cpu_load.clamp(0.0, 1.0);
            let fill = Rect::from_min_size(m.min, vec2(m.width() * load, m.height()));
            let color = if cpu_load >= 0.9 {
                Color32::from_rgb(0xe0, 0x40, 0x30)
            } else {
                LCD_TEXT
            };
            g.rect_filled(fill, 1.0, color);
            let hover = m.expand2(vec2(14.0, 4.0)).with_min_x(x0 + 470.0);
            ui.interact(
                hover,
                egui::Id::unique(("lcd", "cpu_meter")),
                egui::Sense::hover(),
            )
            .on_hover_text(format!("Audio DSP load: {:.0}%", cpu_load * 100.0));
        }
    }

    // Chevron (display mode menu).
    let c = pos2(rect.right() - 11.0, rect.center().y);
    g.add(Shape::line(
        vec![
            c + vec2(-4.0, -2.0),
            c + vec2(0.0, 2.0),
            c + vec2(4.0, -2.0),
        ],
        Stroke::new(1.6, LCD_MID),
    ));
    g.line_segment(
        [
            pos2(rect.right() - 22.0, rect.top() + 6.0),
            pos2(rect.right() - 22.0, rect.bottom() - 6.0),
        ],
        Stroke::new(1.0, Color32::from_gray(0x2a)),
    );
}

/// Level (0..1) for a peak amplitude: -60 dB at the left end, +6 dB at the right end.
fn meter_frac(peak: f32) -> f32 {
    if peak <= 1e-6 {
        return 0.0;
    }
    ((20.0 * peak.log10() + 60.0) / 66.0).clamp(0.0, 1.0)
}

/// Master volume as in Logic Pro: a dark pill holding two thin stereo meter bars (green, yellow
/// above -6 dB, red above 0 dB) with a translucent round knob sliding over them.
fn master_volume(ui: &Ui, g: &Painter, p: &mut Project, rect: Rect, peaks: Option<(f32, f32)>) {
    const RAD: f32 = 10.0;
    let travel = (rect.left() + RAD + 1.0, rect.right() - RAD - 1.0);
    let resp = ui.interact(
        rect.expand2(vec2(4.0, 4.0)),
        egui::Id::unique(("control_bar", "master")),
        Sense::click_and_drag(),
    );
    if let Some(pt) = resp.interact_pointer_pos() {
        p.master_vol = ((pt.x - travel.0) / (travel.1 - travel.0)).clamp(0.0, 1.0);
    }

    // Pill with a soft inner edge.
    let r = rect.height() / 2.0;
    g.rect_filled(rect, r, Color32::from_gray(0x2b));
    g.rect_stroke(
        rect,
        r,
        Stroke::new(1.0, Color32::from_gray(0x4c)),
        egui::StrokeKind::Inside,
    );
    g.rect_stroke(
        rect.shrink(1.5),
        r - 1.5,
        Stroke::new(1.0, Color32::from_gray(0x1c)),
        egui::StrokeKind::Inside,
    );

    // Two stereo meter bars.
    let inner = Rect::from_min_max(
        pos2(rect.left() + 9.0, rect.top() + 5.0),
        pos2(rect.right() - 9.0, rect.bottom() - 5.0),
    );
    let bar_h = ((inner.height() - 2.0) / 2.0).floor().max(3.0);
    let green = Color32::from_rgb(0x2f, 0xa8, 0x3c);
    let yellow = Color32::from_rgb(0xb4, 0xa0, 0x20);
    let red = Color32::from_rgb(0xd8, 0x3a, 0x2c);
    let (l, rr) = peaks.unwrap_or((0.0, 0.0));
    for (i, peak) in [l, rr].into_iter().enumerate() {
        let y = inner.top() + i as f32 * (bar_h + 2.0);
        let bar = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), bar_h));
        g.rect_filled(bar, 2.0, Color32::from_gray(0x1d));
        let f = meter_frac(peak);
        for (lo, hi, col) in [
            (0.0f32, 0.82f32, green),
            (0.82, 0.91, yellow),
            (0.91, 1.0, red),
        ] {
            let x1 = bar.left() + bar.width() * lo;
            let x2 = bar.left() + bar.width() * hi.min(f);
            if x2 > x1 {
                let seg = Rect::from_min_max(pos2(x1, bar.top()), pos2(x2, bar.bottom()));
                g.rect_filled(seg, 1.5, col);
                // Light centre line and a darker lower edge give the bar its soft round look.
                let mid = seg.center().y;
                g.line_segment(
                    [pos2(seg.left(), mid), pos2(seg.right(), mid)],
                    Stroke::new(1.0, col.lerp_to_gamma(Color32::WHITE, 0.35)),
                );
                g.line_segment(
                    [
                        pos2(seg.left(), seg.bottom() - 0.5),
                        pos2(seg.right(), seg.bottom() - 0.5),
                    ],
                    Stroke::new(1.0, col.lerp_to_gamma(Color32::BLACK, 0.35)),
                );
            }
        }
    }

    // Translucent glassy knob.
    let c = pos2(
        travel.0 + (travel.1 - travel.0) * p.master_vol,
        rect.center().y,
    );
    g.circle_filled(c + vec2(0.0, 1.0), RAD + 1.0, Color32::from_black_alpha(70));
    g.circle_filled(
        c,
        RAD,
        Color32::from_rgba_unmultiplied(0xd2, 0xd2, 0xd2, 0xc8),
    );
    g.circle_stroke(
        c,
        RAD,
        Stroke::new(1.2, Color32::from_rgba_unmultiplied(0xf2, 0xf2, 0xf2, 0xe6)),
    );
    g.circle_stroke(
        c,
        RAD - 3.5,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x8a, 0x8a, 0x8a, 0x90)),
    );
}

pub fn control_bar(
    ui: &mut Ui,
    p: &mut Project,
    s: &mut UiState,
    master_peaks: Option<(f32, f32)>,
) {
    let bar = ui.max_rect();
    let g = ui.painter_at(bar);
    g.rect_filled(bar, 0.0, BAR_BG);
    g.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(0x24)),
    );
    let cy = bar.center().y;

    // Left: Library / Inspector / Quick Help / Toolbar.
    let left = [
        (icon_library as IconFn, s.show_library),
        (icon_inspector, s.show_inspector),
        (icon_help, false),
        (icon_toolbar, false),
    ];
    match segmented(ui, &g, bar.left() + 8.0, cy, "view", &left) {
        Some(0) => s.show_library = !s.show_library,
        Some(1) => s.show_inspector = !s.show_inspector,
        _ => {}
    }
    // Smart Controls / Mixer / Editors.
    let editors_on = s.bottom == Bottom::PianoRoll;
    let mid = [
        (icon_smart_controls as IconFn, s.light_menus),
        (icon_mixer, s.show_mixer && s.bottom == Bottom::Mixer),
        (icon_editors, editors_on),
    ];
    match segmented(ui, &g, bar.left() + 140.0, cy, "panes", &mid) {
        // Switches the menu bar and its menus between dark and light (white).
        Some(0) => s.light_menus = !s.light_menus,
        Some(1) => {
            s.show_mixer = !(s.show_mixer && s.bottom == Bottom::Mixer);
            s.bottom = Bottom::Mixer;
        }
        Some(2) => {
            s.bottom = if editors_on {
                Bottom::Mixer
            } else {
                Bottom::PianoRoll
            };
        }
        _ => {}
    }

    // Centre: LCD, transport to its left, count-in & co. to its right.
    let lcd_w = 555.0;
    let lcd_rect = Rect::from_center_size(pos2(bar.center().x + 20.0, cy), vec2(lcd_w, 44.0));
    lcd(ui, &g, p, lcd_rect, s.cpu_load);

    let tx = |dx: f32| pos2(lcd_rect.left() + dx, cy);
    if tbutton(ui, &g, tx(-198.0), "rew", icon_rewind, None, ICON) {
        p.position_beats = (p.position_beats - p.beats_per_bar()).max(0.0);
    }
    if tbutton(ui, &g, tx(-170.0), "ffw", icon_forward, None, ICON) {
        p.position_beats += p.beats_per_bar();
    }
    if tbutton(ui, &g, tx(-142.0), "start", icon_to_start, None, ICON) {
        p.position_beats = 0.0;
    }
    // Logic keeps the triangle while playing; only the background turns green.
    let play_icon: IconFn = icon_play;
    let play_bg = p.playing.then_some(PLAY_GREEN);
    if tbutton(
        ui,
        &g,
        tx(-108.0),
        "play",
        play_icon,
        play_bg,
        if p.playing { Color32::WHITE } else { ICON },
    ) {
        p.playing = !p.playing;
    }
    // Record: red dot (white dot on red while recording).
    let rec_r = Rect::from_center_size(tx(-76.0), vec2(30.0, 26.0));
    if p.recording {
        g.rect_filled(rec_r, 6.0, REC_RED);
    }
    g.circle_filled(
        rec_r.center(),
        6.5,
        if p.recording { Color32::WHITE } else { REC_RED },
    );
    if clicked(ui, rec_r, "rec") {
        p.recording = !p.recording;
    }
    // Capture Recording: red dot inside a ring.
    let cap_r = Rect::from_center_size(tx(-48.0), vec2(30.0, 26.0));
    g.circle_stroke(
        cap_r.center(),
        7.5,
        Stroke::new(1.6, Color32::from_rgb(0xe8, 0x6a, 0x3a)),
    );
    g.circle_filled(cap_r.center(), 4.8, REC_RED);
    let _capture = clicked(ui, cap_r, "capture");
    let cyc_bg = p.cycle.then_some(CYCLE_YELLOW);
    if tbutton(
        ui,
        &g,
        tx(-18.0),
        "cycle",
        icon_cycle,
        cyc_bg,
        if p.cycle { Color32::WHITE } else { ICON },
    ) {
        p.cycle = !p.cycle;
    }

    let rx = |dx: f32| pos2(lcd_rect.right() + dx, cy);
    let _low_latency = tbutton(ui, &g, rx(24.0), "lowlat", icon_low_latency, None, ICON);
    let _tuner = tbutton(ui, &g, rx(54.0), "tuner", icon_tuner, None, ICON_DIM);
    // Count-in "1234": purple when on.
    let ci = Rect::from_center_size(rx(110.0), vec2(28.0, 24.0));
    g.rect_filled(ci, 5.0, if p.count_in { PURPLE } else { SEG_BG });
    g.text(
        ci.center(),
        Align2::CENTER_CENTER,
        "1234",
        FontId::proportional(10.0),
        Color32::WHITE,
    );
    if clicked(ui, ci, "countin") {
        p.count_in = !p.count_in;
    }
    let met_bg = p.metronome.then_some(PURPLE);
    if tbutton(
        ui,
        &g,
        rx(140.0),
        "metronome",
        icon_metronome,
        met_bg,
        if p.metronome {
            Color32::WHITE
        } else {
            Color32::from_rgb(0xc0, 0xa8, 0xd8)
        },
    ) {
        p.metronome = !p.metronome;
    }

    // Right: master volume, then List Editors / Note Pads / Loop Browser / Browsers.
    let vol = Rect::from_min_size(
        pos2(bar.right() - 188.0 - 140.0, cy - 11.0),
        vec2(140.0, 22.0),
    );
    master_volume(ui, &g, p, vol, master_peaks);
    let right: [IconFn; 4] = [icon_list_editors, icon_note_pad, icon_loops, icon_browsers];
    for (i, draw) in right.iter().enumerate() {
        let c = pos2(bar.right() - 106.0 + i as f32 * 27.5, cy);
        let _ = tbutton(ui, &g, c, &format!("right{i}"), *draw, None, ICON);
    }
}
