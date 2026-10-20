//! Project model. UI never mutates the engine directly (see AGENTS.md: Command pattern);
//! this is the UI-side view model until `prowave-engine` lands.
// Some fields (pan, instrument, Bus/Output kinds) are reserved for upcoming mixer/instrument work.
#![allow(dead_code)]
use egui::Color32;
use prowave_engine::AudioFile;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackKind {
    Instrument,
    Audio,
    Drummer,
    Bus,
    Output,
    /// Auxiliary channel strip: gets its sound from a bus, so it has no regions.
    Aux,
    /// Folder (track stack): holds the tracks nested under it and makes no sound itself.
    Folder,
}

impl TrackKind {
    /// Whether tracks of this kind hold regions (Aux and Folder tracks do not).
    pub fn has_regions(self) -> bool {
        matches!(self, Self::Instrument | Self::Audio | Self::Drummer)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub start: f32, // beats from region start
    pub len: f32,
    pub pitch: u8,
    pub vel: u8,
}

/// One pass of a cycle recording.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Take {
    pub notes: Vec<Note>,
    pub controls: Vec<Control>,
}

/// A stretch of a take folder (beats from the region start) that plays this take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompSegment {
    pub start: f32,
    pub end: f32,
    pub take: usize,
}

/// Logic's take folder: several takes of the same passage and the comp that plays one of
/// them in each stretch. See `takes.rs`.
#[derive(Clone, Debug, PartialEq)]
pub struct TakeFolder {
    pub takes: Vec<Take>,
    /// Sorted, without gaps or overlaps, covering `0..len`.
    pub comp: Vec<CompSegment>,
    /// Length of the folder in beats.
    pub len: f32,
    /// Shown opened, with a row for each take (not saved).
    pub open: bool,
}

/// A MIDI controller event in a region (the sustain pedal is controller 64).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Control {
    pub start: f32, // beats from region start
    pub num: u8,
    pub val: u8,
}

#[derive(Clone)]
pub struct Region {
    pub name: String,
    pub start_bar: f32,
    pub len_bars: f32,
    pub notes: Vec<Note>,
    /// Controller events recorded with the notes.
    pub controls: Vec<Control>,
    /// A take folder: the takes of a cycle recording and the comp that picks among them. The
    /// region's own `notes` and `controls` are then the comp, kept up to date by
    /// [`TakeFolder::apply`].
    pub folder: Option<TakeFolder>,
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
    /// Audio regions (Logic: Region Inspector Gain, Fade In / Out, Curve, Reverse), applied
    /// non-destructively by the engine. Gain in dB; fades in quarter-note beats.
    pub gain_db: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub fade_curve: prowave_engine::FadeCurve,
    pub reverse: bool,
}

/// Logic Pro's region gain range, in dB.
pub const MIN_REGION_GAIN: f32 = -96.0;
pub const MAX_REGION_GAIN: f32 = 24.0;

impl RegionParams {
    /// The audio edits as the engine applies them.
    pub fn clip_edit(&self) -> prowave_engine::ClipEdit {
        prowave_engine::ClipEdit {
            gain_db: self.gain_db,
            fade_in_beats: self.fade_in as f64,
            fade_out_beats: self.fade_out as f64,
            fade_curve: self.fade_curve,
            reverse: self.reverse,
        }
    }

    /// Region gain that brings a file whose loudest sample is `peak` (0..1) to 0 dBFS
    /// (Logic: Normalize Region Gain). `None` for a silent file.
    pub fn normalize_gain(peak: f32) -> Option<f32> {
        (peak.is_finite() && peak > 1e-6)
            .then(|| (-20.0 * peak.log10()).clamp(MIN_REGION_GAIN, MAX_REGION_GAIN))
    }
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
    /// Folder nesting level, 0 at the top. A track belongs to the closest `Folder` track above
    /// it with a lower level.
    pub indent: u8,
    /// A `Folder` track whose tracks are hidden in the Tracks area.
    pub folded: bool,
}

impl Track {
    /// Smallest and largest individual height factor.
    pub const MIN_HEIGHT: f32 = 0.5;
    pub const MAX_HEIGHT: f32 = 6.0;

    /// A new track of `kind` with default settings and no regions: software instruments get
    /// the Sampler, and every kind its own colour.
    pub fn new(name: impl Into<String>, kind: TrackKind) -> Self {
        let (color, instrument) = match kind {
            TrackKind::Instrument => (Color32::from_rgb(0x4e, 0xb0, 0x4a), Some("Sampler")),
            TrackKind::Drummer => (Color32::from_rgb(0xd9, 0xaa, 0x2a), Some("Drum Kit")),
            TrackKind::Audio => (Color32::from_rgb(0x3f, 0x5f, 0xc0), None),
            TrackKind::Aux => (Color32::from_rgb(0x2a, 0xa8, 0xa8), None),
            TrackKind::Folder => (Color32::from_rgb(0x88, 0x8c, 0x98), None),
            TrackKind::Bus | TrackKind::Output => (Color32::from_rgb(0x7a, 0x4f, 0xc0), None),
        };
        Self {
            name: name.into(),
            kind,
            color,
            mute: false,
            solo: false,
            rec: false,
            monitor: false,
            volume_db: 0.0,
            pan: 0.0,
            inserts: vec![],
            instrument,
            regions: vec![],
            params: TrackParams::default(),
            send: SendSlot::default(),
            category: None,
            height: 1.0,
            indent: 0,
            folded: false,
        }
    }
}

/// A tempo change on the Tempo track (Logic: a tempo point): `bpm` from `beat` (quarter notes
/// from the start) until the next change.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TempoPoint {
    pub beat: f64,
    pub bpm: f32,
}

/// A marker on the Marker track (Logic: Intro, Verse, Chorus...): from `beat` (quarter notes
/// from the start) until the next marker.
#[derive(Clone, PartialEq, Debug)]
pub struct Marker {
    pub beat: f64,
    pub name: String,
    pub color: Color32,
}

/// Colours offered for markers, in the order new markers take them.
pub const MARKER_COLORS: [Color32; 8] = [
    Color32::from_rgb(0x3f, 0x8f, 0xd8),
    Color32::from_rgb(0x4e, 0xb0, 0x4a),
    Color32::from_rgb(0xd9, 0xaa, 0x2a),
    Color32::from_rgb(0xd8, 0x6a, 0x2c),
    Color32::from_rgb(0xc8, 0x3c, 0x3c),
    Color32::from_rgb(0xb0, 0x4f, 0xb8),
    Color32::from_rgb(0x7a, 0x4f, 0xc0),
    Color32::from_rgb(0x3c, 0xa8, 0xa0),
];

/// Logic Pro's tempo range.
pub const MIN_BPM: f32 = 5.0;
pub const MAX_BPM: f32 = 990.0;

#[derive(Clone)]
pub struct Project {
    pub name: String,
    /// Tempo at the start of the song (the first point of the Tempo track).
    pub tempo: f32,
    /// Later tempo changes, sorted by beat, all after the start (see [`Project::tidy_tempo`]).
    pub tempo_changes: Vec<TempoPoint>,
    /// Marker track, sorted by beat (see [`Project::tidy_markers`]).
    pub markers: Vec<Marker>,
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
            indent: 0,
            folded: false,
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
                controls: Vec::new(),
                folder: None,
                audio: None,
                params: RegionParams::default(),
            }],
            params: TrackParams::default(),
            category: None,
            send: SendSlot::default(),
            height: 1.0,
            indent: 0,
            folded: false,
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
            tempo_changes: Vec::new(),
            markers: Vec::new(),
        }
    }

    /// Adds a marker at `beat` named "Marker <n>" (or replaces the name of one already there).
    /// Returns its index.
    pub fn add_marker(&mut self, beat: f64) -> usize {
        let beat = beat.max(0.0);
        if let Some(i) = self
            .markers
            .iter()
            .position(|m| (m.beat - beat).abs() <= 1e-6)
        {
            return i;
        }
        let n = (1..)
            .find(|n| !self.markers.iter().any(|m| m.name == format!("Marker {n}")))
            .unwrap_or(1);
        let i = self.markers.partition_point(|m| m.beat < beat);
        let color = MARKER_COLORS[self.markers.len() % MARKER_COLORS.len()];
        self.markers.insert(
            i,
            Marker {
                beat,
                name: format!("Marker {n}"),
                color,
            },
        );
        i
    }

    /// The marker the playhead at `beat` is in (the last one at or before it).
    pub fn marker_at(&self, beat: f64) -> Option<usize> {
        let n = self.markers.partition_point(|m| m.beat <= beat + 1e-6);
        n.checked_sub(1)
    }

    /// Start of the first marker after `beat`.
    pub fn next_marker(&self, beat: f64) -> Option<f64> {
        self.markers
            .iter()
            .map(|m| m.beat)
            .find(|b| *b > beat + 1e-6)
    }

    /// Start of the last marker before `beat`.
    pub fn previous_marker(&self, beat: f64) -> Option<f64> {
        self.markers
            .iter()
            .rev()
            .map(|m| m.beat)
            .find(|b| *b < beat - 1e-6)
    }

    /// Sorts the markers and drops invalid ones (negative or non-finite positions, a second
    /// marker on the same beat).
    pub fn tidy_markers(&mut self) {
        self.markers.retain(|m| m.beat.is_finite() && m.beat >= 0.0);
        self.markers.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        self.markers
            .dedup_by(|b, a| (a.beat - b.beat).abs() <= 1e-6);
    }

    /// The tempo in effect at `beat`.
    pub fn tempo_at(&self, beat: f64) -> f32 {
        self.tempo_changes
            .iter()
            .take_while(|t| t.beat <= beat)
            .last()
            .map_or(self.tempo, |t| t.bpm)
    }

    /// Sets the tempo in effect at `beat` (the change before it, or the start tempo), as
    /// dragging the tempo in the control bar does in Logic.
    pub fn set_tempo_at(&mut self, beat: f64, bpm: f32) {
        let bpm = bpm.clamp(MIN_BPM, MAX_BPM);
        match self.tempo_changes.iter_mut().rev().find(|t| t.beat <= beat) {
            Some(t) => t.bpm = bpm,
            None => self.tempo = bpm,
        }
    }

    /// Adds a tempo change at `beat` (replacing one already there). Returns its index.
    pub fn add_tempo_change(&mut self, beat: f64, bpm: f32) -> usize {
        let beat = beat.max(0.0);
        let bpm = bpm.clamp(MIN_BPM, MAX_BPM);
        if beat <= 0.0 {
            self.tempo = bpm;
            return usize::MAX;
        }
        self.tempo_changes.retain(|t| (t.beat - beat).abs() > 1e-6);
        let i = self.tempo_changes.partition_point(|t| t.beat < beat);
        self.tempo_changes.insert(i, TempoPoint { beat, bpm });
        i
    }

    /// Sorts the tempo changes and drops invalid ones: at or before the start, duplicates of a
    /// beat (the last one wins) and non-finite values.
    pub fn tidy_tempo(&mut self) {
        self.tempo_changes
            .retain(|t| t.beat.is_finite() && t.beat > 1e-6 && t.bpm.is_finite());
        for t in &mut self.tempo_changes {
            t.bpm = t.bpm.clamp(MIN_BPM, MAX_BPM);
        }
        self.tempo_changes.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        let mut out: Vec<TempoPoint> = Vec::with_capacity(self.tempo_changes.len());
        for t in self.tempo_changes.drain(..) {
            match out.last_mut() {
                Some(last) if (last.beat - t.beat).abs() <= 1e-6 => *last = t,
                _ => out.push(t),
            }
        }
        self.tempo_changes = out;
    }

    /// The tempo and signature as the engine's tempo map.
    pub fn tempo_map(&self) -> prowave_engine::TempoMap {
        use prowave_engine::{SignatureEvent, TempoCurve, TempoEvent};
        let tempos = std::iter::once(TempoEvent {
            beat: 0.0,
            bpm: self.tempo,
            curve: TempoCurve::Step,
        })
        .chain(self.tempo_changes.iter().map(|t| TempoEvent {
            beat: t.beat,
            bpm: t.bpm,
            curve: TempoCurve::Step,
        }))
        .collect();
        let sigs = vec![SignatureEvent {
            bar: 0,
            num: self.sig.0,
            den: self.sig.1,
        }];
        prowave_engine::TempoMap::new(tempos, sigs)
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

    /// Appends a new track of `kind` (named "Aux 1", "Inst 3"...), selects it and tells the
    /// engine the song changed. Returns its index.
    pub fn add_track(&mut self, kind: TrackKind) -> usize {
        let n = self.tracks.iter().filter(|t| t.kind == kind).count() + 1;
        let name = match kind {
            TrackKind::Instrument => format!("Inst {n}"),
            TrackKind::Audio => format!("Audio {n}"),
            TrackKind::Drummer => format!("Drummer {n}"),
            TrackKind::Aux => format!("Aux {n}"),
            TrackKind::Folder => format!("Folder {n}"),
            TrackKind::Bus => format!("Bus {n}"),
            TrackKind::Output => format!("Output {n}"),
        };
        self.tracks.push(Track::new(name, kind));
        self.selected_track = self.tracks.len() - 1;
        self.song_rev += 1;
        self.selected_track
    }

    /// The channel name the Inspector shows for track `i`: "Audio 2", "Aux 1" or "Inst 3",
    /// counting the tracks of the same class up to and including it.
    pub fn channel_name(&self, i: usize) -> String {
        let class = |k: TrackKind| match k {
            TrackKind::Audio => 0,
            TrackKind::Aux => 1,
            _ => 2,
        };
        let c = class(self.tracks[i].kind);
        let nth = self.tracks[..=i]
            .iter()
            .filter(|t| class(t.kind) == c)
            .count();
        let label = ["Audio", "Aux", "Inst"][c];
        format!("{label} {nth}")
    }

    /// Index just past the last track inside folder `i` (a track with a higher nesting level
    /// than the folder, directly after it). For a track that is not a folder this is `i + 1`.
    pub fn folder_end(&self, i: usize) -> usize {
        let level = self.tracks[i].indent;
        let mut end = i + 1;
        while end < self.tracks.len() && self.tracks[end].indent > level {
            end += 1;
        }
        end
    }

    /// Whether track `i` is hidden because a folder around it is folded.
    pub fn hidden_by_fold(&self, i: usize) -> bool {
        let mut level = self.tracks[i].indent;
        for j in (0..i).rev() {
            let t = &self.tracks[j];
            if t.indent < level {
                if t.kind == TrackKind::Folder && t.folded {
                    return true;
                }
                level = t.indent;
            }
        }
        false
    }

    /// Track > Create Track Stack: puts track `i` (with everything inside it, if it is a
    /// folder) into a new folder at its place, selects the folder and returns its index.
    /// Tracks after it move down one slot, so the engine is told the song changed.
    pub fn create_track_stack(&mut self, i: usize) -> usize {
        let (end, level) = (self.folder_end(i), self.tracks[i].indent);
        for t in &mut self.tracks[i..end] {
            t.indent = t.indent.saturating_add(1);
        }
        let n = self
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Folder)
            .count()
            + 1;
        let mut folder = Track::new(format!("Folder {n}"), TrackKind::Folder);
        folder.indent = level;
        self.tracks.insert(i, folder);
        self.selected_track = i;
        self.song_rev += 1;
        i
    }

    /// Track > Release Track Stack: removes folder `i` and lifts the tracks inside it one
    /// level. Returns false (and changes nothing) when `i` is not a folder.
    pub fn release_track_stack(&mut self, i: usize) -> bool {
        if self
            .tracks
            .get(i)
            .is_none_or(|t| t.kind != TrackKind::Folder)
        {
            return false;
        }
        let end = self.folder_end(i);
        for t in &mut self.tracks[i + 1..end] {
            t.indent = t.indent.saturating_sub(1);
        }
        self.tracks.remove(i);
        if self.selected_track > i {
            self.selected_track -= 1;
        }
        self.selected_track = self.selected_track.min(self.tracks.len().saturating_sub(1));
        self.song_rev += 1;
        true
    }

    /// Track > Delete Track: removes track `i` with its regions. Deleting a folder keeps the
    /// tracks inside it (as Release Track Stack does). The last track cannot be deleted, so
    /// there is always a selected track; returns false then and when `i` does not exist.
    pub fn delete_track(&mut self, i: usize) -> bool {
        if i >= self.tracks.len() || self.tracks.len() == 1 {
            return false;
        }
        if self.tracks[i].kind == TrackKind::Folder {
            return self.release_track_stack(i);
        }
        self.tracks.remove(i);
        if self.selected_track > i {
            self.selected_track -= 1;
        }
        self.selected_track = self.selected_track.min(self.tracks.len() - 1);
        self.normalize_nesting();
        self.song_rev += 1;
        true
    }

    /// Sets Mute and/or Solo on folder `i` and on every track inside it, as a folder's M and S
    /// buttons do.
    pub fn set_folder_flags(&mut self, i: usize, mute: Option<bool>, solo: Option<bool>) {
        let end = self.folder_end(i);
        for t in &mut self.tracks[i..end] {
            t.mute = mute.unwrap_or(t.mute);
            t.solo = solo.unwrap_or(t.solo);
        }
    }

    /// Repairs folder nesting: a track can sit at most one level below the track before it,
    /// and only when that one is a `Folder`. Run after loading a file that may be hand-edited.
    pub fn normalize_nesting(&mut self) {
        let mut allowed = 0u8;
        for t in &mut self.tracks {
            t.indent = t.indent.min(allowed);
            allowed = t.indent + u8::from(t.kind == TrackKind::Folder);
        }
    }

    /// Moves one half of the time signature by `steps` (up is positive) and returns whether it
    /// changed. The numerator runs 1-99, the denominator through the powers of two 1-64; both
    /// stop at their ends. Regions sit on bars, so a change moves them in time: the song
    /// revision is bumped to have the engine reposition them.
    pub fn step_signature(&mut self, part: SigPart, steps: i32) -> bool {
        let before = self.sig;
        match part {
            SigPart::Numerator => self.sig.0 = (i32::from(self.sig.0) + steps).clamp(1, 99) as u8,
            SigPart::Denominator => {
                let at = SIGNATURE_DENOMINATORS
                    .iter()
                    .position(|d| *d == self.sig.1)
                    .unwrap_or(2);
                let last = SIGNATURE_DENOMINATORS.len() as i32 - 1;
                let to = (at as i32 + steps).clamp(0, last) as usize;
                self.sig.1 = SIGNATURE_DENOMINATORS[to];
            }
        }
        let changed = self.sig != before;
        if changed {
            self.song_rev += 1;
        }
        changed
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

/// The beat lengths a time signature can have (its denominator), as Logic offers them.
pub const SIGNATURE_DENOMINATORS: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];

/// One half of a time signature.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SigPart {
    Numerator,
    Denominator,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_parts_step_and_stop_at_their_ends() {
        let mut p = Project::demo();
        assert_eq!(p.sig, (4, 4));
        assert!(p.step_signature(SigPart::Numerator, 2));
        assert!(p.step_signature(SigPart::Denominator, 1));
        assert_eq!(p.sig, (6, 8), "the denominator steps through powers of two");
        assert!(p.step_signature(SigPart::Denominator, 3));
        assert_eq!(p.sig, (6, 64));
        assert!(!p.step_signature(SigPart::Denominator, 5), "stops at 64");
        assert!(p.step_signature(SigPart::Denominator, -9));
        assert_eq!(p.sig.1, 1, "and at 1");
        p.step_signature(SigPart::Numerator, 500);
        assert_eq!(p.sig.0, 99);
        p.step_signature(SigPart::Numerator, -500);
        assert_eq!(p.sig.0, 1);
    }

    #[test]
    fn a_changed_signature_moves_bars_and_asks_the_engine_to_follow() {
        let mut p = Project::demo();
        let rev = p.song_rev;
        assert_eq!(p.beats_per_bar(), 4.0);
        assert!(p.step_signature(SigPart::Numerator, -1));
        assert!(p.step_signature(SigPart::Denominator, 1));
        assert_eq!(p.sig, (3, 8));
        assert_eq!(p.beats_per_bar(), 1.5);
        assert_eq!(p.song_rev, rev + 2);

        // Nothing changes at an end stop, so there is nothing to resend.
        let mut q = Project::demo();
        let rev = q.song_rev;
        assert!(!q.step_signature(SigPart::Numerator, 0));
        assert_eq!(q.song_rev, rev);
    }

    #[test]
    fn new_tracks_have_the_defaults_of_their_kind() {
        let inst = Track::new("Inst 1", TrackKind::Instrument);
        assert_eq!((inst.instrument, inst.regions.len()), (Some("Sampler"), 0));
        for kind in [TrackKind::Aux, TrackKind::Folder] {
            let t = Track::new("x", kind);
            assert!(t.instrument.is_none() && !kind.has_regions());
            assert_eq!(
                (t.indent, t.folded, t.volume_db, t.height),
                (0, false, 0.0, 1.0)
            );
        }
        assert!(TrackKind::Audio.has_regions() && TrackKind::Drummer.has_regions());
    }

    #[test]
    fn deleting_tracks_keeps_the_selection_valid_and_never_empties_the_project() {
        let mut p = Project::demo();
        let n = p.tracks.len();
        p.selected_track = n - 1;
        let rev = p.song_rev;
        assert!(p.delete_track(1));
        assert_eq!(p.tracks.len(), n - 1);
        assert_eq!(p.selected_track, n - 2, "the selection follows its track");
        assert_ne!(p.song_rev, rev);
        assert!(!p.delete_track(99));

        // A folder goes, the tracks inside it stay.
        let stacked = p.tracks[0].name.clone();
        p.create_track_stack(0);
        assert!(p.delete_track(0));
        assert_eq!(
            (p.tracks[0].name.as_str(), p.tracks[0].indent),
            (stacked.as_str(), 0)
        );

        while p.tracks.len() > 1 {
            assert!(p.delete_track(0));
        }
        assert!(!p.delete_track(0), "the last track stays");
        assert_eq!(p.selected_track, 0);
    }

    #[test]
    fn nesting_is_repaired_to_what_the_folders_allow() {
        let mut p = Project::demo();
        let (inst, folder) = (TrackKind::Instrument, TrackKind::Folder);
        let at = |k, indent| {
            let mut t = Track::new("t", k);
            t.indent = indent;
            t
        };
        p.tracks = vec![
            at(inst, 5), // first track: nothing above it
            at(folder, 0),
            at(inst, 9), // right under a folder: at most one level
            at(folder, 1),
            at(inst, 2), // nested folder: two levels is fine
            at(inst, 7), // after a plain track: no deeper than it
        ];
        p.normalize_nesting();
        let got: Vec<u8> = p.tracks.iter().map(|t| t.indent).collect();
        assert_eq!(got, [0, 0, 1, 1, 2, 2]);
    }
    #[test]
    fn added_tracks_are_named_per_kind_selected_and_announced() {
        let mut p = Project::demo();
        let (n, rev) = (p.tracks.len(), p.song_rev);
        let aux = p.add_track(TrackKind::Aux);
        assert_eq!((aux, p.selected_track, p.tracks.len()), (n, n, n + 1));
        assert_eq!(p.tracks[aux].name, "Aux 1");
        assert_eq!(p.tracks[aux].kind, TrackKind::Aux);
        assert!(p.tracks[aux].regions.is_empty());
        assert_ne!(p.song_rev, rev, "the engine is told the track list changed");

        p.add_track(TrackKind::Aux);
        assert_eq!(p.tracks.last().unwrap().name, "Aux 2");
        p.add_track(TrackKind::Instrument);
        assert_eq!(
            p.tracks.last().unwrap().name,
            "Inst 4",
            "the demo has 3 instruments"
        );
    }

    #[test]
    fn channel_names_count_within_their_class() {
        let mut p = Project::demo();
        let a1 = p.add_track(TrackKind::Aux);
        let a2 = p.add_track(TrackKind::Aux);
        let audio = p
            .tracks
            .iter()
            .position(|t| t.kind == TrackKind::Audio)
            .unwrap();
        assert_eq!(p.channel_name(a1), "Aux 1");
        assert_eq!(p.channel_name(a2), "Aux 2");
        assert_eq!(p.channel_name(audio), "Audio 1");
        assert_eq!(p.channel_name(0), "Inst 1");
    }

    /// Tracks "a b c d e": b and c in a folder (a), d outside, e in a nested folder inside d.
    fn stack_fixture() -> Project {
        let mut p = Project::demo();
        let k = |name: &str, kind, indent| {
            let mut t = Track::new(name, kind);
            t.indent = indent;
            t
        };
        let (inst, folder) = (TrackKind::Instrument, TrackKind::Folder);
        p.tracks = vec![
            k("F", folder, 0),
            k("b", inst, 1),
            k("c", inst, 1),
            k("d", inst, 0),
            k("G", folder, 0),
            k("e", inst, 1),
        ];
        p
    }

    #[test]
    fn folders_own_the_deeper_tracks_after_them() {
        let p = stack_fixture();
        assert_eq!(p.folder_end(0), 3);
        assert_eq!(p.folder_end(4), 6);
        assert_eq!(p.folder_end(3), 4, "a plain track is its own range");
    }

    #[test]
    fn folding_hides_exactly_the_tracks_inside() {
        let mut p = stack_fixture();
        let hidden = |p: &Project| {
            (0..p.tracks.len())
                .filter(|i| p.hidden_by_fold(*i))
                .collect::<Vec<_>>()
        };
        assert!(hidden(&p).is_empty());
        p.tracks[0].folded = true;
        assert_eq!(hidden(&p), [1, 2], "the folder itself and the others stay");
        p.tracks[4].folded = true;
        assert_eq!(hidden(&p), [1, 2, 5]);
        // A folded outer folder hides an open inner one's tracks as well.
        let mut p = stack_fixture();
        p.tracks[0].folded = false;
        p.create_track_stack(0);
        p.tracks[0].folded = true;
        assert_eq!((1..4).filter(|i| p.hidden_by_fold(*i)).count(), 3);
    }

    #[test]
    fn a_track_stack_wraps_the_track_and_can_be_released_again() {
        let mut p = stack_fixture();
        let (rev, names) = (p.song_rev, |p: &Project| {
            p.tracks
                .iter()
                .map(|t| (t.name.clone(), t.indent))
                .collect::<Vec<_>>()
        });
        let before = names(&p);

        // Wrap the plain track "d" (index 3).
        let at = p.create_track_stack(3);
        assert_eq!(
            (at, p.selected_track, p.tracks[3].kind),
            (3, 3, TrackKind::Folder)
        );
        assert_eq!(p.tracks[3].name, "Folder 3");
        assert_eq!((p.tracks[4].name.as_str(), p.tracks[4].indent), ("d", 1));
        assert_ne!(p.song_rev, rev);

        assert!(p.release_track_stack(3));
        assert_eq!(names(&p), before, "release undoes create");
        assert!(!p.release_track_stack(3), "d is not a folder");

        // Wrapping a folder takes everything inside it one level deeper.
        p.create_track_stack(0);
        let nested: Vec<u8> = p.tracks[..4].iter().map(|t| t.indent).collect();
        assert_eq!(nested, [0, 1, 2, 2]);
        assert_eq!(p.folder_end(0), 4);
    }

    #[test]
    fn a_folders_mute_and_solo_reach_every_track_inside_it() {
        let mut p = stack_fixture();
        p.set_folder_flags(0, Some(true), None);
        let muted: Vec<bool> = p.tracks.iter().map(|t| t.mute).collect();
        assert_eq!(muted, [true, true, true, false, false, false]);
        p.set_folder_flags(0, None, Some(true));
        assert!(p.tracks[..3].iter().all(|t| t.solo && t.mute));
        assert!(!p.tracks[3].solo, "tracks outside the folder are untouched");
        p.set_folder_flags(0, Some(false), Some(false));
        assert!(p.tracks.iter().all(|t| !t.mute && !t.solo));
    }

    #[test]
    fn an_invalid_denominator_steps_from_quarter_notes() {
        let mut p = Project::demo();
        p.sig = (4, 3);
        assert!(p.step_signature(SigPart::Denominator, 1));
        assert_eq!(p.sig.1, 8);
    }

    #[test]
    fn tempo_changes_hold_until_the_next_one() {
        let mut p = Project::empty();
        p.tempo = 120.0;
        p.add_tempo_change(16.0, 90.0);
        p.add_tempo_change(8.0, 100.0);
        assert_eq!(p.tempo_at(0.0), 120.0);
        assert_eq!(p.tempo_at(8.0), 100.0);
        assert_eq!(p.tempo_at(15.9), 100.0);
        assert_eq!(p.tempo_at(400.0), 90.0);
        // Adding at the same beat replaces; at the start sets the start tempo.
        p.add_tempo_change(8.0, 110.0);
        assert_eq!(p.tempo_changes.len(), 2);
        p.add_tempo_change(0.0, 60.0);
        assert_eq!((p.tempo, p.tempo_changes.len()), (60.0, 2));
        // Dragging the LCD tempo edits the change in effect at the playhead.
        p.set_tempo_at(20.0, 95.0);
        assert_eq!(p.tempo_changes[1].bpm, 95.0);
        p.set_tempo_at(3.0, 70.0);
        assert_eq!(p.tempo, 70.0);
    }

    #[test]
    fn the_engine_tempo_map_follows_the_tempo_track() {
        let mut p = Project::empty();
        p.tempo = 120.0;
        p.add_tempo_change(8.0, 60.0);
        let map = p.tempo_map();
        // 8 beats at 120 = 4 s, then 4 beats at 60 = 4 s.
        assert!((map.seconds_at(8.0) - 4.0).abs() < 1e-9);
        assert!((map.seconds_at(12.0) - 8.0).abs() < 1e-9);
        assert_eq!(map.tempo_at(10.0), 60.0);
    }

    #[test]
    fn markers_are_named_kept_in_order_and_navigable() {
        let mut p = Project::empty();
        assert_eq!(p.add_marker(16.0), 0);
        assert_eq!(p.add_marker(4.0), 0, "sorted by position");
        assert_eq!(p.add_marker(16.0), 1, "one marker per position");
        let names: Vec<&str> = p.markers.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Marker 2", "Marker 1"]);
        assert_ne!(p.markers[0].color, p.markers[1].color);
        assert_eq!(p.marker_at(2.0), None);
        assert_eq!(p.marker_at(4.0), Some(0));
        assert_eq!(p.marker_at(30.0), Some(1));
        assert_eq!(p.next_marker(4.0), Some(16.0));
        assert_eq!(p.next_marker(16.0), None);
        assert_eq!(p.previous_marker(16.0), Some(4.0));
        assert_eq!(p.previous_marker(4.0), None);
        // A deleted name is reused.
        p.markers.remove(1);
        p.add_marker(32.0);
        assert_eq!(p.markers[1].name, "Marker 1");
    }

    #[test]
    fn tidy_markers_sorts_and_drops_bad_or_doubled_ones() {
        let mut p = Project::empty();
        for b in [8.0, -1.0, f64::NAN, 2.0, 8.0] {
            p.markers.push(Marker {
                beat: b,
                name: format!("{b}"),
                color: MARKER_COLORS[0],
            });
        }
        p.tidy_markers();
        let beats: Vec<f64> = p.markers.iter().map(|m| m.beat).collect();
        assert_eq!(beats, [2.0, 8.0]);
    }

    #[test]
    fn normalize_brings_the_peak_to_full_scale() {
        let g = RegionParams::normalize_gain(0.5).unwrap();
        assert!((g - 6.0206).abs() < 1e-3, "half scale needs +6 dB, got {g}");
        assert_eq!(RegionParams::normalize_gain(1.0), Some(0.0));
        assert_eq!(RegionParams::normalize_gain(0.0), None, "silence");
        assert_eq!(RegionParams::normalize_gain(f32::NAN), None);
        assert_eq!(
            RegionParams::normalize_gain(1e-5),
            Some(MAX_REGION_GAIN),
            "never past +24 dB"
        );
        let edit = RegionParams {
            gain_db: -2.0,
            fade_in: 1.5,
            reverse: true,
            ..RegionParams::default()
        }
        .clip_edit();
        assert_eq!(
            (edit.gain_db, edit.fade_in_beats, edit.reverse),
            (-2.0, 1.5, true)
        );
    }
}
