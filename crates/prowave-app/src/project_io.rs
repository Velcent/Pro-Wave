//! Converts the app's project model to and from the `.prowave` document (`prowave-project`).
//!
//! Audio regions point at files on disk; saving copies each distinct file into the archive
//! (`media/…`) and loading extracts them into a per-project folder under the temp directory,
//! where the disk-streaming engine reads them.
use crate::import::audio_region;
use crate::library::CATEGORIES;
use crate::model::{
    Marker, Note, Project, Region, RegionParams, SendSlot, TempoPoint, Track, TrackKind,
    TrackParams, MAX_REGION_GAIN, MIN_REGION_GAIN,
};
use egui::Color32;
use prowave_engine::{AudioFile, FadeCurve};
use prowave_project::{
    media_path, MarkerDoc, MediaFile, NoteDoc, ProjectDoc, RegionDoc, TempoChangeDoc, TrackDoc,
    TrackKindDoc,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn kind_to_doc(k: TrackKind) -> TrackKindDoc {
    match k {
        TrackKind::Instrument => TrackKindDoc::Instrument,
        TrackKind::Audio => TrackKindDoc::Audio,
        TrackKind::Drummer => TrackKindDoc::Drummer,
        TrackKind::Bus => TrackKindDoc::Bus,
        TrackKind::Output => TrackKindDoc::Output,
        TrackKind::Aux => TrackKindDoc::Aux,
        TrackKind::Folder => TrackKindDoc::Folder,
    }
}

fn kind_from_doc(k: TrackKindDoc) -> TrackKind {
    match k {
        TrackKindDoc::Instrument => TrackKind::Instrument,
        TrackKindDoc::Audio => TrackKind::Audio,
        TrackKindDoc::Drummer => TrackKind::Drummer,
        TrackKindDoc::Bus => TrackKind::Bus,
        TrackKindDoc::Output => TrackKind::Output,
        TrackKindDoc::Aux => TrackKind::Aux,
        TrackKindDoc::Folder => TrackKind::Folder,
    }
}

/// Names the model keeps as `&'static str` (plug-ins, categories, keys). Known names map to
/// the built-in static strings; anything else (from a hand-edited file) is leaked once per
/// distinct name for the whole session, so reopening files never leaks more.
fn intern(s: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static OTHERS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    if let Some(k) = builtin_name(s) {
        return k;
    }
    let mut others = match OTHERS.get_or_init(Default::default).lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(k) = others.get(s) {
        return k;
    }
    let k: &'static str = Box::leak(s.to_string().into_boxed_str());
    others.insert(k);
    k
}

fn builtin_name(s: &str) -> Option<&'static str> {
    const EXTRA: &[&str] = &[
        "E-Piano",
        "Sampler",
        "Alchemy",
        "Drum Kit",
        "Drum Machine",
        "Tape Delay",
        "Channel EQ",
        "Compressor",
        "Chorus",
        "Space D",
        "MultiMeter",
        "Cmaj",
        "Cmin",
        "Amin",
        "Gmaj",
        "Dmaj",
        "Fmaj",
        "Emin",
    ];
    if let Some(k) = EXTRA.iter().find(|k| **k == s) {
        return Some(k);
    }
    for c in CATEGORIES {
        if c.name == s {
            return Some(c.name);
        }
        if c.instrument == Some(s) {
            return c.instrument;
        }
        if let Some(fx) = c.fx.iter().find(|f| **f == s) {
            return Some(fx);
        }
    }
    None
}

/// Logic's longest project: 9999 bars.
const MAX_BARS: f32 = 9999.0;
/// Longest note / latest note start inside a region, in beats.
const MAX_BEATS: f32 = MAX_BARS * 99.0 * 4.0;

/// Values read from a file are untrusted: keep them in the ranges the editors and the engine
/// assume (a zero-length region or a 0 signature divides by zero, a pitch above 127 overflows
/// note arithmetic, a reversed key limit panics in `clamp`).
fn finite_in(x: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if x.is_finite() {
        x.clamp(lo, hi)
    } else {
        default
    }
}

fn ordered(range: (u8, u8), min: u8) -> (u8, u8) {
    let (a, b) = (range.0.clamp(min, 127), range.1.clamp(min, 127));
    (a.min(b), a.max(b))
}

fn signature(sig: (u8, u8)) -> (u8, u8) {
    let den = if matches!(sig.1, 1 | 2 | 4 | 8 | 16 | 32 | 64) {
        sig.1
    } else {
        4
    };
    (sig.0.clamp(1, 99), den)
}

/// The document for `p` plus the media files to embed (each distinct audio file once).
pub fn to_doc(p: &Project) -> (ProjectDoc, Vec<MediaFile>) {
    let mut media: Vec<MediaFile> = Vec::new();
    let mut by_source: HashMap<PathBuf, String> = HashMap::new();
    let tracks = p
        .tracks
        .iter()
        .map(|t| TrackDoc {
            name: t.name.clone(),
            kind: kind_to_doc(t.kind),
            color: [t.color.r(), t.color.g(), t.color.b()],
            mute: t.mute,
            solo: t.solo,
            volume_db: t.volume_db,
            pan: t.pan,
            inserts: t.inserts.iter().map(|s| s.to_string()).collect(),
            instrument: t.instrument.map(str::to_string),
            category: t.category.map(str::to_string),
            transpose: t.params.transpose,
            velocity: t.params.velocity,
            key_limit: t.params.key_limit,
            vel_limit: t.params.vel_limit,
            send_bus: t.send.bus,
            send_level: t.send.level,
            send_pre: t.send.pre,
            height: t.height,
            indent: t.indent,
            folded: t.folded,
            regions: t
                .regions
                .iter()
                .map(|r| RegionDoc {
                    name: r.name.clone(),
                    start_bar: r.start_bar,
                    len_bars: r.len_bars,
                    notes: r
                        .notes
                        .iter()
                        .map(|n| NoteDoc {
                            start: n.start,
                            len: n.len,
                            pitch: n.pitch,
                            vel: n.vel,
                        })
                        .collect(),
                    audio: r.audio.as_ref().map(|a| {
                        by_source
                            .entry(a.file.path.clone())
                            .or_insert_with(|| {
                                let zip_path = media_path(media.len(), &a.file.path);
                                media.push(MediaFile {
                                    zip_path: zip_path.clone(),
                                    source: a.file.path.clone(),
                                });
                                zip_path
                            })
                            .clone()
                    }),
                    mute: r.params.mute,
                    quantize: r.params.quantize,
                    transpose: r.params.transpose,
                    velocity: r.params.velocity,
                    gain_db: r.params.gain_db,
                    fade_in: r.params.fade_in,
                    fade_out: r.params.fade_out,
                    fade_curve: curve_name(r.params.fade_curve).into(),
                    reverse: r.params.reverse,
                })
                .collect(),
        })
        .collect();
    let doc = ProjectDoc {
        name: p.name.clone(),
        tempo: p.tempo,
        sig: p.sig,
        key: p.key.to_string(),
        position_beats: p.position_beats,
        cycle: p.cycle,
        cycle_start: p.cycle_start,
        cycle_end: p.cycle_end,
        metronome: p.metronome,
        count_in: p.count_in,
        master_vol: p.master_vol,
        selected_track: p.selected_track,
        tracks,
        tempo_changes: p
            .tempo_changes
            .iter()
            .map(|t| TempoChangeDoc {
                beat: t.beat,
                bpm: t.bpm,
            })
            .collect(),
        markers: p
            .markers
            .iter()
            .map(|m| MarkerDoc {
                beat: m.beat,
                name: m.name.clone(),
                color: [m.color.r(), m.color.g(), m.color.b()],
            })
            .collect(),
        ..ProjectDoc::default()
    };
    (doc, media)
}

/// Builds a project from `doc`; audio regions open their files from `media_dir`. Regions whose
/// file is missing or unreadable are dropped and reported in the returned warnings.
pub fn from_doc(doc: &ProjectDoc, media_dir: &Path, ctx: &egui::Context) -> (Project, Vec<String>) {
    let mut warnings = Vec::new();
    let mut opened: HashMap<String, Option<AudioFile>> = HashMap::new();
    let mut tracks = Vec::new();
    for t in &doc.tracks {
        let mut regions = Vec::new();
        for r in &t.regions {
            let audio = match &r.audio {
                None => None,
                Some(zip_path) if !prowave_project::is_media_path(zip_path) => {
                    // Never follow a path out of the project's media (e.g. `C:\…`, `../…`).
                    warnings.push(format!("{zip_path}: not a media path in the project"));
                    continue;
                }
                Some(zip_path) => {
                    let file = opened.entry(zip_path.clone()).or_insert_with(|| {
                        match AudioFile::open(media_dir.join(zip_path)) {
                            Ok(f) => Some(f),
                            Err(e) => {
                                warnings.push(format!("{}: {e}", zip_path));
                                None
                            }
                        }
                    });
                    match file {
                        Some(f) => Some(audio_region(f.clone(), ctx)),
                        None => continue,
                    }
                }
            };
            regions.push(Region {
                name: r.name.clone(),
                start_bar: finite_in(r.start_bar, 1.0, MAX_BARS, 1.0),
                len_bars: finite_in(r.len_bars, 1.0 / 64.0, MAX_BARS, 1.0),
                notes: r
                    .notes
                    .iter()
                    .map(|n| Note {
                        start: finite_in(n.start, 0.0, MAX_BEATS, 0.0),
                        len: finite_in(n.len, 1.0 / 960.0, MAX_BEATS, 0.25),
                        pitch: n.pitch.min(127),
                        vel: n.vel.clamp(1, 127),
                    })
                    .collect(),
                audio,
                params: RegionParams {
                    mute: r.mute,
                    quantize: r.quantize.filter(|q| q.is_finite() && *q > 0.0),
                    transpose: r.transpose,
                    velocity: r.velocity,
                    gain_db: finite_in(r.gain_db, MIN_REGION_GAIN, MAX_REGION_GAIN, 0.0),
                    fade_in: finite_in(r.fade_in, 0.0, MAX_FADE, 0.0),
                    fade_out: finite_in(r.fade_out, 0.0, MAX_FADE, 0.0),
                    fade_curve: curve_from_name(&r.fade_curve),
                    reverse: r.reverse,
                },
            });
        }
        tracks.push(Track {
            name: t.name.clone(),
            kind: kind_from_doc(t.kind),
            color: Color32::from_rgb(t.color[0], t.color[1], t.color[2]),
            mute: t.mute,
            solo: t.solo,
            rec: false,
            monitor: false,
            volume_db: t.volume_db.clamp(-60.0, 6.0),
            pan: t.pan.clamp(-1.0, 1.0),
            inserts: t.inserts.iter().map(|s| intern(s)).collect(),
            instrument: t.instrument.as_deref().map(intern),
            regions,
            params: TrackParams {
                transpose: t.transpose,
                velocity: t.velocity,
                key_limit: ordered(t.key_limit, 0),
                vel_limit: ordered(t.vel_limit, 1),
            },
            send: SendSlot {
                bus: t.send_bus,
                level: t.send_level.clamp(0.0, 1.0),
                pre: t.send_pre,
            },
            category: t.category.as_deref().map(intern),
            height: finite_in(t.height, Track::MIN_HEIGHT, Track::MAX_HEIGHT, 1.0),
            indent: t.indent,
            folded: t.folded,
        });
    }
    let mut p = Project::empty();
    p.name = doc.name.clone();
    p.tempo = doc.tempo.clamp(5.0, 990.0);
    p.tempo_changes = doc
        .tempo_changes
        .iter()
        .map(|t| TempoPoint {
            beat: t.beat,
            bpm: t.bpm,
        })
        .collect();
    p.tidy_tempo();
    p.markers = doc
        .markers
        .iter()
        .map(|m| Marker {
            beat: m.beat,
            name: m.name.chars().take(64).collect(),
            color: Color32::from_rgb(m.color[0], m.color[1], m.color[2]),
        })
        .collect();
    p.tidy_markers();
    p.sig = signature(doc.sig);
    p.key = intern(&doc.key);
    p.position_beats = doc.position_beats.max(0.0);
    p.cycle = doc.cycle;
    p.cycle_start = doc.cycle_start.max(0.0);
    p.cycle_end = doc.cycle_end.max(p.cycle_start);
    p.metronome = doc.metronome;
    p.count_in = doc.count_in;
    p.master_vol = doc.master_vol.clamp(0.0, 2.0);
    p.selected_track = doc.selected_track.min(tracks.len().saturating_sub(1));
    p.tracks = tracks;
    // A hand-edited file cannot nest tracks deeper than the folders above them allow.
    p.normalize_nesting();
    p.playing = false;
    p.recording = false;
    (p, warnings)
}

/// Longest fade a file may ask for, in beats.
const MAX_FADE: f32 = 4096.0;

fn curve_name(c: FadeCurve) -> &'static str {
    match c {
        FadeCurve::Linear => "linear",
        FadeCurve::EqualPower => "equal_power",
        FadeCurve::SCurve => "s_curve",
    }
}

fn curve_from_name(name: &str) -> FadeCurve {
    match name {
        "linear" => FadeCurve::Linear,
        "s_curve" => FadeCurve::SCurve,
        _ => FadeCurve::EqualPower,
    }
}

/// Root of all extracted project media.
fn media_root() -> PathBuf {
    std::env::temp_dir().join("prowave-media")
}

/// This process's media folder (`<temp>/prowave-media/<pid>`), removed on exit.
fn session_media_dir() -> PathBuf {
    media_root().join(std::process::id().to_string())
}

/// Folder the media of an opened project is extracted to (unique per open).
pub fn media_dir_for(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    session_media_dir().join(format!("{stem}-{n}"))
}

/// On exit (after the engine has closed its files): delete the media this session extracted.
pub fn remove_session_media() {
    let dir = session_media_dir();
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::warn!("cannot remove {}: {e}", dir.display());
        }
    }
}

/// On launch: delete media left by sessions that crashed more than `max_age` ago.
pub fn remove_stale_media(max_age: std::time::Duration) {
    let Ok(entries) = std::fs::read_dir(media_root()) else {
        return;
    };
    for e in entries.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > max_age);
        if old && e.path() != session_media_dir() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// Saves `p` to `path` (adds the `.prowave` extension if missing). Returns the final path.
pub fn save_project(p: &Project, path: &Path) -> Result<PathBuf, prowave_project::ProjectError> {
    let mut path = path.to_path_buf();
    if path.extension().and_then(|e| e.to_str()) != Some(prowave_project::EXTENSION) {
        path.set_extension(prowave_project::EXTENSION);
    }
    let (doc, media) = to_doc(p);
    prowave_project::save(&path, &doc, &media)?;
    Ok(path)
}

/// Opens a project file; returns the project and warnings about media that could not be read.
pub fn open_project(
    path: &Path,
    ctx: &egui::Context,
) -> Result<(Project, Vec<String>), prowave_project::ProjectError> {
    let media_dir = media_dir_for(path);
    let doc = prowave_project::load(path, &media_dir)?;
    let (mut p, warnings) = from_doc(&doc, &media_dir, ctx);
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        p.name = stem.to_string();
    }
    Ok((p, warnings))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod tests {
    use super::*;

    /// 0.5 s, mono, 16-bit, 44.1 kHz silence.
    pub(crate) fn write_wav(path: &Path) {
        let n = 22_050u32;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + n * 2).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&44_100u32.to_le_bytes());
        b.extend_from_slice(&88_200u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(n * 2).to_le_bytes());
        b.resize(b.len() + (n * 2) as usize, 0);
        std::fs::write(path, b).unwrap();
    }

    #[test]
    fn hand_edited_tempo_changes_are_sorted_and_cleaned() {
        let mut doc = to_doc(&Project::empty()).0;
        doc.tempo_changes = [(64.0, 90.0), (0.0, 50.0), (16.0, f32::NAN), (8.0, 2000.0)]
            .into_iter()
            .map(|(beat, bpm)| TempoChangeDoc { beat, bpm })
            .collect();
        let (p, _) = from_doc(&doc, Path::new("."), &egui::Context::default());
        let got: Vec<(f64, f32)> = p.tempo_changes.iter().map(|t| (t.beat, t.bpm)).collect();
        assert_eq!(got, vec![(8.0, 990.0), (64.0, 90.0)]);
    }

    #[test]
    fn save_and_open_round_trip_the_demo_with_edits() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut p = Project::demo();
        p.tempo = 101.0;
        p.add_tempo_change(32.0, 140.0);
        let m = p.add_marker(16.0);
        p.markers[m].name = "Chorus".into();
        p.cycle = true;
        p.tracks[0].regions[0].params.transpose = 5;
        p.tracks[4].regions[0].params.gain_db = -3.5;
        p.tracks[4].regions[0].params.fade_in = 2.0;
        p.tracks[4].regions[0].params.fade_curve = FadeCurve::SCurve;
        p.tracks[4].regions[0].params.reverse = true;
        p.tracks[0].params.key_limit = (40, 90);
        p.tracks[1].category = Some("Piano");
        p.tracks[2].send.pre = true;
        p.tracks[3].height = 2.5;
        let wav = dir.path().join("loop.wav");
        write_wav(&wav);
        crate::import::import_audio(&mut p, &wav, &ctx).unwrap();
        // The same file used twice is embedded once.
        let dup = p.tracks.last().unwrap().regions[0].clone();
        p.tracks.last_mut().unwrap().regions.push(Region {
            start_bar: 9.0,
            ..dup
        });

        let file = save_project(&p, &dir.path().join("song")).unwrap();
        assert_eq!(file.extension().unwrap(), "prowave");
        assert_eq!(to_doc(&p).1.len(), 1);

        let (q, warnings) = open_project(&file, &ctx).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(q.name, "song");
        assert_eq!((q.tempo, q.cycle), (101.0, true));
        assert_eq!(q.tempo_changes, p.tempo_changes, "the Tempo track is saved");
        assert_eq!(q.markers, p.markers, "and the Marker track");
        assert_eq!(q.tracks.len(), p.tracks.len());
        assert_eq!(
            q.tracks[0].regions[0].notes.len(),
            p.tracks[0].regions[0].notes.len()
        );
        assert_eq!(q.tracks[0].regions[0].params.transpose, 5);
        assert!(
            q.tracks[4].regions[0].params == p.tracks[4].regions[0].params,
            "region gain, fade and reverse are saved"
        );
        assert_eq!(q.tracks[0].params.key_limit, (40, 90));
        assert_eq!(q.tracks[1].category, Some("Piano"));
        assert!(q.tracks[2].send.pre);
        assert_eq!(q.tracks[3].height, 2.5);
        assert_eq!(q.tracks[0].inserts, p.tracks[0].inserts);
        let audio = q.tracks.last().unwrap();
        assert_eq!(audio.regions.len(), 2);
        assert!(audio.regions.iter().all(|r| r.audio.is_some()));
        assert_eq!(audio.regions[1].start_bar, 9.0);
    }

    #[test]
    fn aux_and_folder_tracks_and_their_nesting_survive_a_save() {
        let mut p = Project::demo();
        let (kinds, extra): (Vec<_>, Vec<_>) = (
            vec![TrackKind::Folder, TrackKind::Instrument, TrackKind::Aux],
            vec![(0u8, true), (1, false), (0, false)],
        );
        for ((kind, (indent, folded)), name) in
            kinds.into_iter().zip(extra).zip(["Stack", "In", "Rev"])
        {
            let mut t = Track::new(name, kind);
            (t.indent, t.folded) = (indent, folded);
            p.tracks.push(t);
        }
        let first_new = p.tracks.len() - 3;
        let (doc, _) = to_doc(&p);
        let (back, _) = from_doc(&doc, Path::new("."), &egui::Context::default());
        let got: Vec<_> = back.tracks[first_new..]
            .iter()
            .map(|t| (t.name.as_str(), t.kind, t.indent, t.folded))
            .collect();
        assert_eq!(
            got,
            [
                ("Stack", TrackKind::Folder, 0, true),
                ("In", TrackKind::Instrument, 1, false),
                ("Rev", TrackKind::Aux, 0, false),
            ]
        );
    }

    #[test]
    fn missing_media_drops_the_region_and_warns() {
        let dir = tempfile::tempdir().unwrap();
        let doc = ProjectDoc {
            tracks: vec![TrackDoc {
                name: "Vox".into(),
                kind: TrackKindDoc::Audio,
                regions: vec![RegionDoc {
                    name: "gone".into(),
                    audio: Some("media/0_gone.wav".into()),
                    ..RegionDoc::default()
                }],
                ..TrackDoc::default()
            }],
            ..ProjectDoc::default()
        };
        let (p, warnings) = from_doc(&doc, dir.path(), &egui::Context::default());
        assert_eq!(p.tracks.len(), 1);
        assert!(p.tracks[0].regions.is_empty());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn hostile_values_are_brought_into_range() {
        let dir = tempfile::tempdir().unwrap();
        // A real audio file outside the media folder that a crafted project points at.
        let outside = dir.path().join("private.wav");
        write_wav(&outside);
        let doc = ProjectDoc {
            sig: (0, 3),
            cycle_start: -8.0,
            cycle_end: -16.0,
            tracks: vec![TrackDoc {
                key_limit: (200, 10),
                vel_limit: (0, 0),
                height: 1000.0,
                regions: vec![
                    RegionDoc {
                        start_bar: f32::INFINITY,
                        len_bars: 0.0,
                        quantize: Some(0.0),
                        notes: vec![NoteDoc {
                            start: -4.0,
                            len: 0.0,
                            pitch: 255,
                            vel: 0,
                        }],
                        ..RegionDoc::default()
                    },
                    RegionDoc {
                        audio: Some(outside.to_string_lossy().into_owned()),
                        ..RegionDoc::default()
                    },
                    RegionDoc {
                        audio: Some("media/../../private.wav".into()),
                        ..RegionDoc::default()
                    },
                ],
                ..TrackDoc::default()
            }],
            ..ProjectDoc::default()
        };
        let media = dir.path().join("media_root");
        let (p, warnings) = from_doc(&doc, &media, &egui::Context::default());
        assert_eq!(p.sig, (1, 4));
        assert_eq!((p.cycle_start, p.cycle_end), (0.0, 0.0));
        let t = &p.tracks[0];
        assert_eq!(t.height, Track::MAX_HEIGHT);
        assert_eq!(
            (t.params.key_limit, t.params.vel_limit),
            ((10, 127), (1, 1))
        );
        assert_eq!(t.regions.len(), 1, "both escaping audio paths are dropped");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        let r = &t.regions[0];
        assert_eq!(
            (r.start_bar, r.len_bars, r.params.quantize),
            (1.0, 1.0 / 64.0, None)
        );
        let n = &r.notes[0];
        assert_eq!((n.start, n.pitch, n.vel), (0.0, 127, 1));
        assert!(n.len > 0.0);
    }

    #[test]
    fn unknown_names_are_leaked_once_per_session() {
        let a = intern("Hostile Plug-in Name");
        let b = intern(&String::from("Hostile Plug-in Name"));
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn known_names_intern_to_static_strings() {
        assert!(std::ptr::eq(
            intern("Piano"),
            CATEGORIES.iter().find(|c| c.name == "Piano").unwrap().name
        ));
        assert_eq!(
            intern("Some Third-Party Plug-in"),
            "Some Third-Party Plug-in"
        );
    }
}
