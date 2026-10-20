//! cpal output: owns the stream, the command producer and the shared state.
use crate::core::{dsp_load, AudioClip, ClipEdit, Command, EngineCore, Garbage, Shared};
use crate::disk::{AudioError, AudioFile, DiskThread};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, Error, ErrorKind, FromSample, OutputCallbackInfo, SampleFormat, SizedSample,
    Stream, StreamConfig, SupportedBufferSize,
};
use rtrb::{Producer, RingBuffer};
use std::fmt;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

const MAX_BLOCK_FRAMES: usize = 8192;
const GARBAGE_CAPACITY: usize = 32;

/// UI-side queue accounting: each accepted state replacement reserves one return slot.
struct CommandQueue {
    tx: Producer<Command>,
    garbage: rtrb::Consumer<Garbage>,
    credits: usize,
}

impl CommandQueue {
    fn send(&mut self, cmd: Command) -> bool {
        while self.garbage.pop().is_ok() {
            self.credits += 1;
        }
        let retires = cmd.retires_state();
        if retires && self.credits == 0 {
            return false;
        }
        if self.tx.push(cmd).is_err() {
            return false;
        }
        if retires {
            self.credits -= 1;
        }
        true
    }
}

#[cfg(test)]
mod queue_tests {
    use super::*;

    #[test]
    fn reserves_reclamation_capacity_and_recovers_after_collection() {
        let (tx, mut rx) = RingBuffer::new(8);
        let (mut gtx, garbage) = RingBuffer::new(1);
        let mut queue = CommandQueue {
            tx,
            garbage,
            credits: 1,
        };
        assert!(queue.send(Command::SetSong(Box::default())));
        assert!(!queue.send(Command::SetTempoMap(Box::default())));
        // Transport is still deliverable even when all reclamation credits are reserved.
        assert!(queue.send(Command::Stop));
        assert!(matches!(rx.pop(), Ok(Command::SetSong(_))));
        assert!(matches!(rx.pop(), Ok(Command::Stop)));
        assert!(gtx.push(Garbage::Song(Box::default())).is_ok());
        assert!(queue.send(Command::SetTempoMap(Box::default())));
        assert_eq!(queue.credits, 0);
        assert!(matches!(rx.pop(), Ok(Command::SetTempoMap(_))));
    }

    #[test]
    fn rejected_command_does_not_consume_a_reclamation_credit() {
        let (tx, mut rx) = RingBuffer::new(1);
        let (_gtx, garbage) = RingBuffer::new(1);
        let mut queue = CommandQueue {
            tx,
            garbage,
            credits: 1,
        };
        assert!(queue.send(Command::Play));
        assert!(!queue.send(Command::SetNotes(vec![])));
        assert_eq!(queue.credits, 1);
        assert!(rx.pop().is_ok());
        assert!(queue.send(Command::SetNotes(vec![])));
        assert_eq!(queue.credits, 0);
    }
}

#[derive(Debug)]
pub enum EngineError {
    NoOutputDevice,
    UnsupportedSampleFormat(String),
    Backend(String),
    AudioFile(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOutputDevice => write!(f, "no audio output device available"),
            Self::UnsupportedSampleFormat(s) => write!(f, "unsupported sample format {s}"),
            Self::Backend(s) => write!(f, "audio backend error: {s}"),
            Self::AudioFile(s) => write!(f, "audio file error: {s}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<AudioError> for EngineError {
    fn from(error: AudioError) -> Self {
        Self::AudioFile(error.to_string())
    }
}

/// Decodes `file` off the audio thread and returns its maximum absolute sample value.
/// Use `20 * log10(peak)` to calculate the non-destructive gain for normalization.
pub fn peak_of(file: &AudioFile) -> Result<f32, EngineError> {
    file.peak().map_err(EngineError::from)
}

/// UI-thread handle. Not `Send` on some platforms (cpal `Stream`); keep it on the UI thread.
pub struct Engine {
    commands: CommandQueue,
    shared: Arc<Shared>,
    sample_rate: u32,
    device_name: String,
    _stream: Stream,
    disk: DiskThread,
}

impl Engine {
    /// Starts on the default output device with the device's default buffer size.
    pub fn start() -> Result<Self, EngineError> {
        Self::start_with(None)
    }

    /// Like [`Engine::start`], but asks for `buffer_frames` per callback (clamped to what the
    /// device supports). Smaller buffers lower latency (Logic: I/O Buffer Size).
    pub fn start_with(buffer_frames: Option<u32>) -> Result<Self, EngineError> {
        let mut last = EngineError::NoOutputDevice;
        // The platform's default audio system first, then any other one that is available
        // (e.g. ALSA when a sound daemon is broken).
        let default_id = cpal::default_host().id();
        let mut ids = vec![default_id];
        ids.extend(
            cpal::available_hosts()
                .into_iter()
                .filter(|id| *id != default_id),
        );
        for id in ids {
            let Ok(host) = cpal::host_from_id(id) else {
                continue;
            };
            match Self::start_on_host(&host, buffer_frames) {
                Ok(engine) => return Ok(engine),
                Err(e) => {
                    tracing::warn!("audio system {id} failed: {e}");
                    last = e;
                }
            }
        }
        Err(last)
    }

    /// Starts on the ASIO host (Windows builds with the `asio` feature only).
    /// Use this to avoid shared-mode WASAPI buffering when a native ASIO driver is available.
    #[cfg(all(windows, feature = "asio"))]
    pub fn start_asio_with(buffer_frames: Option<u32>) -> Result<Self, EngineError> {
        let host = cpal::host_from_id(cpal::HostId::Asio)
            .map_err(|error| EngineError::Backend(error.to_string()))?;
        Self::start_on_host(&host, buffer_frames)
    }

    fn start_on_host(host: &cpal::Host, buffer_frames: Option<u32>) -> Result<Self, EngineError> {
        let mut last = EngineError::NoOutputDevice;
        if let Some(device) = host.default_output_device() {
            match Self::start_on(&device, buffer_frames) {
                Ok(engine) => return Ok(engine),
                Err(e) => {
                    tracing::warn!("default audio output failed: {e}");
                    last = e;
                }
            }
        }
        // The default device can be missing or busy (common with raw ALSA on Linux): try
        // every other output device before giving up.
        if let Ok(devices) = host.output_devices() {
            for device in devices {
                let name = device.id().map(|i| i.to_string()).unwrap_or_default();
                match Self::start_on(&device, buffer_frames) {
                    Ok(engine) => {
                        tracing::info!("using audio output {name}");
                        return Ok(engine);
                    }
                    Err(e) => {
                        tracing::debug!("audio output {name} failed: {e}");
                        last = e;
                    }
                }
            }
        }
        Err(last)
    }

    /// Starts on `device`. If the requested buffer size is refused, retries with its default.
    fn start_on(device: &cpal::Device, buffer_frames: Option<u32>) -> Result<Self, EngineError> {
        match Self::start_device(device, buffer_frames) {
            Err(e) if buffer_frames.is_some() => {
                tracing::warn!("buffer size {buffer_frames:?} refused ({e}); using the default");
                Self::start_device(device, None)
            }
            other => other,
        }
    }

    fn start_device(
        device: &cpal::Device,
        buffer_frames: Option<u32>,
    ) -> Result<Self, EngineError> {
        let device_name = device
            .id()
            .map(|i| i.to_string())
            .unwrap_or_else(|_| "default".into());
        let supported = device
            .default_output_config()
            .map_err(|e| EngineError::Backend(e.to_string()))?;
        let format = supported.sample_format();
        let range = match supported.buffer_size() {
            SupportedBufferSize::Range { min, max } => Some((*min, *max)),
            SupportedBufferSize::Unknown => None,
        };
        let mut config: StreamConfig = supported.into();
        if let Some(n) = buffer_frames {
            let n = range.map_or(n, |(lo, hi)| n.clamp(lo, hi));
            config.buffer_size = BufferSize::Fixed(n);
        }
        let sample_rate = config.sample_rate;

        let (tx, rx) = RingBuffer::<Command>::new(1024);
        let (gtx, grx) = RingBuffer::<Garbage>::new(GARBAGE_CAPACITY);
        let shared = Arc::new(Shared::new());
        let core = EngineCore::new(sample_rate, rx, gtx, shared.clone());

        let stream = match format {
            SampleFormat::F32 => build::<f32>(device, config, core, shared.clone()),
            SampleFormat::I16 => build::<i16>(device, config, core, shared.clone()),
            SampleFormat::U16 => build::<u16>(device, config, core, shared.clone()),
            SampleFormat::I32 => build::<i32>(device, config, core, shared.clone()),
            SampleFormat::U32 => build::<u32>(device, config, core, shared.clone()),
            SampleFormat::I8 => build::<i8>(device, config, core, shared.clone()),
            SampleFormat::U8 => build::<u8>(device, config, core, shared.clone()),
            SampleFormat::F64 => build::<f64>(device, config, core, shared.clone()),
            other => return Err(EngineError::UnsupportedSampleFormat(other.to_string())),
        }?;
        stream
            .play()
            .map_err(|e| EngineError::Backend(e.to_string()))?;
        Ok(Self {
            commands: CommandQueue {
                tx,
                garbage: grx,
                credits: GARBAGE_CAPACITY,
            },
            shared,
            sample_rate,
            device_name,
            _stream: stream,
            disk: DiskThread::spawn(),
        })
    }

    /// Queue a command (Logic: transport or project edit). Returns `false` if command or
    /// state-reclamation capacity is exhausted. The rejected command is freed here on the
    /// UI thread; callers must retry their desired change before marking it synchronized.
    pub fn send(&mut self, cmd: Command) -> bool {
        let ok = self.commands.send(cmd);
        if !ok {
            tracing::error!("engine command or reclamation queue full; command dropped");
        }
        ok
    }

    pub fn shared(&self) -> &Shared {
        &self.shared
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Streams `file` from disk as an audio region on `track` starting at `start_beat`.
    /// Put the result in [`crate::Song::with_clips`] and send it with `Command::SetSong`.
    pub fn stream_clip(&self, file: &AudioFile, track: u8, start_beat: f64) -> AudioClip {
        self.stream_clip_with(file, track, start_beat, ClipEdit::default())
    }

    /// [`Engine::stream_clip_with`] for a song with `clips_in_song` streamed regions in all:
    /// every clip buffers ahead as much as the song's 8 MiB prefetch budget allows
    /// (between 170 ms and 0.68 s), so a few regions stay safe against a held-up disk and
    /// hundreds of regions stay light. Pass the same count for every clip of the song.
    pub fn stream_clip_in(
        &self,
        file: &AudioFile,
        track: u8,
        start_beat: f64,
        edit: ClipEdit,
        clips_in_song: usize,
    ) -> AudioClip {
        self.disk.stream_with_ring(
            file,
            self.sample_rate,
            track,
            start_beat,
            edit,
            crate::disk::ring_blocks_for(clips_in_song),
        )
    }

    /// Streams an audio region with non-destructive gain, beat-based fades, and reverse.
    /// It buffers the most ahead; for a song with many regions use [`Engine::stream_clip_in`].
    pub fn stream_clip_with(
        &self,
        file: &AudioFile,
        track: u8,
        start_beat: f64,
        edit: ClipEdit,
    ) -> AudioClip {
        self.disk
            .stream_with(file, self.sample_rate, track, start_beat, edit)
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut core: EngineCore,
    shared: Arc<Shared>,
) -> Result<Stream, EngineError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = (config.channels as usize).max(1);
    let sample_rate = config.sample_rate;
    // Pre-allocated once, before the stream starts; the callback never allocates.
    let mut scratch = vec![0f32; MAX_BLOCK_FRAMES * 2];
    let shared_cb = shared.clone();
    let err_fn = move |err: Error| match err.kind() {
        ErrorKind::Xrun => {
            shared.xruns.fetch_add(1, Ordering::Relaxed);
        }
        _ => tracing::error!("audio stream error: {err}"),
    };
    device
        .build_output_stream(
            config,
            move |data: &mut [T], info: &OutputCallbackInfo| {
                // `Instant::now` reads the OS monotonic clock (no lock, no allocation).
                let started = Instant::now();
                let ts = info.timestamp();
                let lat = ts
                    .playback
                    .duration_since(ts.callback)
                    .as_micros()
                    .min(u32::MAX as u128) as u32;
                shared_cb.output_latency_us.store(lat, Ordering::Relaxed);
                shared_cb
                    .block_frames
                    .store((data.len() / channels) as u32, Ordering::Relaxed);
                for chunk in data.chunks_mut(MAX_BLOCK_FRAMES * channels) {
                    let frames = chunk.len() / channels;
                    let buf = &mut scratch[..frames * 2];
                    assert_no_alloc::assert_no_alloc(|| core.process(buf));
                    for (i, frame) in chunk.chunks_exact_mut(channels).enumerate() {
                        let (l, r) = (buf[i * 2], buf[i * 2 + 1]);
                        for (c, s) in frame.iter_mut().enumerate() {
                            *s = T::from_sample(match c {
                                0 => l,
                                1 => r,
                                _ => 0.0,
                            });
                        }
                    }
                }
                let load = dsp_load(started.elapsed(), data.len() / channels, sample_rate);
                shared_cb.record_dsp_load(load);
            },
            err_fn,
            None,
        )
        .map_err(|e| EngineError::Backend(e.to_string()))
}
