//! MIDI keyboard input: connects to the first MIDI input port (not the system's "Through"
//! port) and turns its note messages into [`LiveNote`]s for the app to play and record.
//! Without a MIDI device nothing happens; the app looks again every few seconds.

use crate::recorder::LiveNote;
use midir::{MidiInput, MidiInputConnection};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// How often to look for a MIDI keyboard while none is connected.
const RESCAN: Duration = Duration::from_secs(3);

pub struct MidiIn {
    connection: Option<MidiInputConnection<()>>,
    port: Option<String>,
    tx: Sender<LiveNote>,
    rx: Receiver<LiveNote>,
    last_try: Option<Instant>,
}

/// The note message in a raw MIDI message, if it is one (Note On with velocity 0 is a Note
/// Off). Channel and everything else are ignored.
pub fn parse(msg: &[u8]) -> Option<LiveNote> {
    let [status, pitch, vel, ..] = *msg else {
        return None;
    };
    if pitch > 127 {
        return None;
    }
    match status & 0xf0 {
        0xb0 if pitch == crate::recorder::SUSTAIN => Some(LiveNote::Sustain { val: vel.min(127) }),
        0x90 if vel > 0 => Some(LiveNote::On {
            pitch,
            vel: vel.min(127),
        }),
        0x90 | 0x80 => Some(LiveNote::Off { pitch }),
        _ => None,
    }
}

impl MidiIn {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let mut midi = Self {
            connection: None,
            port: None,
            tx,
            rx,
            last_try: None,
        };
        midi.connect();
        midi
    }

    /// The name of the connected keyboard.
    #[allow(dead_code)]
    pub fn port_name(&self) -> Option<&str> {
        self.port.as_deref()
    }

    fn connect(&mut self) {
        self.last_try = Some(Instant::now());
        let Ok(input) = MidiInput::new("Pro Wave") else {
            return;
        };
        let ports = input.ports();
        let picked = ports.iter().find_map(|port| {
            let name = input.port_name(port).ok()?;
            (!name.to_lowercase().contains("through")).then_some((port, name))
        });
        let Some((port, name)) = picked else {
            return;
        };
        let tx = self.tx.clone();
        match input.connect(
            port,
            "prowave-in",
            move |_stamp, msg, _| {
                if let Some(note) = parse(msg) {
                    let _ = tx.send(note);
                }
            },
            (),
        ) {
            Ok(connection) => {
                tracing::info!("MIDI input: {name}");
                self.connection = Some(connection);
                self.port = Some(name);
            }
            Err(e) => tracing::warn!("cannot open MIDI input {name}: {e}"),
        }
    }

    /// Appends the notes played since the last call to `out`.
    pub fn poll(&mut self, out: &mut Vec<LiveNote>) {
        if self.connection.is_none() && self.last_try.is_none_or(|t| t.elapsed() >= RESCAN) {
            self.connect();
        }
        while let Ok(note) = self.rx.try_recv() {
            out.push(note);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_messages_become_live_notes_on_any_channel() {
        assert_eq!(
            parse(&[0x90, 60, 100]),
            Some(LiveNote::On {
                pitch: 60,
                vel: 100
            })
        );
        assert_eq!(
            parse(&[0x93, 72, 1]),
            Some(LiveNote::On { pitch: 72, vel: 1 })
        );
        assert_eq!(parse(&[0x80, 60, 64]), Some(LiveNote::Off { pitch: 60 }));
        // Note On with velocity 0 is a Note Off.
        assert_eq!(parse(&[0x90, 60, 0]), Some(LiveNote::Off { pitch: 60 }));
    }

    #[test]
    fn the_sustain_pedal_is_controller_64() {
        assert_eq!(
            parse(&[0xb0, 64, 127]),
            Some(LiveNote::Sustain { val: 127 })
        );
        assert_eq!(parse(&[0xb2, 64, 0]), Some(LiveNote::Sustain { val: 0 }));
    }

    #[test]
    fn other_messages_are_ignored() {
        assert_eq!(parse(&[0xb0, 1, 127]), None, "modulation");
        assert_eq!(parse(&[0xe0, 0, 64]), None, "pitch bend");
        assert_eq!(parse(&[0xf8]), None, "clock");
        assert_eq!(parse(&[0x90, 60]), None, "cut short");
        assert_eq!(parse(&[0x90, 200, 100]), None, "pitch out of range");
        assert_eq!(parse(&[]), None);
    }
}
