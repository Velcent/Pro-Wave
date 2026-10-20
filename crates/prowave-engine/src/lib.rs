//! ProWave Studio real-time engine.
//!
//! Logic Pro equivalent: transport + software-instrument playback + metronome (Klopfgeist).
//! The UI talks to the engine only through [`Command`]s on a lock-free SPSC queue and reads
//! state back from atomics ([`Shared`]); the audio callback never locks, allocates or does I/O.
#![deny(clippy::unwrap_used, clippy::expect_used)]

// Debug test builds abort if anything inside `assert_no_alloc(..)` allocates or frees.
#[cfg(all(test, debug_assertions))]
#[global_allocator]
static ALLOC_GUARD: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

mod core;
mod disk;
mod gain_smoothing;
mod output;
mod synth;
mod tempo_map;

pub use crate::core::{
    dsp_load, AudioClip, ClipEdit, Command, EngineCore, FadeCurve, NoteEvent, Shared, Song,
    TrackMix, MAX_TRACKS,
};
pub use crate::disk::{
    ring_blocks_for, AudioError, AudioFile, DiskThread, RING_BLOCKS, RING_BLOCKS_MAX,
};
pub use crate::output::{peak_of, Engine, EngineError};
pub use crate::synth::{Kit, SynthPatch, Wave};
pub use crate::tempo_map::{
    MusicalPosition, SignatureEvent, Smpte, SmpteRate, TempoCurve, TempoEvent, TempoMap, MAX_BPM,
    MIN_BPM,
};
