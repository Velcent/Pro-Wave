//! Project model. UI never mutates the engine directly (see AGENTS.md: Command pattern);
//! this is the UI-side view model until `prowave-engine` lands.
// Some fields (pan, instrument, Bus/Output kinds) are reserved for upcoming mixer/instrument work.
#![allow(dead_code)]
use egui::Color32;
use prowave_engine::AudioFile;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Instrument,
    Audio,
    Drummer,
    Bus,
    Output,
}

#[derive(Clone)]
pub struct Note {
    pub start: f32, // beats from region start
    pub len: f32,
    pub pitch: u8,
    pub vel: u8,
}

#[derive(Clone)]
pub struct Region {
    pub name: String,
    pub start_bar: f32,
    pub len_bars: f32,
    pub notes: Vec<Note>,
    /// Set for audio regions backed by a file on disk.
    pub audio: Option<AudioRegion>,
    /// Region inspector parameters (non-destructive, applied when the song is built).
    pub params: RegionParams,
}

/// Logic Pro Region Inspector values.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct RegionParams {
    pub mute: bool,
    /// Playback quantize grid in beats (`None` = Off).
    pub quantize: Option<f32>,
    /// Semitones.
    pub transpose: i8,
    /// Added to every note velocity.
    pub velocity: i8,
}

/// One send slot of a channel strip (Logic: "Bus 1" with a level knob). Not routed in the
/// engine yet: it is stored and shown, but has no effect on the sound.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct SendSlot {
    /// Bus number 1..=8, or `None` for an empty slot.
    pub bus: Option<u8>,
    /// 0..=1 send level.
    pub level: f32,
    /// Pre-fader send (shown with the "eye" icon).
    pub pre: bool,
}

/// Logic Pro Track Inspector values for instrument tracks.
#[derive(Clone, Copy, PartialEq)]
pub struct TrackParams {
    pub transpose: i8,
    pub velocity: i8,
    /// Lowest/highest MIDI note that plays (Key Limit).
    pub key_limit: (u8, u8),
    /// Lowest/highest velocity that plays (Vel Limit).
    pub vel_limit: (u8, u8),
}

impl Default for TrackParams {
    fn default() -> Self {
        Self {
            transpose: 0,
            velocity: 0,
            key_limit: (0, 127),
            vel_limit: (1, 127),
        }
    }
}

#[derive(Clone)]
pub struct AudioRegion {
    pub file: AudioFile,
    /// Min/max overview, filled by a background thread after import.
    pub peaks: Arc<OnceLock<Vec<(f32, f32)>>>,
}

#[derive(Clone)]
pub struct Track {
    pub name: String,
    pub kind: TrackKind,
    pub color: Color32,
    pub mute: bool,
    pub solo: bool,
    pub rec: bool,
    /// Input monitoring ("I" button, audio tracks only).
    pub monitor: bool,
    pub volume_db: f32,
    pub pan: f32,
    pub inserts: Vec<&'static str>,
    pub instrument: Option<&'static str>,
    pub regions: Vec<Region>,
    pub params: TrackParams,
    pub send: SendSlot,
    /// Library category of the loaded patch (drives the track icon), if any.
    pub category: Option<&'static str>,
    /// Individual track height as a factor of the global track zoom (Logic: drag the lower
    /// edge of a track). 1.0 = the zoom slider's height.
    pub height: f32,
}

impl Track {
    /// Smallest and largest individual height factor.
    pub const MIN_HEIGHT: f32 = 0.5;
    pub const MAX_HEIGHT: f32 = 6.0;
}

pub struct Project {
    pub name: String,
    pub tempo: f32,
    pub sig: (u8, u8),
    pub key: &'static str,
    pub tracks: Vec<Track>,
    pub playing: bool,
    pub recording: bool,
    pub cycle: bool,
    pub metronome: bool,
    /// Count-in before recording (the purple "1234" button).
    pub count_in: bool,
    /// Cycle (left/right) locators, in beats.
    pub cycle_start: f64,
    pub cycle_end: f64,
    pub position_beats: f64, // 0-based
    pub selected_track: usize,
    pub master_vol: f32,
    /// Bumped whenever tracks/regions change so the engine song is rebuilt.
    pub song_rev: u64,
}

/// Deterministic pseudo-random so demo content is stable across runs.
fn lcg(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    *seed >> 8
}

fn demo_notes(seed: u32, bars: f32, lo: u8, hi: u8) -> Vec<Note> {
    let mut s = seed;
    let mut out = Vec::new();
    let mut t = 0.0;
    while t < bars * 4.0 {
        let len = [0.5, 0.75, 1.0, 1.5, 0.25][(lcg(&mut s) % 5) as usize];
        let pitch = lo + (lcg(&mut s) % (hi - lo) as u32) as u8;
        out.push(Note {
            start: t,
            len: len * 0.9,
            pitch,
            vel: 70 + (lcg(&mut s) % 50) as u8,
        });
        t += [0.5, 1.0, 0.25, 0.75][(lcg(&mut s) % 4) as usize];
    }
    out
}

impl Project {
    /// A new, empty project with one software instrument track (File > New).
    pub fn empty() -> Self {
        let mut p = Self::demo();
        p.name = "Untitled".into();
        p.tracks = vec![Track {
            name: "Inst 1".into(),
            kind: TrackKind::Instrument,
            color: Color32::from_rgb(0x4e, 0xb0, 0x4a),
            mute: false,
            solo: false,
            rec: false,
            monitor: false,
            volume_db: 0.0,
            pan: 0.0,
            inserts: vec![],
            instrument: Some("Sampler"),
            regions: vec![],
            params: TrackParams::default(),
            category: None,
            send: SendSlot::default(),
            height: 1.0,
        }];
        p.position_beats = 0.0;
        p.cycle = false;
        p.selected_track = 0;
        p
    }

    pub fn demo() -> Self {
        let green = Color32::from_rgb(0x4e, 0xb0, 0x4a);
        let blue = Color32::from_rgb(0x3f, 0x5f, 0xc0);
        let yellow = Color32::from_rgb(0xd9, 0xaa, 0x2a);
        let purple = Color32::from_rgb(0x7a, 0x4f, 0xc0);
        let mk = |name: &str,
                  kind,
                  color,
                  inst: Option<&'static str>,
                  ins: Vec<&'static str>,
                  seed,
                  vol| Track {
            name: name.into(),
            kind,
            color,
            mute: false,
            solo: false,
            rec: false,
            monitor: false,
            volume_db: vol,
            pan: 0.0,
            inserts: ins,
            instrument: inst,
            regions: vec![Region {
                name: name.into(),
                start_bar: 1.0,
                len_bars: 12.0,
                notes: if matches!(kind, TrackKind::Audio) {
                    vec![]
                } else {
                    demo_notes(seed, 12.0, 50, 76)
                },
                audio: None,
                params: RegionParams::default(),
            }],
            params: TrackParams::default(),
            category: None,
            send: SendSlot::default(),
            height: 1.0,
        };
        let tracks = vec![
            mk(
                "Classic Electric Piano",
                TrackKind::Instrument,
                green,
                Some("E-Piano"),
                vec!["Tape Delay", "Channel EQ"],
                1,
                -3.2,
            ),
            mk(
                "Steinway Grand Piano",
                TrackKind::Instrument,
                green,
                Some("Sampler"),
                vec!["Channel EQ", "Compressor"],
                2,
                -2.7,
            ),
            mk(
                "Analog Pad",
                TrackKind::Instrument,
                green,
                Some("Alchemy"),
                vec!["Chorus", "Space D"],
                3,
                -5.6,
            ),
            mk(
                "Drummer",
                TrackKind::Drummer,
                yellow,
                Some("Drum Kit"),
                vec!["Compressor"],
                4,
                -4.0,
            ),
            mk(
                "Electric Bass",
                TrackKind::Audio,
                blue,
                None,
                vec!["Channel EQ"],
                5,
                -8.2,
            ),
            mk(
                "Lead Vocal",
                TrackKind::Audio,
                purple,
                None,
                vec!["Channel EQ", "Compressor", "Space D"],
                6,
                -7.8,
            ),
        ];
        let mut tracks = tracks;
        for (t, bus) in tracks
            .iter_mut()
            .zip([Some(1u8), Some(3), Some(3), Some(3)])
        {
            t.send = SendSlot {
                bus,
                level: 0.0,
                pre: false,
            };
        }
        Self {
            name: "Untitled".into(),
            tempo: 120.0,
            sig: (4, 4),
            key: "Cmaj",
            tracks,
            playing: false,
            recording: false,
            cycle: false,
            metronome: false,
            count_in: true,
            cycle_start: 8.0,
            cycle_end: 88.0,
            position_beats: 16.0,
            selected_track: 0,
            master_vol: 0.8,
            song_rev: 0,
        }
    }

    /// Length of one signature beat in quarter notes: 1 for x/4, 0.5 for x/8, 2 for x/2.
    /// Positions ("beats" elsewhere in the app) are quarter notes, as in the engine's tempo
    /// map and Logic, whatever the signature. An invalid denominator counts as 4.
    pub fn beat_unit(&self) -> f64 {
        match self.sig.1 {
            d @ (1 | 2 | 4 | 8 | 16 | 32 | 64) => 4.0 / d as f64,
            _ => 1.0,
        }
    }

    /// Quarter notes per bar: 4 for 4/4, 3 for 6/8 and 3/4, 3.5 for 7/8.
    pub fn beats_per_bar(&self) -> f64 {
        self.sig.0.max(1) as f64 * self.beat_unit()
    }

    /// Logic-style position for a beat: (bar, beat, division, tick), all 1-based, where
    /// "beat" is the signature's beat (an eighth in 6/8), divisions are 16th notes and there
    /// are 240 ticks per division.
    pub fn bbdt(&self, beats: f64) -> (u32, u32, u32, u32) {
        let beats = beats.max(0.0);
        let per_bar = self.beats_per_bar();
        let bar = (beats / per_bar) as u32 + 1;
        let in_beats = (beats % per_bar) / self.beat_unit();
        let beat = in_beats as u32 + 1;
        let divs = (self.beat_unit() * 4.0).max(1.0);
        let sixteenths = in_beats.fract() * divs;
        let div = sixteenths as u32 + 1;
        let tick = (sixteenths.fract() * 240.0) as u32 + 1;
        (bar, beat, div, tick)
    }

    pub fn bar_beat_tick(&self) -> (u32, u32, u32) {
        let beats = self.position_beats.max(0.0);
        let per_bar = self.beats_per_bar();
        let bar = (beats / per_bar) as u32 + 1;
        let in_beats = (beats % per_bar) / self.beat_unit();
        let beat = in_beats as u32 + 1;
        let tick = (in_beats.fract() * 960.0) as u32;
        (bar, beat, tick)
    }

    pub fn timecode(&self) -> String {
        let secs = self.position_beats * 60.0 / self.tempo as f64;
        let total = secs + 3600.0; // SMPTE start offset 01:00:00:00
        let (h, m, s) = (
            (total / 3600.0) as u32,
            ((total % 3600.0) / 60.0) as u32,
            (total % 60.0) as u32,
        );
        // 25 fps, 80 subframes per frame (Logic's SMPTE display).
        let f = total.fract() * 25.0;
        let frames = f as u32;
        let sub = (f.fract() * 80.0) as u32;
        format!("{h:02}:{m:02}:{s:02}:{frames:02}.{sub:02}")
    }
}
