//! Disk streaming for audio regions (Logic Pro equivalent: audio file playback from disk).
//!
//! One background thread decodes with `symphonia`, resamples to the device rate (linear) and
//! pushes fixed-size [`Block`]s into a lock-free ring per clip. The audio thread only pops
//! blocks; it never touches the file system. Seeks are requested through an atomic
//! `(generation, frame)` word; blocks carry the generation they were produced for so stale
//! data is discarded without any locking.
use crate::core::AudioClip;
use rtrb::{Consumer, Producer, RingBuffer};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const BLOCK_FRAMES: usize = 1024;
/// ~1.4 s of prefetch per clip at 48 kHz.
pub const RING_BLOCKS: usize = 64;
const BLOCKS_PER_SERVICE: usize = 8;
const MAX_SAMPLE_RATE: u32 = 192_000;
const MAX_CHANNELS: usize = 64;

fn validate_format(sample_rate: u32, channels: usize) -> Result<(), AudioError> {
    if !(1..=MAX_SAMPLE_RATE).contains(&sample_rate) || !(1..=MAX_CHANNELS).contains(&channels) {
        return Err(AudioError::Decode(format!(
            "unsupported audio format: {sample_rate} Hz, {channels} channels (limits: 1-{MAX_SAMPLE_RATE} Hz, 1-{MAX_CHANNELS} channels)"
        )));
    }
    Ok(())
}

fn finite_sample(sample: f32) -> f32 {
    if sample.is_finite() {
        sample
    } else {
        0.0
    }
}

/// A chunk of interleaved stereo audio in the device sample rate.
pub struct Block {
    pub gen: u16,
    /// Index of the first frame, relative to the clip start (device-rate frames).
    pub start: u64,
    pub frames: u32,
    pub data: [f32; BLOCK_FRAMES * 2],
}

/// Audio-thread -> disk-thread seek request, packed as `gen << 48 | frame`.
#[derive(Default)]
pub struct ClipShared {
    request: AtomicU64,
}

impl ClipShared {
    pub fn request(&self, gen: u16, frame: u64) {
        self.request.store(
            ((gen as u64) << 48) | (frame & 0x0000_FFFF_FFFF_FFFF),
            Ordering::Release,
        );
    }
    fn load(&self) -> (u16, u64) {
        let v = self.request.load(Ordering::Acquire);
        ((v >> 48) as u16, v & 0x0000_FFFF_FFFF_FFFF)
    }
}

#[derive(Debug)]
pub enum AudioError {
    Io(std::io::Error),
    Decode(String),
    /// The container does not report a length, so the clip length is unknown.
    UnknownLength,
    NoTrack,
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::Decode(e) => write!(f, "decode error: {e}"),
            Self::UnknownLength => write!(f, "file length unknown"),
            Self::NoTrack => write!(f, "no audio track in file"),
        }
    }
}

impl std::error::Error for AudioError {}

impl From<std::io::Error> for AudioError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<SymError> for AudioError {
    fn from(e: SymError) -> Self {
        Self::Decode(e.to_string())
    }
}

/// Metadata of an audio file on disk (WAV, AIFF, FLAC).
#[derive(Clone, Debug)]
pub struct AudioFile {
    pub path: PathBuf,
    pub sample_rate: u32,
    /// Length in frames at the file's own sample rate.
    pub frames: u64,
    pub channels: usize,
}

impl AudioFile {
    /// Opens an audio region's source (Logic: Import Audio). Rejects zero/unsupported
    /// sample rates and channel counts before creating the decoder; see SECURITY.md limits.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AudioError> {
        let path = path.as_ref();
        let src = Source::open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            sample_rate: src.sample_rate,
            frames: src.frames,
            channels: src.channels,
        })
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames as f64 / self.sample_rate.max(1) as f64
    }

    /// Min/max overview for waveform display. Decodes the whole file: run off the UI thread.
    /// Non-finite source samples are replaced with silence, as in streamed playback.
    pub fn peaks(&self, bins: usize) -> Result<Vec<(f32, f32)>, AudioError> {
        let bins = bins.max(1);
        let mut out = vec![(0f32, 0f32); bins];
        let mut src = Source::open(&self.path)?;
        let mut buf = Vec::new();
        let mut frame = 0u64;
        let total = self.frames.max(1);
        while src.next_stereo(&mut buf)? {
            for f in buf.as_chunks::<2>().0 {
                let m = (f[0] + f[1]) * 0.5;
                let b = ((frame * bins as u64) / total).min(bins as u64 - 1) as usize;
                let e = &mut out[b];
                e.0 = e.0.min(m);
                e.1 = e.1.max(m);
                frame += 1;
            }
            buf.clear();
        }
        Ok(out)
    }
}

/// Sequential decoder returning interleaved stereo f32 at the file's sample rate.
struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    frames: u64,
    channels: usize,
    sbuf: Option<SampleBuffer<f32>>,
}

impl Source {
    fn open(path: &Path) -> Result<Self, AudioError> {
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let format = probed.format;
        let track = format.default_track().ok_or(AudioError::NoTrack)?;
        let params = track.codec_params.clone();
        let sample_rate = params.sample_rate.ok_or(AudioError::NoTrack)?;
        let frames = params.n_frames.ok_or(AudioError::UnknownLength)?;
        let channels = params
            .channels
            .map(|c| c.count())
            .ok_or(AudioError::NoTrack)?;
        validate_format(sample_rate, channels)?;
        let decoder = symphonia::default::get_codecs().make(&params, &DecoderOptions::default())?;
        Ok(Self {
            track_id: track.id,
            format,
            decoder,
            sample_rate,
            frames,
            channels,
            sbuf: None,
        })
    }

    /// Appends the next packet as stereo frames. `Ok(false)` at end of stream.
    fn next_stereo(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(false)
                }
                Err(SymError::ResetRequired) => return Ok(false),
                Err(e) => return Err(e.into()),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                Err(SymError::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            };
            let spec = *decoded.spec();
            validate_format(spec.rate, spec.channels.count())?;
            let cap = decoded.capacity() as u64;
            let sbuf = match &mut self.sbuf {
                Some(b) if b.capacity() as u64 >= cap * spec.channels.count() as u64 => b,
                slot => slot.insert(SampleBuffer::<f32>::new(cap, spec)),
            };
            sbuf.copy_interleaved_ref(decoded);
            let nch = spec.channels.count().max(1);
            for f in sbuf.samples().chunks_exact(nch) {
                // Float WAV/AIFF samples can contain NaN/Inf. Keep them out of the
                // resampler and mixer; ordinary finite values above unity remain valid.
                let l = finite_sample(f[0]);
                out.push(l);
                out.push(if nch > 1 { finite_sample(f[1]) } else { l });
            }
            return Ok(true);
        }
    }

    /// Seeks to `ts` (source frames); returns the frame the stream actually starts at.
    fn seek(&mut self, ts: u64) -> Result<u64, AudioError> {
        let to = self.format.seek(
            SeekMode::Accurate,
            SeekTo::TimeStamp {
                ts,
                track_id: self.track_id,
            },
        )?;
        self.decoder.reset();
        Ok(to.actual_ts)
    }
}

enum Msg {
    Register {
        file: AudioFile,
        producer: Producer<Block>,
        shared: Arc<ClipShared>,
        out_rate: u32,
        out_len: u64,
    },
    Quit,
}

enum Progress {
    Idle,
    Worked,
    Gone,
}

struct Stream {
    src: Source,
    producer: Producer<Block>,
    shared: Arc<ClipShared>,
    ratio: f64,
    out_len: u64,
    gen: Option<u16>,
    out_frame: u64,
    buf: Vec<f32>,
    buf_base: u64,
    eof: bool,
}

impl Stream {
    fn seek_out(&mut self, frame: u64) {
        self.out_frame = frame.min(self.out_len);
        let s = (self.out_frame as f64 * self.ratio) as u64;
        self.buf.clear();
        self.eof = false;
        match self.src.seek(s) {
            Ok(actual) => self.buf_base = actual,
            Err(e) => {
                tracing::error!("disk seek failed: {e}");
                self.out_frame = self.out_len; // stop producing
            }
        }
    }

    fn ensure(&mut self, upto: u64) {
        while !self.eof && self.buf_base + (self.buf.len() / 2) as u64 <= upto {
            match self.src.next_stereo(&mut self.buf) {
                Ok(true) => {}
                Ok(false) => self.eof = true,
                Err(e) => {
                    tracing::error!("disk decode failed: {e}");
                    self.eof = true;
                }
            }
        }
    }

    fn frame(&self, i: u64) -> (f32, f32) {
        let n = self.buf.len() / 2;
        if n == 0 {
            return (0.0, 0.0);
        }
        let idx = (i.saturating_sub(self.buf_base) as usize).min(n - 1);
        (self.buf[idx * 2], self.buf[idx * 2 + 1])
    }

    fn fill_block(&mut self, gen: u16) -> Block {
        let n = (BLOCK_FRAMES as u64).min(self.out_len - self.out_frame) as usize;
        let mut block = Block {
            gen,
            start: self.out_frame,
            frames: n as u32,
            data: [0.0; BLOCK_FRAMES * 2],
        };
        for i in 0..n {
            let pos = (self.out_frame + i as u64) as f64 * self.ratio;
            let i0 = pos.floor() as u64;
            let frac = (pos - i0 as f64) as f32;
            self.ensure(i0 + 1);
            let (l0, r0) = self.frame(i0);
            let (l1, r1) = self.frame(i0 + 1);
            block.data[i * 2] = l0 + (l1 - l0) * frac;
            block.data[i * 2 + 1] = r0 + (r1 - r0) * frac;
        }
        self.out_frame += n as u64;
        // Drop source frames we will never read again.
        let keep_from = ((self.out_frame as f64 * self.ratio).floor() as u64).saturating_sub(1);
        if keep_from > self.buf_base {
            let drop_frames = ((keep_from - self.buf_base) as usize).min(self.buf.len() / 2);
            self.buf.drain(..drop_frames * 2);
            self.buf_base += drop_frames as u64;
        }
        block
    }

    fn service(&mut self) -> Progress {
        if self.producer.is_abandoned() {
            return Progress::Gone;
        }
        let (gen, frame) = self.shared.load();
        if self.gen != Some(gen) {
            self.gen = Some(gen);
            self.seek_out(frame);
        }
        let mut worked = false;
        for _ in 0..BLOCKS_PER_SERVICE {
            if self.producer.slots() == 0 || self.out_frame >= self.out_len {
                break;
            }
            let block = self.fill_block(gen);
            if self.producer.push(block).is_err() {
                break;
            }
            worked = true;
        }
        if worked {
            Progress::Worked
        } else {
            Progress::Idle
        }
    }
}

/// Owns the streaming thread. Dropping it stops the thread.
pub struct DiskThread {
    tx: mpsc::Sender<Msg>,
    handle: Option<JoinHandle<()>>,
}

impl DiskThread {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<Msg>();
        let handle = std::thread::Builder::new()
            .name("prowave-disk".into())
            .spawn(move || run(rx))
            .map_err(|e| tracing::error!("cannot spawn disk thread: {e}"))
            .ok();
        Self { tx, handle }
    }

    /// Creates a streamed clip placed at `start_beat` on `track`. The file is opened on the
    /// disk thread; if that fails the clip simply stays silent.
    pub fn stream(&self, file: &AudioFile, out_rate: u32, track: u8, start_beat: f64) -> AudioClip {
        let out_len =
            (file.frames as f64 * out_rate as f64 / file.sample_rate.max(1) as f64) as u64;
        let (producer, consumer): (Producer<Block>, Consumer<Block>) = RingBuffer::new(RING_BLOCKS);
        let shared = Arc::new(ClipShared::default());
        shared.request(0, 0);
        let msg = Msg::Register {
            file: file.clone(),
            producer,
            shared: shared.clone(),
            out_rate,
            out_len,
        };
        if self.tx.send(msg).is_err() {
            tracing::error!("disk thread is not running");
        }
        AudioClip::new(track, start_beat, out_len, consumer, shared)
    }
}

impl Drop for DiskThread {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Quit);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Applies one message; `false` means quit.
fn handle(msg: Msg, streams: &mut Vec<Stream>) -> bool {
    match msg {
        Msg::Register {
            file,
            producer,
            shared,
            out_rate,
            out_len,
        } => match Source::open(&file.path) {
            Ok(src) => streams.push(Stream {
                ratio: src.sample_rate as f64 / out_rate.max(1) as f64,
                src,
                producer,
                shared,
                out_len,
                gen: None,
                out_frame: 0,
                buf: Vec::new(),
                buf_base: 0,
                eof: false,
            }),
            Err(e) => tracing::error!("cannot open {}: {e}", file.path.display()),
        },
        Msg::Quit => return false,
    }
    true
}

fn run(rx: mpsc::Receiver<Msg>) {
    let mut streams: Vec<Stream> = Vec::new();
    loop {
        // Nothing to stream (no audio regions): block until a clip arrives instead of
        // waking up every 2 ms.
        if streams.is_empty() {
            let Ok(msg) = rx.recv() else { return };
            if !handle(msg, &mut streams) {
                return;
            }
        }
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    if !handle(msg, &mut streams) {
                        return;
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => return,
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        let mut any = false;
        streams.retain_mut(|s| match s.service() {
            Progress::Gone => false,
            Progress::Worked => {
                any = true;
                true
            }
            Progress::Idle => true,
        });
        if !any {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::core::{Command, EngineCore, Shared, Song, TrackMix};
    use std::io::Write;
    use std::sync::atomic::Ordering;

    const SRC_RATE: u32 = 44_100;
    const OUT_RATE: u32 = 48_000;

    #[test]
    fn format_limits_reject_invalid_metadata() {
        for rate in [1, 44_100, 48_000, MAX_SAMPLE_RATE] {
            for channels in [1, 2, MAX_CHANNELS] {
                assert!(validate_format(rate, channels).is_ok());
            }
        }
        for (rate, channels) in [(0, 2), (MAX_SAMPLE_RATE + 1, 2), (48_000, 0), (48_000, 65)] {
            assert!(validate_format(rate, channels).is_err());
        }
    }

    #[test]
    fn non_finite_samples_become_silence_without_clipping_finite_audio() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(finite_sample(value), 0.0);
        }
        for value in [-2.0, -0.25, 0.0, 0.25, 2.0] {
            assert_eq!(finite_sample(value), value);
        }
    }

    /// 1 s, mono, 16-bit PCM, 440 Hz sine at amplitude 0.5.
    fn write_sine_wav(dir: &Path) -> PathBuf {
        let path = dir.join("sine.wav");
        let n = SRC_RATE as usize;
        let mut bytes = Vec::with_capacity(44 + n * 2);
        let data_len = (n * 2) as u32;
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&SRC_RATE.to_le_bytes());
        bytes.extend_from_slice(&(SRC_RATE * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..n {
            let s = (i as f32 / SRC_RATE as f32 * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            bytes.extend_from_slice(&((s * 32767.0) as i16).to_le_bytes());
        }
        let mut f = File::create(&path).unwrap();
        f.write_all(&bytes).unwrap();
        path
    }

    struct Rig {
        core: EngineCore,
        tx: Producer<Command>,
        shared: Arc<Shared>,
        _garbage: Consumer<crate::core::Garbage>,
        _disk: DiskThread,
        _dir: tempfile::TempDir,
    }

    fn rig() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let file = AudioFile::open(write_sine_wav(dir.path())).unwrap();
        let disk = DiskThread::spawn();
        let clip = disk.stream(&file, OUT_RATE, 0, 0.0);
        let (mut tx, rx) = RingBuffer::new(64);
        let (gtx, grx) = RingBuffer::new(8);
        let shared = Arc::new(Shared::new());
        let core = EngineCore::new(OUT_RATE, rx, gtx, shared.clone());
        let song = Song::new(vec![], vec![TrackMix::default()]).with_clips(vec![clip]);
        assert!(tx.push(Command::SetSong(Box::new(song))).is_ok());
        Rig {
            core,
            tx,
            shared,
            _garbage: grx,
            _disk: disk,
            _dir: dir,
        }
    }

    /// Runs `secs` of audio in 512-frame callbacks and returns the left channel.
    fn run(r: &mut Rig, secs: f32) -> Vec<f32> {
        let mut buf = [0f32; 1024];
        let mut left = Vec::new();
        for _ in 0..((secs * OUT_RATE as f32) as usize / 512) {
            assert_no_alloc::assert_no_alloc(|| r.core.process(&mut buf));
            left.extend(buf.iter().step_by(2));
        }
        left
    }

    fn prefetch(r: &mut Rig) {
        run(r, 0.02); // applies queued commands (seek requests)
        std::thread::sleep(Duration::from_millis(300));
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    fn crossings(x: &[f32]) -> usize {
        x.windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count()
    }

    #[test]
    fn reads_wav_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let f = AudioFile::open(write_sine_wav(dir.path())).unwrap();
        assert_eq!(
            (f.sample_rate, f.frames, f.channels),
            (SRC_RATE, SRC_RATE as u64, 1)
        );
        let peaks = f.peaks(50).unwrap();
        let max = peaks.iter().map(|p| p.1).fold(0.0, f32::max);
        assert!((max - 0.5).abs() < 0.02, "{max}");
    }

    #[test]
    fn streams_resampled_region_without_underruns() {
        let mut r = rig();
        prefetch(&mut r);
        assert!(r.tx.push(Command::Play).is_ok());
        let out = run(&mut r, 0.5);
        // 0.5 amplitude * 0.8 master, sine RMS = A/sqrt(2)
        let expected = 0.5 * 0.8 / std::f32::consts::SQRT_2;
        assert!(
            (rms(&out) - expected).abs() < expected * 0.05,
            "rms {}",
            rms(&out)
        );
        let c = crossings(&out) as f32 / (out.len() as f32 / OUT_RATE as f32);
        assert!((c / 2.0 - 440.0).abs() < 5.0, "freq {}", c / 2.0);
        assert!(out.iter().all(|x| x.is_finite()));
        assert_eq!(r.shared.underruns.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn seek_into_region_then_silence_after_end() {
        let mut r = rig();
        // 120 BPM: beat 1.5 = 0.75 s into the 1 s file.
        assert!(r.tx.push(Command::SetPosition { beat: 1.5 }).is_ok());
        prefetch(&mut r);
        assert!(r.tx.push(Command::Play).is_ok());
        let out = run(&mut r, 0.6);
        let head = &out[..(0.2 * OUT_RATE as f32) as usize];
        let tail = &out[(0.35 * OUT_RATE as f32) as usize..];
        assert!(rms(head) > 0.25, "head rms {}", rms(head));
        assert!(rms(tail) == 0.0, "tail rms {}", rms(tail));
        assert_eq!(r.shared.underruns.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn muted_track_silences_region() {
        let mut r = rig();
        let mix = TrackMix {
            gain_db: 0.0,
            mute: true,
            solo: false,
            pan: 0.0,
        };
        assert!(r.tx.push(Command::SetTrackMix { track: 0, mix }).is_ok());
        prefetch(&mut r);
        assert!(r.tx.push(Command::Play).is_ok());
        assert_eq!(rms(&run(&mut r, 0.3)), 0.0);
    }
}
