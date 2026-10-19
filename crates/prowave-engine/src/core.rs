//! Device-independent engine core. `EngineCore::process` is the audio-thread entry point:
//! no allocation, no locks, no syscalls. Unit-testable without an audio device.
use crate::disk::{Block, ClipShared};
use crate::gain_smoothing::GainRamp;
use crate::synth::{kit_voice, osc, Kit, Svf, SynthPatch};
use crate::tempo_map::TempoMap;
use rtrb::{Consumer, Producer};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

pub const MAX_TRACKS: usize = 64;
const MAX_VOICES: usize = 48;

/// A note scheduled on the timeline (beats are 0-based quarter notes).
#[derive(Clone, Copy, Debug)]
pub struct NoteEvent {
    pub start_beat: f64,
    pub end_beat: f64,
    pub pitch: u8,
    pub vel: u8,
    pub track: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackMix {
    pub gain_db: f32,
    pub mute: bool,
    pub solo: bool,
    /// -1.0 (left) .. 1.0 (right), equal-power, unity at centre.
    pub pan: f32,
}

impl Default for TrackMix {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            mute: false,
            solo: false,
            pan: 0.0,
        }
    }
}

/// Immutable-by-the-audio-thread snapshot of the arrangement.
#[derive(Default)]
pub struct Song {
    /// Must be sorted by `start_beat`; use [`Song::new`].
    pub notes: Vec<NoteEvent>,
    pub tracks: Vec<TrackMix>,
    /// Streamed audio regions (create with `Engine::stream_clip`).
    pub clips: Vec<AudioClip>,
}

impl Song {
    pub fn new(mut notes: Vec<NoteEvent>, tracks: Vec<TrackMix>) -> Self {
        notes.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
        Self {
            notes,
            tracks,
            clips: Vec::new(),
        }
    }

    pub fn with_clips(mut self, clips: Vec<AudioClip>) -> Self {
        self.clips = clips;
        self
    }
}

/// Audio-thread side of a streamed audio region (Logic Pro: audio region on an audio track).
/// Reads [`Block`]s produced by the disk thread; never allocates or blocks.
pub struct AudioClip {
    pub track: u8,
    pub start_beat: f64,
    /// Length in device-rate frames.
    pub len_frames: u64,
    rx: Consumer<Block>,
    shared: Arc<ClipShared>,
    gen: u16,
    cur: Option<Block>,
    /// Next clip-relative frame to play (negative = before the region starts).
    pos: i64,
    underruns: u32,
}

impl AudioClip {
    pub(crate) fn new(
        track: u8,
        start_beat: f64,
        len_frames: u64,
        rx: Consumer<Block>,
        shared: Arc<ClipShared>,
    ) -> Self {
        Self {
            track,
            start_beat,
            len_frames,
            rx,
            shared,
            gen: 0,
            cur: None,
            pos: 0,
            underruns: 0,
        }
    }

    /// Continue playback `offset` frames into the clip (negative: before it starts).
    fn reposition(&mut self, offset: i64) {
        self.pos = offset;
        self.gen = self.gen.wrapping_add(1);
        self.cur = None;
        let want = self.pos.clamp(0, self.len_frames as i64) as u64;
        self.shared.request(self.gen, want);
    }

    /// Once per callback: drop stale blocks so the disk thread gets ring space back.
    fn service(&mut self) {
        while self.cur.as_ref().is_none_or(|b| b.gen != self.gen) {
            match self.rx.pop() {
                Ok(b) if b.gen == self.gen => self.cur = Some(b),
                Ok(_) => self.cur = None,
                Err(_) => break,
            }
        }
    }

    fn tick(&mut self) -> Option<(f32, f32)> {
        let p = self.pos;
        self.pos += 1;
        if p < 0 || p as u64 >= self.len_frames {
            return None;
        }
        let p = p as u64;
        loop {
            if let Some(b) = &self.cur {
                if b.gen == self.gen {
                    if p < b.start {
                        return None; // gap: data for later frames already queued
                    }
                    if p < b.start + b.frames as u64 {
                        let i = (p - b.start) as usize * 2;
                        return Some((b.data[i], b.data[i + 1]));
                    }
                }
                self.cur = None;
            }
            match self.rx.pop() {
                Ok(b) if b.gen == self.gen => self.cur = Some(b),
                Ok(_) => {}
                Err(_) => {
                    self.underruns += 1;
                    return None;
                }
            }
        }
    }
}

/// UI -> engine messages. Heap payloads are boxed so the queue slot stays small; the old
/// payload travels back through the garbage queue so it is freed off the audio thread.
pub enum Command {
    Play,
    Pause,
    /// Stop and return to the position the transport started from.
    Stop,
    SetPosition {
        beat: f64,
    },
    /// Tempo and time-signature changes (Logic: Tempo / Signature tracks). Build the map on
    /// the UI thread; the old one is sent back through the garbage queue.
    SetTempoMap(Box<TempoMap>),
    SetMetronome {
        on: bool,
    },
    /// Master fader (Logic: Stereo Out level), clamped to 0..2 linear. Changes ramp over
    /// 5 ms during playback/audition; silent stopped setup is immediate. Non-finite values
    /// are ignored so malformed commands cannot poison the output.
    SetMasterGain {
        linear: f32,
    },
    /// Logic's cycle locators, in quarter-note beats. The end is exclusive; wrapping
    /// preserves fractional-sample elapsed time. Invalid or non-finite bounds disable cycle.
    SetCycle {
        on: bool,
        start_beat: f64,
        end_beat: f64,
    },
    /// Track fader/pan (Logic: channel strip). Gain and pan ramp over 5 ms while sounding;
    /// mute and solo retain immediate switching. New song snapshots initialize immediately.
    SetTrackMix {
        track: u8,
        mix: TrackMix,
    },
    SetSong(Box<Song>),
    /// Replaces only the song's notes (sorted by start; build with [`Command::set_notes`]).
    /// Streamed audio clips keep playing and sounding voices are not cut, so MIDI edits while
    /// playing cause no audio dropout. Use `SetSong` when clips or the track count change.
    SetNotes(Vec<NoteEvent>),
    /// Logic Pro "Test Oscillator": a sine on the output, independent of the transport.
    /// `hz <= 0` turns it off.
    SetTestTone {
        hz: f32,
        gain: f32,
    },
    /// Live note (clicking the Piano Roll keys, auditioning while editing). Works while the
    /// transport is stopped and is released by `NoteOff` (or automatically after 4 s).
    NoteOn {
        pitch: u8,
        vel: u8,
        track: u8,
    },
    /// Releases this live pitch on every track (legacy/global audition release).
    NoteOff {
        pitch: u8,
    },
    /// Releases a live pitch only on its instrument track, like Logic's per-track MIDI
    /// routing. Other tracks and timeline notes are unaffected; unknown tracks are ignored.
    NoteOffTrack {
        pitch: u8,
        track: u8,
    },
    /// Sets the synth patch used by instrument notes on `track` (Library patch).
    SetTrackPatch {
        track: u8,
        patch: SynthPatch,
    },
}

impl Command {
    pub(crate) fn retires_state(&self) -> bool {
        matches!(
            self,
            Self::SetSong(_) | Self::SetNotes(_) | Self::SetTempoMap(_)
        )
    }

    /// [`Command::SetNotes`] with `notes` sorted as the engine requires.
    pub fn set_notes(mut notes: Vec<NoteEvent>) -> Self {
        notes.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
        Command::SetNotes(notes)
    }
}

pub enum Garbage {
    Song(Box<Song>),
    Notes(Vec<NoteEvent>),
    TempoMap(Box<TempoMap>),
}

/// Engine -> UI state, all lock-free.
pub struct Shared {
    pub position_beats: AtomicU64, // f64 bits
    pub playing: AtomicBool,
    pub peak_l: AtomicU32, // f32 bits
    pub peak_r: AtomicU32,
    pub xruns: AtomicU32,
    /// Callbacks in which a streamed clip had no data ready (disk too slow).
    pub underruns: AtomicU32,
    /// Device-reported delay from callback to DAC, in microseconds (0 if unknown).
    pub output_latency_us: AtomicU32,
    /// Frames in the most recent device callback.
    pub block_frames: AtomicU32,
    /// Post-fader stereo peak per track (f32 bits), for the mixer meters.
    pub track_peak_l: [AtomicU32; MAX_TRACKS],
    pub track_peak_r: [AtomicU32; MAX_TRACKS],
    /// Highest DSP load (f32 bits, 1.0 = the whole callback budget) since the UI last read it.
    dsp_load_max: AtomicU32,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            position_beats: AtomicU64::new(0f64.to_bits()),
            playing: AtomicBool::new(false),
            peak_l: AtomicU32::new(0),
            peak_r: AtomicU32::new(0),
            xruns: AtomicU32::new(0),
            underruns: AtomicU32::new(0),
            output_latency_us: AtomicU32::new(0),
            block_frames: AtomicU32::new(0),
            track_peak_l: std::array::from_fn(|_| AtomicU32::new(0)),
            track_peak_r: std::array::from_fn(|_| AtomicU32::new(0)),
            dsp_load_max: AtomicU32::new(0),
        }
    }

    /// Audio thread: report one callback's DSP load (see [`dsp_load`]). Lock- and alloc-free.
    pub fn record_dsp_load(&self, load: f32) {
        // Non-negative f32s order like their bit patterns, so an integer max works.
        self.dsp_load_max
            .fetch_max(load.max(0.0).to_bits(), Ordering::Relaxed);
    }

    /// UI thread: the highest DSP load since the previous call (Logic's CPU meter), then reset.
    pub fn take_dsp_load(&self) -> f32 {
        f32::from_bits(self.dsp_load_max.swap(0, Ordering::Relaxed))
    }

    /// Post-fader (left, right) peak of `track`, linear.
    pub fn track_peaks(&self, track: usize) -> (f32, f32) {
        match (self.track_peak_l.get(track), self.track_peak_r.get(track)) {
            (Some(l), Some(r)) => (
                f32::from_bits(l.load(Ordering::Relaxed)),
                f32::from_bits(r.load(Ordering::Relaxed)),
            ),
            _ => (0.0, 0.0),
        }
    }
    pub fn position(&self) -> f64 {
        f64::from_bits(self.position_beats.load(Ordering::Relaxed))
    }
    pub fn peaks(&self) -> (f32, f32) {
        (
            f32::from_bits(self.peak_l.load(Ordering::Relaxed)),
            f32::from_bits(self.peak_r.load(Ordering::Relaxed)),
        )
    }
}

/// Share of the real-time budget a callback used: time spent rendering `frames` divided by
/// the time those frames take to play. Above 1.0 the device underruns (Logic: "System
/// Overload").
pub fn dsp_load(busy: std::time::Duration, frames: usize, sample_rate: u32) -> f32 {
    if frames == 0 || sample_rate == 0 {
        return 0.0;
    }
    (busy.as_secs_f64() * sample_rate as f64 / frames as f64) as f32
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    releasing: bool,
    phase_a: f32,
    phase_b: f32,
    base_hz: f32,
    amp: f32,
    env: f32,
    /// 0 attack, 1 decay, 2 sustain, 3 release.
    stage: u8,
    rel_step: f32,
    end_beat: f64,
    track: u8,
    /// Live note: ignores the transport and the timeline, ends on `NoteOff`.
    live: bool,
    pitch: u8,
    /// Safety net so a lost `NoteOff` cannot hang a note forever.
    live_left: u32,
    patch: SynthPatch,
    filter: Svf,
    vibrato: f32,
    noise: u32,
    pitch_env: f32,
    /// Oscillator B / A frequency ratio (fixed per note).
    ratio_b: f32,
    /// `pitch_env` decay over one control block.
    pitch_decay_block: f32,
    /// Samples until the control-rate values below are recomputed.
    ctrl_left: u32,
    dt_a: f32,
    dt_b: f32,
    filter_f: f32,
    filter_damping: f32,
    /// Envelope increments per sample for the attack and decay stages (fixed per note).
    attack_step: f32,
    decay_step: f32,
}

/// Modulation (vibrato, pitch drop, filter envelope) is recomputed every `CTRL_BLOCK` samples
/// (0.33 ms at 48 kHz) instead of every sample: it removes the per-sample `powf`/`sin` calls,
/// the bulk of the synth's cost, without audible stepping.
const CTRL_BLOCK: u32 = 16;

pub struct EngineCore {
    sr: f32,
    rx: Consumer<Command>,
    trash: Producer<Garbage>,
    shared: Arc<Shared>,
    song: Box<Song>,
    next_note: usize,
    beat: f64,
    start_beat: f64,
    playing: bool,
    tempo_map: Box<TempoMap>,
    metronome: bool,
    cycle: Option<(f64, f64)>,
    master: GainRamp,
    voices: [Voice; MAX_VOICES],
    click_remaining: u32,
    click_phase: f32,
    click_freq: f32,
    track_gain_l: [f32; MAX_TRACKS],
    track_gain_r: [f32; MAX_TRACKS],
    track_ramp_l: [GainRamp; MAX_TRACKS],
    track_ramp_r: [GainRamp; MAX_TRACKS],
    mix_ramp_remaining: u32,
    track_pk_l: [f32; MAX_TRACKS],
    track_pk_r: [f32; MAX_TRACKS],
    track_audible: [bool; MAX_TRACKS],
    track_patch: [SynthPatch; MAX_TRACKS],
    test_hz: f32,
    test_gain: f32,
    test_phase: f32,
}

/// Keeps an oscillator phase in 0..1. `fract` costs a `floorf` call per sample on baseline
/// x86-64; the phase only passes 1.0 once per period, so test first.
#[inline]
fn wrap_phase(p: f32) -> f32 {
    if p < 1.0 {
        p
    } else {
        p.fract()
    }
}

/// `x`, or 0 when it is NaN or infinite.
#[inline]
fn finite_or_zero(x: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        0.0
    }
}

/// A meter's new peak: this callback's peak, or the old one decayed. Never NaN or infinite, so
/// a bad value cannot stick (`inf * 0.85` is still `inf`).
#[inline]
fn decayed_peak(new: f32, old: f32) -> f32 {
    finite_or_zero(new.max(old * 0.85)).min(16.0)
}

fn midi_hz(p: u8) -> f32 {
    440.0 * 2f32.powf((p as f32 - 69.0) / 12.0)
}

impl EngineCore {
    pub fn new(
        sample_rate: u32,
        rx: Consumer<Command>,
        trash: Producer<Garbage>,
        shared: Arc<Shared>,
    ) -> Self {
        Self {
            sr: sample_rate as f32,
            rx,
            trash,
            shared,
            song: Box::new(Song::default()),
            next_note: 0,
            beat: 0.0,
            start_beat: 0.0,
            playing: false,
            tempo_map: Box::default(),
            metronome: false,
            cycle: None,
            master: GainRamp::new(0.8),
            voices: [Voice::default(); MAX_VOICES],
            click_remaining: 0,
            click_phase: 0.0,
            click_freq: 1000.0,
            track_gain_l: [1.0; MAX_TRACKS],
            track_gain_r: [1.0; MAX_TRACKS],
            track_ramp_l: [GainRamp::new(1.0); MAX_TRACKS],
            track_ramp_r: [GainRamp::new(1.0); MAX_TRACKS],
            mix_ramp_remaining: 0,
            track_pk_l: [0.0; MAX_TRACKS],
            track_pk_r: [0.0; MAX_TRACKS],
            track_audible: [true; MAX_TRACKS],
            track_patch: [SynthPatch::default(); MAX_TRACKS],
            test_hz: 0.0,
            test_gain: 0.0,
            test_phase: 0.0,
        }
    }

    /// Silences timeline voices (seek, pause). Live notes keep playing until `NoteOff`.
    fn kill_voices(&mut self) {
        for v in self.voices.iter_mut().filter(|v| !v.live) {
            v.active = false;
        }
    }

    fn seek(&mut self, beat: f64) {
        self.beat = beat.max(0.0);
        self.kill_voices();
        self.next_note = self
            .song
            .notes
            .partition_point(|n| n.start_beat < self.beat);
        self.reposition_clips();
    }

    fn reposition_clips(&mut self) {
        // Clips play in real time, so their offset is the time since the region start.
        let map = &self.tempo_map;
        let now = map.seconds_at(self.beat);
        for c in self.song.clips.iter_mut() {
            let since = now - map.seconds_at(c.start_beat);
            c.reposition((since * self.sr as f64).round() as i64);
        }
    }

    fn smoothing_samples(&self) -> u32 {
        let sounding = self.playing
            || self.voices.iter().any(|v| v.active)
            || self.click_remaining > 0
            || (self.test_hz > 0.0 && self.test_gain > 0.0);
        if sounding {
            (self.sr * 0.005).round().max(1.0) as u32
        } else {
            0
        }
    }

    fn rebuild_mix(&mut self, smooth: bool) {
        let samples = if smooth { self.smoothing_samples() } else { 0 };
        self.mix_ramp_remaining = samples;
        let any_solo = self.song.tracks.iter().any(|t| t.solo);
        for i in 0..MAX_TRACKS {
            match self.song.tracks.get(i) {
                Some(t) => {
                    let g = 10f32.powf(t.gain_db / 20.0);
                    // Equal-power pan law, normalised so the centre is unity gain.
                    let a = (t.pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
                    self.track_ramp_l[i].set(g * a.cos() * std::f32::consts::SQRT_2, samples);
                    self.track_ramp_r[i].set(g * a.sin() * std::f32::consts::SQRT_2, samples);
                    self.track_audible[i] = !t.mute && (!any_solo || t.solo);
                }
                None => {
                    self.track_ramp_l[i].set(1.0, 0);
                    self.track_ramp_r[i].set(1.0, 0);
                    self.track_audible[i] = true;
                }
            }
            if samples == 0 {
                self.track_gain_l[i] = self.track_ramp_l[i].next();
                self.track_gain_r[i] = self.track_ramp_r[i].next();
            }
        }
    }

    fn drain_commands(&mut self) {
        while let Ok(next) = self.rx.peek() {
            // Leave owning commands in the queue until their old state can be returned.
            // This is the only garbage producer; the consumer can only free more slots,
            // so the single push below is guaranteed to fit after this check.
            if next.retires_state() && self.trash.is_full() {
                break;
            }
            let Ok(cmd) = self.rx.pop() else { break };
            match cmd {
                Command::Play => {
                    if !self.playing {
                        self.start_beat = self.beat;
                    }
                    self.playing = true;
                }
                Command::Pause => {
                    self.playing = false;
                    self.kill_voices();
                }
                Command::Stop => {
                    self.playing = false;
                    let b = self.start_beat;
                    self.seek(b);
                }
                Command::SetPosition { beat } => self.seek(beat),
                Command::SetTempoMap(map) => {
                    let old = std::mem::replace(&mut self.tempo_map, map);
                    let _ = self.trash.push(Garbage::TempoMap(old));
                    self.reposition_clips();
                }
                Command::SetMetronome { on } => self.metronome = on,
                Command::SetMasterGain { linear } => {
                    if linear.is_finite() {
                        let samples = self.smoothing_samples();
                        self.master.set(linear.clamp(0.0, 2.0), samples);
                    }
                }
                Command::SetCycle {
                    on,
                    start_beat,
                    end_beat,
                } => {
                    self.cycle = (on
                        && start_beat.is_finite()
                        && end_beat.is_finite()
                        && start_beat >= 0.0
                        && end_beat > start_beat)
                        .then_some((start_beat, end_beat));
                }
                Command::SetTrackMix { track, mix } => {
                    if let Some(t) = self.song.tracks.get_mut(track as usize) {
                        *t = mix;
                        self.rebuild_mix(true);
                    }
                }
                Command::NoteOn { pitch, vel, track } => {
                    self.live_note_on(pitch.min(127), vel.clamp(1, 127), track)
                }
                Command::NoteOff { pitch } => self.live_note_off(pitch, None),
                Command::NoteOffTrack { pitch, track } => self.live_note_off(pitch, Some(track)),
                Command::SetTrackPatch { track, patch } => {
                    if let Some(slot) = self.track_patch.get_mut(track as usize) {
                        *slot = patch;
                    }
                }
                Command::SetTestTone { hz, gain } => {
                    self.test_hz = hz.clamp(0.0, self.sr * 0.45);
                    self.test_gain = gain.clamp(0.0, 1.0);
                }
                Command::SetSong(new) => {
                    let old = std::mem::replace(&mut self.song, new);
                    let _ = self.trash.push(Garbage::Song(old));
                    self.rebuild_mix(false);
                    let b = self.beat;
                    self.seek(b);
                }
                Command::SetNotes(notes) => {
                    let old = std::mem::replace(&mut self.song.notes, notes);
                    let _ = self.trash.push(Garbage::Notes(old));
                    self.next_note = self
                        .song
                        .notes
                        .partition_point(|n| n.start_beat < self.beat);
                }
            }
        }
    }

    fn trigger(&mut self, n: NoteEvent) -> usize {
        // No free voice: steal the first one.
        let idx = self
            .voices
            .iter()
            .position(|v| !v.active)
            .unwrap_or_default();
        let track = n.track.min(MAX_TRACKS as u8 - 1);
        let mut patch = self.track_patch[track as usize];
        let mut base_hz = midi_hz(n.pitch);
        if patch.kit != Kit::None {
            (patch, base_hz) = kit_voice(&patch, n.pitch);
        }
        self.voices[idx] = Voice {
            active: true,
            releasing: false,
            phase_a: 0.0,
            phase_b: 0.0,
            base_hz,
            amp: (n.vel as f32 / 127.0) * 0.18 * patch.gain,
            env: 0.0,
            stage: 0,
            rel_step: 0.0,
            end_beat: n.end_beat,
            track,
            live: false,
            pitch: n.pitch,
            live_left: 0,
            patch,
            filter: Svf::default(),
            vibrato: 0.0,
            noise: 0x9E37_79B9 ^ (n.pitch as u32).wrapping_mul(2_654_435_761) | 1,
            pitch_env: patch.pitch_drop,
            ratio_b: 2f32.powf((patch.semi_b + patch.detune_b / 100.0) / 12.0),
            pitch_decay_block: (-(CTRL_BLOCK as f32) / (patch.pitch_decay.max(0.001) * self.sr))
                .exp(),
            ctrl_left: 0,
            dt_a: 0.0,
            dt_b: 0.0,
            filter_f: 0.0,
            filter_damping: Svf::damping(patch.resonance),
            attack_step: 1.0 / (patch.attack.max(0.001) * self.sr),
            decay_step: (1.0 - patch.sustain).max(0.0) / (patch.decay.max(0.001) * self.sr),
        };
        idx
    }

    fn live_note_on(&mut self, pitch: u8, vel: u8, track: u8) {
        let idx = self.trigger(NoteEvent {
            start_beat: 0.0,
            end_beat: f64::MAX,
            pitch,
            vel,
            track,
        });
        // Use the exact slot allocated or stolen, including same-pitch timeline voices.
        let live_left = (4.0 * self.sr) as u32;
        let v = &mut self.voices[idx];
        v.live = true;
        v.live_left = live_left;
    }

    fn live_note_off(&mut self, pitch: u8, track: Option<u8>) {
        for v in self.voices.iter_mut().filter(|v| {
            v.active && v.live && v.pitch == pitch && track.is_none_or(|t| v.track == t)
        }) {
            v.releasing = true;
        }
    }

    /// Render interleaved stereo (`out.len() == frames * 2`).
    pub fn process(&mut self, out: &mut [f32]) {
        self.drain_commands();
        let frame_secs = 1.0 / self.sr as f64;
        let (mut pk_l, mut pk_r) = (0f32, 0f32);
        for c in self.song.clips.iter_mut() {
            c.service();
        }
        self.track_pk_l = [0.0; MAX_TRACKS];
        self.track_pk_r = [0.0; MAX_TRACKS];

        for frame in out.as_chunks_mut::<2>().0 {
            // Advance once per track/sample, not once per voice or overlapping audio clip.
            if self.mix_ramp_remaining > 0 {
                for i in 0..MAX_TRACKS {
                    self.track_gain_l[i] = self.track_ramp_l[i].next();
                    self.track_gain_r[i] = self.track_ramp_r[i].next();
                }
                self.mix_ramp_remaining -= 1;
            }
            let (mut l, mut r) = (0f32, 0f32);
            if self.playing {
                while let Some(n) = self.song.notes.get(self.next_note).copied() {
                    if n.start_beat > self.beat {
                        break;
                    }
                    self.next_note += 1;
                    if n.end_beat > self.beat {
                        self.trigger(n);
                    }
                }
                for c in self.song.clips.iter_mut() {
                    if let Some((cl, cr)) = c.tick() {
                        let t = (c.track as usize).min(MAX_TRACKS - 1);
                        if self.track_audible[t] {
                            let (xl, xr) = (cl * self.track_gain_l[t], cr * self.track_gain_r[t]);
                            l += xl;
                            r += xr;
                            self.track_pk_l[t] = self.track_pk_l[t].max(xl.abs());
                            self.track_pk_r[t] = self.track_pk_r[t].max(xr.abs());
                        }
                    }
                }
            }
            for v in self.voices.iter_mut().filter(|v| v.active) {
                if v.live {
                    v.live_left = v.live_left.saturating_sub(1);
                    if v.live_left == 0 {
                        v.releasing = true;
                    }
                } else if self.beat >= v.end_beat || !self.playing {
                    v.releasing = true;
                }
                let sr = self.sr;
                if v.releasing && v.stage != 3 {
                    v.stage = 3;
                    v.rel_step = v.env / (v.patch.release.max(0.002) * sr);
                }
                let pt = &v.patch;
                match v.stage {
                    0 => {
                        v.env += v.attack_step;
                        if v.env >= 1.0 {
                            v.env = 1.0;
                            v.stage = 1;
                        }
                    }
                    1 => {
                        v.env -= v.decay_step;
                        if v.env <= pt.sustain {
                            v.env = pt.sustain;
                            v.stage = 2;
                        }
                    }
                    2 => {}
                    _ => {
                        v.env -= v.rel_step;
                        if v.env <= 0.0 {
                            v.active = false;
                            continue;
                        }
                    }
                }
                let filtered = pt.cutoff < 19_000.0 || pt.highpass;
                if v.ctrl_left == 0 {
                    v.ctrl_left = CTRL_BLOCK;
                    let vib = if pt.vibrato_cents != 0.0 {
                        v.vibrato = (v.vibrato + pt.vibrato_hz * CTRL_BLOCK as f32 / sr).fract();
                        (v.vibrato * std::f32::consts::TAU).sin() * pt.vibrato_cents / 1200.0
                    } else {
                        0.0
                    };
                    let bend = v.pitch_env / 12.0 + vib;
                    let mul = if bend == 0.0 { 1.0 } else { 2f32.powf(bend) };
                    if pt.pitch_drop != 0.0 {
                        v.pitch_env *= v.pitch_decay_block;
                    }
                    v.dt_a = v.base_hz * mul / sr;
                    v.dt_b = v.dt_a * v.ratio_b;
                    if filtered {
                        let fc = if pt.filter_env == 0.0 {
                            pt.cutoff
                        } else {
                            pt.cutoff * 2f32.powf(pt.filter_env * v.env * 4.0)
                        };
                        v.filter_f =
                            Svf::coefficient(fc, sr).min(Svf::max_coefficient(v.filter_damping));
                    }
                }
                v.ctrl_left -= 1;
                let (dt_a, dt_b) = (v.dt_a, v.dt_b);
                v.phase_a = wrap_phase(v.phase_a + dt_a);
                v.phase_b = wrap_phase(v.phase_b + dt_b);
                let mut x = osc(pt.wave_a, v.phase_a, dt_a, pt.pulse_width) * pt.level_a
                    + osc(pt.wave_b, v.phase_b, dt_b, pt.pulse_width) * pt.level_b;
                if pt.noise > 0.0 {
                    v.noise ^= v.noise << 13;
                    v.noise ^= v.noise >> 17;
                    v.noise ^= v.noise << 5;
                    x += ((v.noise as f32 / u32::MAX as f32) * 2.0 - 1.0) * pt.noise;
                }
                if filtered {
                    let (low, high) = v.filter.tick(x, v.filter_f, v.filter_damping);
                    x = if pt.highpass { high } else { low };
                }
                // Last line of defence: a voice that ever goes NaN or infinite is silenced for
                // that sample and its filter reset, instead of poisoning the mix and meters.
                if !x.is_finite() {
                    v.filter = Svf::default();
                    x = 0.0;
                }
                let t = v.track as usize;
                if self.track_audible[t] {
                    let x = x * v.amp * v.env;
                    let (xl, xr) = (x * self.track_gain_l[t], x * self.track_gain_r[t]);
                    l += xl;
                    r += xr;
                    self.track_pk_l[t] = self.track_pk_l[t].max(xl.abs());
                    self.track_pk_r[t] = self.track_pk_r[t].max(xr.abs());
                }
            }
            if self.test_hz > 0.0 {
                self.test_phase += self.test_hz / self.sr;
                if self.test_phase >= 1.0 {
                    self.test_phase -= 1.0;
                }
                let t = (self.test_phase * std::f32::consts::TAU).sin() * self.test_gain;
                l += t;
                r += t;
            }
            if self.click_remaining > 0 {
                self.click_remaining -= 1;
                self.click_phase += self.click_freq / self.sr;
                let env = self.click_remaining as f32 / (0.03 * self.sr);
                let c = (self.click_phase * std::f32::consts::TAU).sin() * env * 0.3;
                l += c;
                r += c;
            }
            let master = self.master.next();
            // `clamp` lets NaN through, so a non-finite sum becomes silence first.
            l = finite_or_zero(l * master).clamp(-1.0, 1.0);
            r = finite_or_zero(r * master).clamp(-1.0, 1.0);
            pk_l = pk_l.max(l.abs());
            pk_r = pk_r.max(r.abs());
            frame[0] = l;
            frame[1] = r;
            // Render events at the current sample position, then advance to the next one.
            if self.playing {
                let prev = self.beat;
                self.beat = self.tempo_map.advance(prev, frame_secs);
                if let Some((s, e)) = self.cycle {
                    if self.beat >= e {
                        let start = self.tempo_map.seconds_at(s);
                        let end = self.tempo_map.seconds_at(e);
                        let duration = end - start;
                        if duration > 0.0 {
                            let excess = self.tempo_map.seconds_at(self.beat) - end;
                            let wrapped =
                                self.tempo_map.beat_at(start + excess.rem_euclid(duration));
                            self.seek(wrapped);
                            // A fractional-sample wrap must still trigger notes at cycle start.
                            self.next_note = self.song.notes.partition_point(|n| n.start_beat < s);
                        }
                    }
                }
                if self.metronome {
                    let (a, b) = (
                        self.tempo_map.position(prev),
                        self.tempo_map.position(self.beat),
                    );
                    if (a.bar, a.beat) != (b.bar, b.beat) {
                        self.click_remaining = (0.03 * self.sr) as u32;
                        self.click_phase = 0.0;
                        self.click_freq = if b.beat == 0 { 1500.0 } else { 1000.0 };
                    }
                }
            }
        }
        let mut underruns = 0;
        for c in self.song.clips.iter_mut() {
            underruns += std::mem::take(&mut c.underruns).min(1);
        }
        if underruns > 0 {
            self.shared
                .underruns
                .fetch_add(underruns, Ordering::Relaxed);
        }
        self.shared
            .position_beats
            .store(self.beat.to_bits(), Ordering::Relaxed);
        self.shared.playing.store(self.playing, Ordering::Relaxed);
        // Peak with simple decay so the UI meter does not flicker between callbacks.
        let (ol, or) = self.shared.peaks();
        self.shared
            .peak_l
            .store(decayed_peak(pk_l, ol).to_bits(), Ordering::Relaxed);
        self.shared
            .peak_r
            .store(decayed_peak(pk_r, or).to_bits(), Ordering::Relaxed);
        for t in 0..MAX_TRACKS {
            let (ol, or) = self.shared.track_peaks(t);
            self.shared.track_peak_l[t].store(
                decayed_peak(self.track_pk_l[t], ol).to_bits(),
                Ordering::Relaxed,
            );
            self.shared.track_peak_r[t].store(
                decayed_peak(self.track_pk_r[t], or).to_bits(),
                Ordering::Relaxed,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rig() -> (EngineCore, Producer<Command>, Arc<Shared>) {
        let (tx, rx) = rtrb::RingBuffer::new(64);
        let (gtx, _grx) = rtrb::RingBuffer::new(8);
        let shared = Arc::new(Shared::new());
        (EngineCore::new(48_000, rx, gtx, shared.clone()), tx, shared)
    }

    fn note(start: f64, end: f64) -> NoteEvent {
        NoteEvent {
            start_beat: start,
            end_beat: end,
            pitch: 69,
            vel: 100,
            track: 0,
        }
    }

    #[test]
    fn full_garbage_queue_defers_state_in_order_and_does_not_allocate() {
        let (mut tx, rx) = rtrb::RingBuffer::new(8);
        let (gtx, mut grx) = rtrb::RingBuffer::new(1);
        let shared = Arc::new(Shared::new());
        let mut e = EngineCore::new(48_000, rx, gtx, shared);
        let commands = [
            Command::SetSong(Box::new(Song::new(vec![note(0.0, 1.0)], vec![]))),
            Command::SetNotes(vec![note(2.0, 3.0)]),
            Command::SetTempoMap(Box::new(TempoMap::constant(90.0, (6, 8)))),
            Command::SetSong(Box::new(Song::new(vec![note(4.0, 5.0)], vec![]))),
            Command::Play,
        ];
        for cmd in commands {
            assert!(tx.push(cmd).is_ok());
        }
        let mut buf = [0.0; 256];
        for remaining in [4, 3, 2, 0] {
            assert_no_alloc::assert_no_alloc(|| e.process(&mut buf));
            assert_eq!(e.rx.slots(), remaining);
            assert!(buf.iter().all(|s| s.is_finite()));
            if remaining > 0 {
                // Repeated callbacks must neither consume nor destroy the blocked command.
                assert_no_alloc::assert_no_alloc(|| e.process(&mut buf));
                assert_eq!(e.rx.slots(), remaining);
                assert!(!e.playing);
            }
            assert!(grx.pop().is_ok()); // reclamation outside the audio callback
        }
        assert!(e.playing);
        assert_eq!(e.song.notes[0].start_beat, 4.0);
    }

    #[test]
    fn one_second_of_silence_is_finite_and_zero() {
        let (mut e, _tx, _s) = rig();
        let mut buf = vec![1.0f32; 48_000 * 2];
        e.process(&mut buf);
        assert!(buf.iter().all(|x| x.is_finite() && *x == 0.0));
    }

    #[test]
    fn transport_advances_one_beat_per_half_second_at_120() {
        let (mut e, mut tx, s) = rig();
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 24_000 * 2];
        e.process(&mut buf);
        assert!((s.position() - 1.0).abs() < 1e-3, "{}", s.position());
        assert!(s.playing.load(Ordering::Relaxed));
    }

    #[test]
    fn stop_returns_to_start_position() {
        let (mut e, mut tx, s) = rig();
        assert!(tx.push(Command::SetPosition { beat: 8.0 }).is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 4800 * 2];
        e.process(&mut buf);
        assert!(s.position() > 8.0);
        assert!(tx.push(Command::Stop).is_ok());
        e.process(&mut buf);
        assert_eq!(s.position(), 8.0);
    }

    #[test]
    fn note_makes_sound_and_never_nan() {
        let (mut e, mut tx, _s) = rig();
        let song = Song::new(
            vec![note(0.0, 1.0), note(0.5, 2.0)],
            vec![TrackMix::default()],
        );
        assert!(tx.push(Command::SetSong(Box::new(song))).is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 48_000 * 2];
        e.process(&mut buf);
        assert!(buf.iter().all(|x| x.is_finite()));
        assert!(buf.iter().any(|x| x.abs() > 0.01));
    }

    #[test]
    fn muted_track_is_silent() {
        let (mut e, mut tx, _s) = rig();
        let song = Song::new(
            vec![note(0.0, 1.0)],
            vec![TrackMix {
                gain_db: 0.0,
                mute: true,
                solo: false,
                pan: 0.0,
            }],
        );
        assert!(tx.push(Command::SetSong(Box::new(song))).is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 24_000 * 2];
        e.process(&mut buf);
        assert!(buf.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn metronome_clicks_on_beat() {
        let (mut e, mut tx, _s) = rig();
        assert!(tx.push(Command::SetMetronome { on: true }).is_ok());
        assert!(tx.push(Command::SetPosition { beat: 0.99 }).is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 4_800 * 2];
        e.process(&mut buf);
        assert!(buf.iter().any(|x| x.abs() > 0.05));
    }

    #[test]
    fn hard_left_pan_silences_right_and_meters_track() {
        let (mut e, mut tx, s) = rig();
        let mix = TrackMix {
            pan: -1.0,
            ..TrackMix::default()
        };
        let song = Song::new(vec![note(0.0, 2.0)], vec![mix]);
        assert!(tx.push(Command::SetSong(Box::new(song))).is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 4800 * 2];
        e.process(&mut buf);
        let right = buf
            .iter()
            .skip(1)
            .step_by(2)
            .fold(0f32, |m, x| m.max(x.abs()));
        let left = buf.iter().step_by(2).fold(0f32, |m, x| m.max(x.abs()));
        assert!(right < 1e-6, "right {right}");
        assert!(left > 0.05, "left {left}");
        let (tl, tr) = s.track_peaks(0);
        assert!(tl > 0.05 && tr < 1e-6, "meters {tl} {tr}");
    }

    #[test]
    fn live_note_sounds_while_stopped_and_stops_on_note_off() {
        let (mut e, mut tx, s) = rig();
        assert!(tx
            .push(Command::SetSong(Box::new(Song::new(
                vec![],
                vec![TrackMix::default()]
            ))))
            .is_ok());
        assert!(tx
            .push(Command::NoteOn {
                pitch: 69,
                vel: 100,
                track: 0
            })
            .is_ok());
        let mut buf = vec![0.0f32; 4800 * 2];
        e.process(&mut buf);
        assert!(buf.iter().any(|x| x.abs() > 0.05), "no sound while stopped");
        assert!(!s.playing.load(Ordering::Relaxed));
        // A seek must not cut a live note.
        assert!(tx.push(Command::SetPosition { beat: 1.0 }).is_ok());
        e.process(&mut buf);
        assert!(
            buf.iter().any(|x| x.abs() > 0.05),
            "seek killed the live note"
        );
        assert!(tx.push(Command::NoteOff { pitch: 69 }).is_ok());
        e.process(&mut buf); // 100 ms: release is 90 ms
        e.process(&mut buf);
        assert!(buf.iter().all(|x| x.abs() < 1e-4), "note did not release");
    }

    #[test]
    fn live_note_releases_by_itself_after_four_seconds() {
        let (mut e, mut tx, _s) = rig();
        assert!(tx
            .push(Command::NoteOn {
                pitch: 60,
                vel: 100,
                track: 0
            })
            .is_ok());
        let mut buf = vec![0.0f32; 48_000 * 2];
        for _ in 0..3 {
            e.process(&mut buf);
        }
        assert!(
            buf.iter().any(|x| x.abs() > 0.05),
            "should still sound at 3 s"
        );
        // The safety release starts at 4.0 s and fades over 90 ms: silent from the 6th second.
        for _ in 0..3 {
            e.process(&mut buf);
        }
        assert!(
            buf.iter().all(|x| x.abs() < 1e-4),
            "should be silent after 6 s"
        );
    }

    #[test]
    fn live_note_marks_its_own_voice_and_preserves_timeline_voice() {
        let (mut e, _tx, _s) = rig();
        let timeline = e.trigger(note(0.0, 8.0));
        e.live_note_on(69, 100, 1);
        assert!(!e.voices[timeline].live);
        assert!(e.voices[1].live);
        assert_eq!(e.voices[1].track, 1);
        // Fill the remaining pool, then exercise the voice-stealing path.
        for _ in 2..MAX_VOICES {
            e.trigger(note(0.0, 8.0));
        }
        e.live_note_on(72, 100, 2);
        assert!(e.voices[0].live);
        assert_eq!((e.voices[0].pitch, e.voices[0].track), (72, 2));
        assert_eq!(e.voices.iter().filter(|v| v.active).count(), MAX_VOICES);
    }

    #[test]
    fn scoped_note_off_preserves_other_tracks_and_timeline_notes() {
        let (mut e, mut tx, _s) = rig();
        let timeline = e.trigger(note(0.0, 8.0));
        e.live_note_on(69, 100, 0);
        e.live_note_on(69, 100, 1);
        e.live_note_on(72, 100, 0);
        assert!(tx
            .push(Command::NoteOffTrack {
                pitch: 69,
                track: 0
            })
            .is_ok());
        assert_no_alloc::assert_no_alloc(|| e.process(&mut []));
        assert!(!e.voices[timeline].releasing);
        assert!(e.voices[1].releasing);
        assert!(!e.voices[2].releasing);
        assert!(!e.voices[3].releasing);
        assert!(tx
            .push(Command::NoteOffTrack {
                pitch: 69,
                track: 255
            })
            .is_ok());
        assert_no_alloc::assert_no_alloc(|| e.process(&mut []));
        assert!(!e.voices[2].releasing);
        // Existing callers retain the global-release behavior.
        assert!(tx.push(Command::NoteOff { pitch: 69 }).is_ok());
        assert_no_alloc::assert_no_alloc(|| e.process(&mut []));
        assert!(e.voices[2].releasing);
        assert!(!e.voices[timeline].releasing);
        assert!(!e.voices[3].releasing);
    }

    fn play_note_with(patch: SynthPatch, pitch: u8, frames: usize) -> Vec<f32> {
        let (mut e, mut tx, _s) = rig();
        assert!(tx
            .push(Command::SetSong(Box::new(Song::new(
                vec![],
                vec![TrackMix::default()]
            ))))
            .is_ok());
        assert!(tx.push(Command::SetTrackPatch { track: 0, patch }).is_ok());
        assert!(tx
            .push(Command::NoteOn {
                pitch,
                vel: 110,
                track: 0
            })
            .is_ok());
        let mut buf = vec![0.0f32; frames * 2];
        e.process(&mut buf);
        buf.iter().step_by(2).copied().collect()
    }

    #[test]
    fn slow_attack_patch_starts_quieter_than_the_default() {
        let slow = SynthPatch {
            attack: 0.4,
            ..SynthPatch::default()
        };
        let early = |x: &[f32]| x[..480].iter().map(|v| v * v).sum::<f32>();
        let fast = play_note_with(SynthPatch::default(), 69, 4800);
        let slow = play_note_with(slow, 69, 4800);
        assert!(
            early(&slow) < early(&fast) * 0.05,
            "{} vs {}",
            early(&slow),
            early(&fast)
        );
    }

    /// A GitHub report: piano patches (no resonance, a filter envelope that opens the cutoff
    /// to its limit) made the filter diverge, and the track's peak meter stuck at `inf`.
    #[test]
    fn a_fully_opened_filter_without_resonance_stays_finite() {
        use crate::synth::Wave;
        let patch = SynthPatch {
            wave_a: Wave::Saw,
            cutoff: 4000.0,
            resonance: 0.0,
            filter_env: 1.0,
            ..SynthPatch::default()
        };
        for pitch in [48, 72, 96] {
            let out = play_note_with(patch, pitch, 48_000);
            assert!(out.iter().all(|x| x.is_finite()), "pitch {pitch}");
            assert!(out.iter().any(|x| x.abs() > 0.01), "pitch {pitch} silent");
        }
    }

    #[test]
    fn stuck_or_bad_peaks_fall_back_to_finite_values() {
        assert_eq!(decayed_peak(f32::INFINITY, 0.5), 0.0);
        assert_eq!(decayed_peak(0.2, f32::INFINITY), 0.0);
        assert_eq!(
            decayed_peak(f32::NAN, 0.5),
            0.5 * 0.85,
            "NaN is ignored by max"
        );
        assert_eq!(decayed_peak(0.0, 1.0), 0.85, "decays");
        assert_eq!(finite_or_zero(f32::NAN), 0.0);
        assert_eq!(finite_or_zero(-0.25), -0.25);
    }

    #[test]
    fn the_stability_limit_holds_for_every_resonance() {
        for r in [0.0, 0.05, 0.3, 1.0] {
            let d = Svf::damping(r);
            let f = Svf::max_coefficient(d);
            assert!(f * f + 2.0 * f * d < 4.0, "resonance {r}");
            assert!(f > 0.9, "the limit leaves the filter bright: {f}");
        }
    }

    #[test]
    fn a_dark_filtered_saw_is_smoother_than_a_bright_one() {
        use crate::synth::Wave;
        let base = SynthPatch {
            wave_a: Wave::Saw,
            level_b: 0.0,
            ..SynthPatch::default()
        };
        let dark = SynthPatch {
            cutoff: 400.0,
            ..base
        };
        let rough = |x: &[f32]| {
            x.windows(3)
                .map(|w| (w[2] - 2.0 * w[1] + w[0]).abs())
                .sum::<f32>()
        };
        let a = play_note_with(base, 57, 9600);
        let b = play_note_with(dark, 57, 9600);
        assert!(rough(&b[2400..]) < rough(&a[2400..]) * 0.6);
        assert!(b.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn drum_kit_notes_sound_different_and_decay() {
        let kit = SynthPatch {
            kit: Kit::Acoustic,
            ..SynthPatch::default()
        };
        let kick = play_note_with(kit, 36, 24_000);
        let hat = play_note_with(kit, 42, 24_000);
        assert!(kick.iter().any(|x| x.abs() > 0.05), "kick silent");
        assert!(hat.iter().any(|x| x.abs() > 0.02), "hat silent");
        // Both are short: the last quarter second is quiet.
        let tail = |x: &[f32]| x[18_000..].iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(tail(&kick) < 0.02 && tail(&hat) < 0.02);
        assert!(kick != hat);
    }

    /// Roadmap §5 acceptance: zero allocations/frees on the audio thread.
    #[test]
    fn process_does_not_allocate() {
        let (mut e, mut tx, _s) = rig();
        let notes = (0..64)
            .map(|i| note(i as f64 * 0.25, i as f64 * 0.25 + 0.5))
            .collect();
        let song = Song::new(notes, vec![TrackMix::default()]);
        assert!(tx.push(Command::SetSong(Box::new(song))).is_ok());
        let mut buf = vec![0.0f32; 512 * 2];
        let cmds = [
            Command::SetMetronome { on: true },
            Command::SetCycle {
                on: true,
                start_beat: 0.0,
                end_beat: 4.0,
            },
            Command::SetTestTone {
                hz: 440.0,
                gain: 0.1,
            },
            Command::SetTrackMix {
                track: 0,
                mix: TrackMix {
                    gain_db: -6.0,
                    mute: false,
                    solo: true,
                    pan: 0.0,
                },
            },
            Command::SetTempoMap(Box::new(TempoMap::constant(140.0, (3, 4)))),
            Command::NoteOn {
                pitch: 64,
                vel: 90,
                track: 0,
            },
            Command::SetTrackPatch {
                track: 0,
                patch: SynthPatch {
                    kit: Kit::Acoustic,
                    ..SynthPatch::default()
                },
            },
            Command::NoteOn {
                pitch: 42,
                vel: 90,
                track: 0,
            },
            Command::Play,
        ];
        for c in cmds {
            assert!(tx.push(c).is_ok());
        }
        for i in 0..400 {
            if i == 200 {
                assert!(tx.push(Command::SetPosition { beat: 2.0 }).is_ok());
            }
            assert_no_alloc::assert_no_alloc(|| e.process(&mut buf));
        }
        assert!(tx.push(Command::Stop).is_ok());
        assert_no_alloc::assert_no_alloc(|| e.process(&mut buf));
        assert!(buf.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn cycle_loops_back() {
        let (mut e, mut tx, s) = rig();
        assert!(tx
            .push(Command::SetCycle {
                on: true,
                start_beat: 0.0,
                end_beat: 1.0
            })
            .is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 48_000 * 2];
        e.process(&mut buf);
        assert!(s.position() < 1.0);
    }

    #[test]
    fn midi_note_does_not_start_one_sample_early() {
        let (mut e, mut tx, _s) = rig();
        let song = Song::new(vec![note(1.0 / 24_000.0, 1.0)], vec![TrackMix::default()]);
        assert!(tx.push(Command::SetSong(Box::new(song))).is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut frame = [0.0; 2];
        e.process(&mut frame);
        assert_eq!(frame, [0.0; 2]);
        assert_eq!(e.next_note, 0);
        e.process(&mut frame);
        assert_eq!(e.next_note, 1);
        assert!(frame[0].abs() > 0.0);
    }

    #[test]
    fn fractional_cycle_wrap_retriggers_note_at_cycle_start() {
        let (mut e, mut tx, _s) = rig();
        let song = Song::new(vec![note(0.0, 1.0)], vec![TrackMix::default()]);
        for cmd in [
            Command::SetSong(Box::new(song)),
            Command::SetCycle {
                on: true,
                start_beat: 0.0,
                end_beat: 2.5 / 24_000.0,
            },
            Command::Play,
        ] {
            assert!(tx.push(cmd).is_ok());
        }
        e.process(&mut [0.0; 6]);
        assert!(e.beat > 0.0, "fractional remainder preserved");
        assert_eq!(e.next_note, 0, "cycle-start note must not be skipped");
        let mut frame = [0.0; 2];
        e.process(&mut frame);
        assert_eq!(e.next_note, 1);
        assert!(frame[0].abs() > 0.0);
    }

    #[test]
    fn invalid_cycle_bounds_are_ignored() {
        let (mut e, mut tx, _s) = rig();
        for (start, end) in [
            (0.0, f64::INFINITY),
            (f64::NAN, 1.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ] {
            assert!(tx
                .push(Command::SetCycle {
                    on: true,
                    start_beat: start,
                    end_beat: end
                })
                .is_ok());
            e.process(&mut []);
            assert!(e.cycle.is_none());
        }
    }

    #[test]
    fn dsp_load_is_busy_time_over_buffer_time() {
        use std::time::Duration;
        // 256 frames at 48 kHz play for 5.333 ms; 1.333 ms of work is a quarter of that.
        let load = dsp_load(Duration::from_micros(1_333), 256, 48_000);
        assert!((load - 0.25).abs() < 0.001, "{load}");
        assert_eq!(dsp_load(Duration::from_millis(1), 0, 48_000), 0.0);
    }

    #[test]
    fn dsp_load_meter_keeps_the_peak_until_read() {
        let s = Shared::new();
        for load in [0.2, 0.7, 0.3] {
            assert_no_alloc::assert_no_alloc(|| s.record_dsp_load(load));
        }
        assert_eq!(s.take_dsp_load(), 0.7);
        assert_eq!(s.take_dsp_load(), 0.0);
        s.record_dsp_load(1.5);
        assert_eq!(s.take_dsp_load(), 1.5, "overloads above 100% are reported");
    }

    #[test]
    fn transport_follows_tempo_changes() {
        use crate::tempo_map::TempoEvent;
        let (mut e, mut tx, s) = rig();
        // 120 BPM for the first beat (0.5 s), then 60 BPM.
        let map = TempoMap::new(
            vec![
                TempoEvent {
                    beat: 0.0,
                    bpm: 120.0,
                },
                TempoEvent {
                    beat: 1.0,
                    bpm: 60.0,
                },
            ],
            vec![],
        );
        assert!(tx.push(Command::SetTempoMap(Box::new(map))).is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 48_000 * 2];
        e.process(&mut buf);
        assert!((s.position() - 1.5).abs() < 1e-6, "{}", s.position());
    }

    #[test]
    fn metronome_accents_the_first_beat_of_each_bar_in_three_four() {
        let (mut e, mut tx, _s) = rig();
        let map = TempoMap::constant(120.0, (3, 4));
        assert!(tx.push(Command::SetTempoMap(Box::new(map))).is_ok());
        assert!(tx.push(Command::SetMetronome { on: true }).is_ok());
        let mut buf = vec![0.0f32; 64 * 2];
        let mut click_at = |e: &mut EngineCore, tx: &mut Producer<Command>, beat: f64| {
            assert!(tx.push(Command::SetPosition { beat: beat - 0.001 }).is_ok());
            assert!(tx.push(Command::Play).is_ok());
            e.process(&mut buf);
            assert!(e.click_remaining > 0, "no click at beat {beat}");
            e.click_freq
        };
        assert_eq!(click_at(&mut e, &mut tx, 2.0), 1000.0);
        assert_eq!(
            click_at(&mut e, &mut tx, 3.0),
            1500.0,
            "bar 2 starts at beat 3"
        );
        assert_eq!(click_at(&mut e, &mut tx, 4.0), 1000.0, "not a 4/4 downbeat");
    }

    #[test]
    fn set_notes_swaps_notes_without_cutting_voices() {
        let (mut e, mut tx, _s) = rig();
        let song = Song::new(vec![note(0.0, 8.0)], vec![TrackMix::default()]);
        assert!(tx.push(Command::SetSong(Box::new(song))).is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 4_800 * 2];
        e.process(&mut buf);
        assert_eq!(e.voices.iter().filter(|v| v.active).count(), 1);
        // Unsorted on purpose: `set_notes` sorts.
        let notes = vec![note(1.0, 2.0), note(0.05, 0.5), note(0.5, 1.0)];
        assert!(tx.push(Command::set_notes(notes)).is_ok());
        assert_no_alloc::assert_no_alloc(|| e.process(&mut buf));
        assert_eq!(e.song.notes.len(), 3);
        assert!(e
            .song
            .notes
            .windows(2)
            .all(|w| w[0].start_beat <= w[1].start_beat));
        // The held note survives; the next note to play is the one at beat 0.5.
        assert!(e.voices.iter().any(|v| v.active && v.end_beat == 8.0));
        assert_eq!(e.song.notes[e.next_note].start_beat, 0.5);
    }

    #[test]
    fn solo_plays_only_soloed_tracks_until_released() {
        let (mut e, mut tx, s) = rig();
        let on = |track| NoteEvent {
            start_beat: 0.0,
            end_beat: 8.0,
            pitch: 60,
            vel: 100,
            track,
        };
        let soloed = TrackMix {
            solo: true,
            ..TrackMix::default()
        };
        let song = Song::new(vec![on(0), on(1)], vec![TrackMix::default(), soloed]);
        assert!(tx.push(Command::SetSong(Box::new(song))).is_ok());
        assert!(tx.push(Command::Play).is_ok());
        let mut buf = vec![0.0f32; 4_800 * 2];
        e.process(&mut buf);
        assert_eq!(s.track_peaks(0), (0.0, 0.0), "not soloed: silent");
        assert!(s.track_peaks(1).0 > 0.0, "soloed: audible");

        // Releasing solo brings the other track back.
        let mix = TrackMix::default();
        assert!(tx.push(Command::SetTrackMix { track: 1, mix }).is_ok());
        e.process(&mut buf);
        assert!(s.track_peaks(0).0 > 0.0 && s.track_peaks(1).0 > 0.0);
    }
}
