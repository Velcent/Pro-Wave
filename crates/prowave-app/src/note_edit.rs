//! Pure note-editing operations for the Piano Roll (no UI): quantize, scale quantize,
//! marquee selection and hit testing. Kept separate so they can be unit tested.
use crate::model::Note;

/// Scale modes for Scale Quantize: name and semitone offsets from the root.
pub const MODES: [(&str, &[u8]); 7] = [
    ("Major", &[0, 2, 4, 5, 7, 9, 11]),
    ("Minor", &[0, 2, 3, 5, 7, 8, 10]),
    ("Dorian", &[0, 2, 3, 5, 7, 9, 10]),
    ("Mixolydian", &[0, 2, 4, 5, 7, 9, 10]),
    ("Major Pentatonic", &[0, 2, 4, 7, 9]),
    ("Minor Pentatonic", &[0, 3, 5, 7, 10]),
    ("Chromatic", &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
];

pub const ROOTS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Time Quantize grid choices: label and length in beats.
pub const GRIDS: [(&str, f32); 8] = [
    ("1/4 Note", 1.0),
    ("1/8 Note", 0.5),
    ("1/8 Triplet", 1.0 / 3.0),
    ("1/16 Note", 0.25),
    ("1/16 Triplet", 1.0 / 6.0),
    ("1/32 Note", 0.125),
    ("1/32 Triplet", 1.0 / 12.0),
    ("1/64 Note", 0.0625),
];

/// Moves the start of every selected note towards the grid.
/// `strength` 0..=1 (1 = exactly on the grid). `swing` 0..=1 delays every second grid step by
/// up to a third of the step (ProWave's mapping of Logic's Swing percentage).
pub fn quantize(notes: &mut [Note], sel: &[usize], grid: f32, strength: f32, swing: f32) {
    if grid <= 0.0 {
        return;
    }
    let strength = strength.clamp(0.0, 1.0);
    let delay = swing.clamp(0.0, 1.0) * grid / 3.0;
    for &i in sel {
        let Some(n) = notes.get_mut(i) else { continue };
        let step = (n.start / grid).round();
        let mut target = step * grid;
        if (step as i64).rem_euclid(2) == 1 {
            target += delay;
        }
        n.start = (n.start + (target - n.start) * strength).max(0.0);
    }
}

/// Nearest pitch inside the scale (ties resolve downwards).
pub fn scale_snap(pitch: u8, root: u8, offsets: &[u8]) -> u8 {
    if offsets.is_empty() {
        return pitch;
    }
    let in_scale = |p: i32| {
        let pc = (p - root as i32).rem_euclid(12) as u8;
        offsets.contains(&pc)
    };
    for d in 0..12i32 {
        for p in [pitch as i32 - d, pitch as i32 + d] {
            if (0..=127).contains(&p) && in_scale(p) {
                return p as u8;
            }
        }
    }
    pitch
}

pub fn scale_quantize(notes: &mut [Note], sel: &[usize], root: u8, offsets: &[u8]) {
    for &i in sel {
        if let Some(n) = notes.get_mut(i) {
            n.pitch = scale_snap(n.pitch, root, offsets);
        }
    }
}

/// Indices of notes that intersect the beat/pitch rectangle.
pub fn marquee(notes: &[Note], beat0: f32, beat1: f32, pitch0: u8, pitch1: u8) -> Vec<usize> {
    let (b0, b1) = (beat0.min(beat1), beat0.max(beat1));
    let (p0, p1) = (pitch0.min(pitch1), pitch0.max(pitch1));
    notes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.pitch >= p0 && n.pitch <= p1 && n.start < b1 && n.start + n.len > b0)
        .map(|(i, _)| i)
        .collect()
}

/// Topmost note under (beat, pitch). The bool is true when the point is on the note's right
/// edge (within `edge` beats), which starts a resize instead of a move.
pub fn hit_note(notes: &[Note], beat: f32, pitch: u8, edge: f32) -> Option<(usize, bool)> {
    notes
        .iter()
        .enumerate()
        .rev()
        .find(|(_, n)| n.pitch == pitch && beat >= n.start && beat <= n.start + n.len)
        .map(|(i, n)| (i, beat >= n.start + n.len - edge.min(n.len * 0.5)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(start: f32, pitch: u8) -> Note {
        Note {
            start,
            len: 0.5,
            pitch,
            vel: 90,
        }
    }

    #[test]
    fn quantize_full_strength_snaps_to_grid() {
        let mut notes = vec![note(0.9, 60), note(1.3, 62)];
        quantize(&mut notes, &[0, 1], 0.5, 1.0, 0.0);
        assert!((notes[0].start - 1.0).abs() < 1e-6);
        assert!((notes[1].start - 1.5).abs() < 1e-6);
    }

    #[test]
    fn quantize_half_strength_moves_halfway_and_only_selected() {
        let mut notes = vec![note(0.9, 60), note(0.9, 62)];
        quantize(&mut notes, &[0], 1.0, 0.5, 0.0);
        assert!((notes[0].start - 0.95).abs() < 1e-6);
        assert!((notes[1].start - 0.9).abs() < 1e-6);
    }

    #[test]
    fn swing_delays_odd_steps_only() {
        let mut notes = vec![note(0.0, 60), note(0.5, 60), note(1.0, 60), note(1.5, 60)];
        quantize(&mut notes, &[0, 1, 2, 3], 0.5, 1.0, 1.0);
        let d = 0.5 / 3.0;
        let want = [0.0, 0.5 + d, 1.0, 1.5 + d];
        for (n, w) in notes.iter().zip(want) {
            assert!((n.start - w).abs() < 1e-5, "{} vs {w}", n.start);
        }
    }

    #[test]
    fn scale_snap_c_major() {
        let major = MODES[0].1;
        assert_eq!(scale_snap(61, 0, major), 60); // C# -> C (tie goes down)
        assert_eq!(scale_snap(63, 0, major), 62); // D# -> D
        assert_eq!(scale_snap(64, 0, major), 64); // E stays
        assert_eq!(scale_snap(70, 0, major), 69); // A# -> A
                                                  // Other roots: D major contains F# but not F.
        assert_eq!(scale_snap(65, 2, major), 64); // F -> E
        assert_eq!(scale_snap(66, 2, major), 66);
    }

    #[test]
    fn scale_quantize_changes_only_selected() {
        let mut notes = vec![note(0.0, 61), note(1.0, 61)];
        scale_quantize(&mut notes, &[1], 0, MODES[0].1);
        assert_eq!((notes[0].pitch, notes[1].pitch), (61, 60));
    }

    #[test]
    fn marquee_picks_intersecting_notes() {
        let notes = vec![note(0.0, 60), note(2.0, 64), note(2.0, 72)];
        assert_eq!(marquee(&notes, 1.9, 3.0, 62, 66), vec![1]);
        assert_eq!(marquee(&notes, 3.0, 1.9, 66, 62), vec![1]);
        assert!(marquee(&notes, 5.0, 6.0, 0, 127).is_empty());
    }

    #[test]
    fn hit_note_detects_body_and_right_edge() {
        let notes = vec![Note {
            start: 1.0,
            len: 2.0,
            pitch: 60,
            vel: 100,
        }];
        assert_eq!(hit_note(&notes, 1.5, 60, 0.2), Some((0, false)));
        assert_eq!(hit_note(&notes, 2.9, 60, 0.2), Some((0, true)));
        assert_eq!(hit_note(&notes, 1.5, 61, 0.2), None);
        assert_eq!(hit_note(&notes, 3.5, 60, 0.2), None);
    }
}
