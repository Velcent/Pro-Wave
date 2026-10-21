//! A way to try the editors without a window: egui is fed pointer events frame by frame, as
//! it would get them from the operating system, and the editors run in it. Nothing outside the
//! process is touched.
use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, Vec2};

pub struct Harness {
    pub ctx: Context,
    time: f64,
    pub size: Vec2,
}

impl Harness {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            ctx: Context::default(),
            time: 0.0,
            size: Vec2::new(width, height),
        }
    }

    /// Runs one frame with `events`.
    pub fn frame(&mut self, events: Vec<Event>, mut ui: impl FnMut(&mut Ui)) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            ..RawInput::default()
        };
        let mut out = self.ctx.run_ui(input, |u| ui(u));
        // Nothing paints these frames, so the texture changes are not needed.
        out.textures_delta.clear();
    }

    /// A frame without input (widgets register where they are).
    pub fn settle(&mut self, mut ui: impl FnMut(&mut Ui)) {
        for _ in 0..2 {
            self.frame(vec![], &mut ui);
        }
    }

    fn button(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    pub fn move_to(&mut self, pos: Pos2, ui: impl FnMut(&mut Ui)) {
        self.frame(vec![Event::PointerMoved(pos)], ui);
    }

    /// Presses `button` at `pos` (after moving there), without letting go.
    pub fn press(&mut self, pos: Pos2, button: PointerButton, mut ui: impl FnMut(&mut Ui)) {
        self.move_to(pos, &mut ui);
        self.frame(vec![Self::button(pos, button, true)], &mut ui);
    }

    pub fn release(&mut self, pos: Pos2, button: PointerButton, ui: impl FnMut(&mut Ui)) {
        self.frame(vec![Self::button(pos, button, false)], ui);
    }

    /// A left click.
    pub fn click(&mut self, pos: Pos2, mut ui: impl FnMut(&mut Ui)) {
        self.press(pos, PointerButton::Primary, &mut ui);
        self.release(pos, PointerButton::Primary, &mut ui);
        self.frame(vec![], &mut ui);
    }

    pub fn right_click(&mut self, pos: Pos2, mut ui: impl FnMut(&mut Ui)) {
        self.press(pos, PointerButton::Secondary, &mut ui);
        self.release(pos, PointerButton::Secondary, &mut ui);
        self.frame(vec![], &mut ui);
    }

    /// Presses at `path[0]`, moves through the rest in steps, and lets go at the end.
    pub fn drag(&mut self, path: &[Pos2], button: PointerButton, mut ui: impl FnMut(&mut Ui)) {
        let (first, last) = (path[0], path[path.len() - 1]);
        self.press(first, button, &mut ui);
        for pt in &path[1..] {
            self.frame(vec![Event::PointerMoved(*pt)], &mut ui);
        }
        self.release(last, button, &mut ui);
        self.frame(vec![], &mut ui);
    }
}
