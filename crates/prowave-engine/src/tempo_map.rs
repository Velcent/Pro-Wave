//! Tempo map: tempo and time-signature changes (Logic Pro: the Tempo and Signature global
//! tracks).
//!
//! Positions are beats = quarter notes from the project start (0-based), like the rest of the
//! engine; as in Logic, the tempo is always quarter notes per minute whatever the signature.
//! Tempo changes are steps (the tempo holds until the next event). Segments are computed when
//! the map is built (on the UI thread), so every lookup on the audio thread is a binary search
//! that never allocates. Replace the engine's map with `Command::SetTempoMap`.

/// Logic Pro's tempo range.
pub const MIN_BPM: f32 = 5.0;
pub const MAX_BPM: f32 = 990.0;

/// Tempo change at `beat` (quarter notes from the start).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TempoEvent {
    pub beat: f64,
    pub bpm: f32,
}

/// Time signature `num/den` from the start of `bar` (0-based) on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignatureEvent {
    pub bar: u32,
    pub num: u8,
    pub den: u8,
}

/// A position in bars and beats (all 0-based; `beat` counts `den` notes, e.g. eighths in 6/8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MusicalPosition {
    pub bar: u32,
    pub beat: u32,
    /// How far into `beat` (0..1).
    pub fraction: f64,
    pub num: u8,
    pub den: u8,
}

#[derive(Clone, Copy, Debug)]
struct TempoSegment {
    beat: f64,
    seconds: f64,
    bpm: f64,
}

#[derive(Clone, Copy, Debug)]
struct SigSegment {
    bar: u32,
    beat: f64,
    num: u8,
    den: u8,
}

impl SigSegment {
    /// Quarter notes per signature beat (a `den` note).
    fn unit(&self) -> f64 {
        4.0 / self.den as f64
    }
    fn bar_len(&self) -> f64 {
        self.num as f64 * self.unit()
    }
}

#[derive(Clone, Debug)]
pub struct TempoMap {
    /// Sorted by beat; the first starts at beat 0.
    tempos: Vec<TempoSegment>,
    /// Sorted by bar; the first starts at bar 0.
    sigs: Vec<SigSegment>,
}

impl Default for TempoMap {
    fn default() -> Self {
        Self::constant(120.0, (4, 4))
    }
}

impl TempoMap {
    /// One tempo and one signature for the whole project.
    pub fn constant(bpm: f32, sig: (u8, u8)) -> Self {
        Self::new(
            vec![TempoEvent { beat: 0.0, bpm }],
            vec![SignatureEvent {
                bar: 0,
                num: sig.0,
                den: sig.1,
            }],
        )
    }

    /// Builds a map from events in any order. The first tempo and signature apply from the
    /// start (defaults 120 BPM, 4/4); of several events at the same place the last one wins.
    /// Tempos are clamped to Logic's 5-990 BPM; an invalid denominator becomes 4.
    pub fn new(mut tempos: Vec<TempoEvent>, mut sigs: Vec<SignatureEvent>) -> Self {
        tempos.retain(|e| e.beat.is_finite() && e.bpm.is_finite());
        for e in tempos.iter_mut() {
            e.beat = e.beat.max(0.0);
        }
        tempos.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        tempos.dedup_by(|later, kept| {
            let same = later.beat == kept.beat;
            if same {
                kept.bpm = later.bpm;
            }
            same
        });
        let mut segs: Vec<TempoSegment> = Vec::with_capacity(tempos.len().max(1));
        for (i, e) in tempos.iter().enumerate() {
            let bpm = e.bpm.clamp(MIN_BPM, MAX_BPM) as f64;
            // The first tempo holds from the start of the project.
            let beat = if i == 0 { 0.0 } else { e.beat };
            let seconds = match segs.last() {
                Some(p) => p.seconds + (beat - p.beat) * 60.0 / p.bpm,
                None => 0.0,
            };
            segs.push(TempoSegment { beat, seconds, bpm });
        }
        if segs.is_empty() {
            segs.push(TempoSegment {
                beat: 0.0,
                seconds: 0.0,
                bpm: 120.0,
            });
        }

        sigs.sort_by_key(|s| s.bar);
        sigs.dedup_by(|later, kept| {
            let same = later.bar == kept.bar;
            if same {
                *kept = *later;
            }
            same
        });
        let mut sig_segs: Vec<SigSegment> = Vec::with_capacity(sigs.len().max(1));
        for (i, s) in sigs.iter().enumerate() {
            let bar = if i == 0 { 0 } else { s.bar };
            let den = if matches!(s.den, 1 | 2 | 4 | 8 | 16 | 32 | 64) {
                s.den
            } else {
                4
            };
            let beat = match sig_segs.last() {
                Some(p) => p.beat + (bar - p.bar) as f64 * p.bar_len(),
                None => 0.0,
            };
            sig_segs.push(SigSegment {
                bar,
                beat,
                num: s.num.clamp(1, 99),
                den,
            });
        }
        if sig_segs.is_empty() {
            sig_segs.push(SigSegment {
                bar: 0,
                beat: 0.0,
                num: 4,
                den: 4,
            });
        }
        Self {
            tempos: segs,
            sigs: sig_segs,
        }
    }

    fn tempo_seg(&self, beat: f64) -> &TempoSegment {
        let i = self.tempos.partition_point(|s| s.beat <= beat);
        &self.tempos[i.saturating_sub(1)]
    }

    fn sig_seg(&self, beat: f64) -> &SigSegment {
        let i = self.sigs.partition_point(|s| s.beat <= beat);
        &self.sigs[i.saturating_sub(1)]
    }

    /// Tempo in effect at `beat`.
    pub fn tempo_at(&self, beat: f64) -> f32 {
        self.tempo_seg(beat).bpm as f32
    }

    /// Time signature in effect at `beat`.
    pub fn signature_at(&self, beat: f64) -> (u8, u8) {
        let s = self.sig_seg(beat);
        (s.num, s.den)
    }

    /// Seconds from the project start to `beat`.
    pub fn seconds_at(&self, beat: f64) -> f64 {
        let s = self.tempo_seg(beat);
        s.seconds + (beat - s.beat) * 60.0 / s.bpm
    }

    /// Beat reached `seconds` after the project start (inverse of [`TempoMap::seconds_at`]).
    pub fn beat_at(&self, seconds: f64) -> f64 {
        let i = self.tempos.partition_point(|s| s.seconds <= seconds);
        let s = &self.tempos[i.saturating_sub(1)];
        s.beat + (seconds - s.seconds) * s.bpm / 60.0
    }

    /// The beat `seconds` after `beat`, following every tempo change in between
    /// (the transport clock: called once per sample with `1 / sample_rate`).
    pub fn advance(&self, beat: f64, seconds: f64) -> f64 {
        self.beat_at(self.seconds_at(beat) + seconds)
    }

    /// Beat at which `bar` (0-based) starts.
    pub fn beat_of_bar(&self, bar: u32) -> f64 {
        let i = self.sigs.partition_point(|s| s.bar <= bar);
        let s = &self.sigs[i.saturating_sub(1)];
        s.beat + (bar - s.bar) as f64 * s.bar_len()
    }

    /// Bars and beats of `beat` (negative positions count as the start).
    pub fn position(&self, beat: f64) -> MusicalPosition {
        let beat = beat.max(0.0);
        let s = self.sig_seg(beat);
        let into = beat - s.beat;
        let bars = (into / s.bar_len()).floor();
        let in_bar = (into - bars * s.bar_len()) / s.unit();
        let b = in_bar.floor();
        MusicalPosition {
            bar: s.bar + bars as u32,
            beat: (b as u32).min(s.num as u32 - 1),
            fraction: in_bar - b,
            num: s.num,
            den: s.den,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn tempo(beat: f64, bpm: f32) -> TempoEvent {
        TempoEvent { beat, bpm }
    }

    fn sig(bar: u32, num: u8, den: u8) -> SignatureEvent {
        SignatureEvent { bar, num, den }
    }

    #[test]
    fn constant_tempo_converts_both_ways() {
        let m = TempoMap::constant(120.0, (4, 4));
        assert!(close(m.seconds_at(4.0), 2.0));
        assert!(close(m.beat_at(2.0), 4.0));
        assert_eq!(m.tempo_at(1000.0), 120.0);
    }

    #[test]
    fn tempo_change_splits_time_at_the_event() {
        // 120 BPM for 4 beats (2 s), then 60 BPM.
        let m = TempoMap::new(vec![tempo(4.0, 60.0), tempo(0.0, 120.0)], vec![]);
        assert_eq!(m.tempo_at(3.999), 120.0);
        assert_eq!(m.tempo_at(4.0), 60.0);
        assert!(close(m.seconds_at(8.0), 6.0));
        assert!(close(m.beat_at(6.0), 8.0));
        // 0.5 s reaches beat 4, the other 0.5 s at 60 BPM adds half a beat.
        assert!(close(m.advance(3.0, 1.0), 4.5));
    }

    #[test]
    fn advancing_sample_by_sample_lands_on_the_exact_beat() {
        let m = TempoMap::new(vec![tempo(0.0, 120.0), tempo(2.0, 90.0)], vec![]);
        let mut beat = 0.0;
        for _ in 0..48_000 * 2 {
            beat = m.advance(beat, 1.0 / 48_000.0);
        }
        // 1 s at 120 (2 beats) + 1 s at 90 (1.5 beats).
        assert!((beat - 3.5).abs() < 1e-6, "{beat}");
    }

    #[test]
    fn messy_input_is_normalised() {
        let m = TempoMap::new(
            vec![tempo(8.0, 100.0), tempo(2.0, 3000.0), tempo(8.0, 80.0)],
            vec![sig(0, 3, 5)],
        );
        // The first event holds from the start; out-of-range tempo is clamped.
        assert_eq!(m.tempo_at(0.0), MAX_BPM);
        // Last of two events at the same beat wins.
        assert_eq!(m.tempo_at(8.0), 80.0);
        assert_eq!(m.signature_at(0.0), (3, 4));

        let empty = TempoMap::new(vec![], vec![]);
        assert_eq!(
            (empty.tempo_at(0.0), empty.signature_at(0.0)),
            (120.0, (4, 4))
        );
    }

    #[test]
    fn signature_changes_move_bar_lines() {
        // Bars 0-1 in 4/4 (8 beats), then 6/8 (3 quarter notes per bar).
        let m = TempoMap::new(vec![], vec![sig(0, 4, 4), sig(2, 6, 8)]);
        assert!(close(m.beat_of_bar(2), 8.0));
        assert!(close(m.beat_of_bar(3), 11.0));
        assert_eq!(m.signature_at(7.9), (4, 4));
        assert_eq!(m.signature_at(8.0), (6, 8));

        let p = m.position(11.75);
        assert_eq!((p.bar, p.beat, p.num, p.den), (3, 1, 6, 8));
        assert!(close(p.fraction, 0.5));

        let p = m.position(5.0);
        assert_eq!((p.bar, p.beat), (1, 1));
        assert_eq!(m.position(-3.0).bar, 0);
    }
}
