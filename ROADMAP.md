# ProWave Studio — Complete Logic Pro Clone Roadmap
**Codename: `prowave-studio` | Stack: Pure Rust + WebAssembly | Targets: macOS, Linux, Windows, Web**
> ProWave Studio is an independent, clean-room DAW inspired by Apple Logic Pro. Logic Pro is a trademark of Apple Inc. No Apple code, assets, samples, or UI assets are reused. This document is a functional specification for re-implementation.

## 0. Goal

Build a 1:1 functional clone of Logic Pro 11 (as of 2024-2025) in pure Rust + WASM. Every menu, editor, instrument, effect, and workflow must be accounted for so an AI coding agent (Claude) can implement without guessing.

This roadmap is the single source of truth. If it's not in here, it doesn't get built.

---

## 1. Complete Logic Pro Feature Inventory — What We Must Clone

This is exhaustive. Based on Logic Pro 10.7 / 11.

### 1.1 Transport, Timeline & Project

- **Transport:** Play, Stop, Record, Play from Selection, Play from Last Locate, Pause, Cycle/Loop, Count-in, Metronome (Klopfgeist with custom sounds), Varispeed (Speed Only, Varispeed + Pitch)
- **Timeline:** Bars|Beats, Time (SMPTE, HMS, Samples), Tempo Track, Time Signature Track, Beat Mapping Track, Arrangement Track (Intro, Verse, Chorus markers with colors), Marker Track
- **Smart Tempo:** Adapt Project Tempo to imported audio, Adapt Region Tempo, Keep Project Tempo, Automatic, Aligned. Smart Tempo Editor (edit downbeats, beats). Multitrack Smart Tempo analysis
- **Project Management:** Alternatives (like git branches for arrangement), Backups, Project Clean Up, Consolidate, 1000 Audio Tracks, 1000 Software Instrument Tracks, 1000 Aux, 1000 External MIDI, Sample Rate 44.1-192kHz, 64-bit summing
- **Tuning:** Equal Tempered, Hermode Tuning (HMT), User Tuning (Scala .tun), A=440-446 variable, per-project

### 1.2 Track Types & Track Management

- **Audio Track:** Mono, Stereo, Input Monitoring, Record Enable, Input/Output assignment, Freeze, Track Stacks (Folder Stack = grouping, Summing Stack = submix bus with aux), Track Alternatives (playlists), Groove Track (master timing reference), Track Icon, Track Color
- **Software Instrument Track:** MIDI Channel, Layered, Multi-timbral (16 channels per instance)
- **Drummer Track:** Virtual drummer with 28+ drummers (Rock, Hip-Hop, EDM, etc.), X/Y pad Complexity/Loudness, Fill knob, Kit piece mutes, Follow Track (groove), Drum Kit Designer integration, Convert to MIDI region
- **External MIDI / Instrument Track:** Program Change, Bank, External synth routing
- **Aux / Channel Strip / Bus / VCA / Master:** Aux as destination for sends/busses, VCA faders
- **Folder Track:** Pack folders
- **Track Inspector:** Region inspector (Quantize, Velocity, Transpose, Gain, Flex) + Track Inspector (Instrument, MIDI FX, Audio FX chain, Sends, EQ thumbnail)
- **Library & Patches:** Patch = Channel Strip + MIDI FX + Instrument + FX. Full Library with categories (Synth, Piano, Guitar...). User patches

### 1.3 Audio Recording, Editing & Comping

- **Recording:** Punch-in/out, Autopunch, Replace mode, Take Folders (pack takes into folder), Quick Swipe Comping (swipe to select best parts of takes), Comp to new track, Crossfade Tool (equal power, equal gain, custom curve, S-curve), Fade Tool
- **Audio Regions:** Gain tool (clip gain +6/-24dB), Flex Time on region, Gain envelope, Reverse, Trim, Split at Playhead / Silence / Transient
- **Audio Editor:** Audio File Editor (destructive view of file), Audio Track Editor (non-destructive, Flex Time/Pitch), Transient Detection (Sensitivity slider), Strip Silence (threshold, min length)
- **Marquee Tool:** Marquee selection, Marquee + Selection-based processing (Gain, Normalize, Reverse, Silence)
- **File Management:** Project Audio Browser (all audio files), Audio File metadata, Convert sample rate, Copy/Convert to project, Optimize files

### 1.4 Flex Time & Flex Pitch (Critical)

- **Flex Time Modes:** Slicing (drums), Rhythmic, Monophonic (vocals/bass), Polyphonic (chords), Tempophone (creative), Speed (time stretch + pitch shift linked)
- **Flex Time Editing:** Flex markers, Quantize audio (like MIDI), Groove extraction, Move transients, Time stretch region by dragging
- **Flex Pitch:** Per-note pitch editing inside Audio Track Editor, like piano roll for audio. Pitch Drift, Fine Pitch, Gain per note, Vibrato, Formant shift. Scale quantize (snap to scale), Reference Tuning. Create MIDI from Flex Pitch

### 1.5 MIDI Sequencing & Editors

- **Piano Roll:** Quantize (with Swing, Strength, Q-Swing), Velocity tool, Brush tool (paint notes), Time Handles (stretch MIDI), Note Repeat, Humanize, Select Equal Positions/Color/Pitch, MIDI Draw (CC automation per region), Scale Quantize, Flam, Velocity Limiter, Note Color = Velocity/Pitch/Voice
- **Score Editor:** Full notation, Staff Styles (Piano, Guitar Tab, Drum), Score Sets, Brackets, Lyrics, Chord Symbols, Time/Key Signatures per staff, MIDI Meaning -> Notation, Print
- **Step Sequencer:** Pattern Regions, Step Sequencer Grid (like 808), per-step: Note, Velocity, Gate, Tie, Chance, Repeat, Loop direction (Forward, Backward, Ping-Pong, Random), Automation per row, Convert to MIDI region
- **Event List:** Raw MIDI event editor, SysEx
- **Step Editor (Hyper Editor):** Drum lane editor for CCs
- **MIDI Transform:** Fixed transforms (Crescendo, Humanize, Quantize, Transpose, Double Speed, etc.) + Custom Transform with conditions (Select and Operate)
- **MIDI FX Chain (pre-instrument, 4 slots):** Arpeggiator, Chord Trigger, Modulator (LFO/Env to MIDI CC), Note Repeater, Randomizer, Transposer, Scripter (JavaScript MIDI processing - we will use Rhai/WASM scripting in Rust), Velocity Processor
- **MIDI 2.0 & MPE:** MPE editing (per-note Pitch Bend, Pressure, Timbre), MIDI 2.0 High Resolution CC, Per-Note controllers

### 1.6 Instruments — Logic Pro has 40+ — We must clone all categories

**Sampler Family:**
- **Sampler (EXS24 successor):** Multisampled, Zones, Groups, Velocity layers, Round Robin, Filter, LFOs, ADSR, Mod Matrix (10 slots), Import EXS, SFZ, DLS. Visual mapping editor
- **Quick Sampler:** Drag-drop audio -> auto-slice (Transient, Beat, Equal), One-Shot, Classic, Slice mode with 8 slices, Record directly, Trim, Loop, Flex Time integration
- **Drum Machine Designer:** 16/24/48 pads, each pad = Quick Sampler + channel strip with inserts/sends. Pad mute/solo, Q-Sampler controls per pad
- **Drum Kit Designer:** Acoustic kits (20 kits), Producer Kits (multi-mic with bleed control, overhead/room mics), Drum Replacement (replace kit piece)

**Synths:**
- **Alchemy (Flagship):** 4 sources (VA, Additive, Spectral, Granular, Sampler), 3000+ presets, Morph pad X/Y, 16 LFOs, 16 Env, MSEG, Arp, Effects, Granular engine. For v1.0 we will build simplified version: Alchemy-Lite (2 sources, VA + Sampler)
- **ES2:** Vector synth, 3 oscillators, 1 filter, Vector Envelope, Planer Pad, Distortion, Modulation Router (10 slots) - Complex but must be cloned exactly
- **ES1, EFM1 (FM), ES P, ES E, ES M (Monophonic)** — simpler analog/FM synths
- **Vintage Series:** Vintage B3 Organ (drawbars, percussion, Leslie emulation), Vintage Clav (D6), Vintage Electric Piano (Rhodes/Wurli), Vintage Mellotron (Tape playback)
- **Sculpture:** Physical modeling (string, nylon, steel, glass), Object + Material model, Morphing
- **Retro Synth:** 4 engines in one: Analog, Sync, Wavetable, FM
- **Ultrabeat:** 25 voices drum synth (VA, FM, Sample, Model), Step Sequencer built-in
- **Vocal Synth (Vocal Transformer)**
- **Studio Strings, Studio Horns:** Orchestral with Articulation ID (key switching)
- **New Logic 11 Session Players:** Bass Player, Keyboard Player (like Drummer but for bass/keys)

**Implementation Plan for Instruments:** Build trait `Instrument: DspNode` with `nih-plug` compatible. All instruments share common `ModMatrix`, `Lfo`, `Adsr`.

### 1.7 Effects — Logic has 80+ effects — Clone by Category

**Must clone all, with identical controls for preset compatibility:**

**Dynamics:**
- Compressor: Models Platinum Digital, Studio VCA, Studio FET, Vintage VCA, Vintage FET, Vintage Opto. Threshold, Ratio, Knee, Attack, Release, Auto Gain, Sidechain filter, Mix
- Limiter, Adaptive Limiter, Multipressor (4-band compressor), Enveloper (transient shaper), Noise Gate, Expander, DeEsser 2, Surround Compressor

**EQ:**
- Channel EQ: 8-band parametric, Analyzer (FFT), Analyzer Pre/Post, Q-Couple, Gain-Q-Couple, 20Hz-20kHz, 4x oversampling
- Linear Phase EQ (same as Channel EQ but linear phase)
- Vintage EQs: Vintage Graphic EQ (API 560 style), Vintage Tube EQ (Pultec), Vintage Console EQ (Neve)
- Single Band EQs, Match EQ (EQ matching)

**Filter:**
- AutoFilter: LFO, Env Follower, Distortion

**Reverb & Space:**
- Space Designer: Convolution reverb, IRs (1000+ IRs), Length, Filter, Envelope, Direct/Rev
- ChromaVerb: Algorithmic reverb, 14 models (Hall, Room, Plate...), 3-band Decay EQ, Attack/Smooth/Spin
- SilverVerb, PlatinumVerb, EnVerb, Early Reflections

**Delay:**
- Delay Designer: Tap Delay Designer (visual tap programming, 26 taps, pitch/filter per tap)
- Tape Delay: Saturation, Flutter, LFO
- Echo, Stereo Delay, Sample Delay (sample-accurate delay for phase correction)

**Modulation:**
- Chorus, Ensemble, Flanger, Phaser, Tremolo, Auto Pan, Spreader, Microphaser, Ringshifter (frequency shifter), Scanner Vibrato (Leslie vibrato)

**Distortion & Overdrive:**
- Overdrive, Distortion, Bitcrusher, Clip Distortion, Phase Distortion, Distortion II

**Imaging & Panning:**
- Direction Mixer, Stereo Spread, Binaural Post-Processing, Gain (utility), MultiMeter, Loudness Meter, Correlation Meter, Tuner, Test Oscillator

**Pitch:**
- Pitch Correction (auto-tune like), Pitch Shifter

**Special / Utility:**
- Exciter, SubBass (bass synth), Voice Transformer (formant), Limiter, Spectral Gate, EnVerb

**Mastering:**
- Mastering Assistant (AI in Logic 11): Analyze track, suggest EQ, Dynamics, Stereo Spread, Loudness. We will clone as rule-based + ML (ONNX in Rust via `ort`)
- Stem Splitter (Logic 11): Split mix into Drums, Bass, Vocals, Other using ML (Demucs). We will use `tract` + Demucs model

**MIDI FX (already listed) + Audio FX from Logic's Pedalboard:** 35 guitar pedals (Delay, Distortion, Modulation pedals). Amp Designer: Amp + Cab + Mic modeling (100+ amps/cabs)

**Implementation:** All FX implement `DspNode` with latency reporting for PDC. Use `fundsp` + custom. Each FX must have preset serialization JSON compatible with Logic's .pst where possible.

### 1.8 Mixing, Automation & Metering

- **Mixer:** Tracks, Aux, Bus, Output, Master. Fader (0 to +6dB), Pan/Balance/Stereo Pan (with Stereo Pan for stereo tracks showing left/right position), Input, Sends (pre/post, 8 per channel), Inserts (15 per channel), EQ thumbnail (click to open Channel EQ), Sends on Fader, VCA assignment, Group (Mix groups with attributes: Volume, Pan, Mute, Solo, etc.), All Tracks vs Single Track view, Channel Strip Settings (Save/Load)
- **Automation:** Track Automation vs Region Automation (MIDI Draw). Modes: Read, Touch, Latch, Write. Trim. Automation curves: Linear, Exponential, Bezier with 2 handles. Automation Select Tool, Automation Snap, Relative/Absolute, Automation for every plugin parameter (sample-accurate), Automation Event List
- **Metering:** Pre/Post fader metering, Peak, RMS, Loudness (LUFS), True Peak, Clip Indicator, Peak Hold, Level Meter with scale (0dB = -12dBFS option)
- **Latency Compensation:** Plugin Delay Compensation (PDC) full, Low Latency Mode (bypass high-latency plugins when recording), Plug-in Manager showing latency

### 1.9 Browsing, Loops & Sound Library

- **Loop Browser:** Apple Loops (.aiff/.caf with metadata: key, tempo, style, instrument, category), Audio Loops (blue), MIDI Loops (green), Pattern Loops (yellow), Drag-Drop with auto-adaptation to project tempo/key
- **Sound Library:** Downloadable content (70GB), Relocate Sound Library, Logic Content (samples, IRs, Alchemy samples)
- **All Files Browser:** Finder-like browser, Audio files, Movies

### 1.10 Live Loops & Performance

- **Live Loops Grid:** Cells (loops), Scenes (columns), Trigger modes (Start/Stop, One Shot), Quantize Start (1 bar, 2 beats, etc.), Play From Here, Live Loops for Drummer, Step Sequencer patterns inside Live Loops
- **Performance Recording:** Record Live Loops performance into Tracks area (arrangement)
- **Remix FX:** DJ-style effects (Gate, Bitcrusher, Filter, Gater, Repeater, Reverse) with X/Y pad, applied to master or bus

### 1.11 Score, Video & Sync

- **Score Editor:** As above, plus Drum Notation, Tablature, Chord Track -> Score, Export MusicXML, Print
- **Video Track:** Movie Track, Show Movie (floating window), Filmstrip thumbnails, SMPTE Sync, Detect Cuts, Export Audio to Movie, Synchronization Settings (MTC, MMC)
- **Tempo & Time Signature List Editor:** Edit tempo events

### 1.12 Sharing, Bounce & Export

- **Bounce:** Bounce Project/Region/Track in Place, Offline/Real-time, Include/Exclude Automation, Normalize On/Off/Overload Protection, MP3/AAC/WAV/AIFF, 16/24/32-bit, Dither (Apogee UV22HR)
- **Export:** Export All Tracks as Audio Files, Export MIDI, Export Score, Export as AAF (for Pro Tools), Export as MusicXML
- **Share to:** GarageBand, Voice Memos, SoundCloud (deprecated, but we will do generic upload), Dolby Atmos ADM BWF

### 1.13 Control Surfaces & Hardware

- **Smart Controls:** Auto-mapped knobs for each instrument/effect (8 knobs), Mapping editor (Map knob to multiple parameters with range), Smart Controls for whole Patch
- **Controller Assignments:** Learn mode (MIDI Learn), Assignment for Fader, Pan, Plugin Params, Key Commands
- **Control Surface Support:** Mackie Control, Logic Remote (iPad app) - we will build Web Remote instead via WASM/WebSocket
- **Key Commands:** 1200+ key commands, Customizable, Presets (US, etc.), Search, Conflict detection

---

## 2. Technical Architecture for ProWave (Pure Rust + WASM)

```
[ UI Thread: egui ] --(rtrb SPSC: Command)--> [ Engine Thread: prowave-engine ]
         ^                                          |
         |----(rtrb SPSC: Metering/Events)----------|
                                                    v
                                           [ Audio Thread: 1-8ms deadline, no alloc ]
                                                    |
                                    cpal (native) / AudioWorklet (web)
```

**Workspace Structure:**
```
/crates
  /prowave-engine    # lock-free audio graph, transport, PDC
  /prowave-dsp       # all FX: Eq, Comp, Reverb, etc. trait DspNode
  /prowave-instruments # all instruments: Sampler, Quick Sampler, ES2, Sculpture, Vintage...
  /prowave-mixer     # summing, busses, VCA, groups
  /prowave-midi      # piano roll logic, score data, step seq, MPE, MIDI 2.0, MIDI FX
  /prowave-plugin-host # CLAP primary, VST3, AU, sandboxed process
  /prowave-project   # .prowave file (zip: project.json + media/ + irs/ + sampler instruments)
  /prowave-ui        # egui widgets: Tracks, Mixer, Piano Roll, Score, Loop Browser, Library
  /prowave-wasm      # wasm-bindgen glue + AudioWorklet entry + OPFS + Web Remote
/xtask
```

**Critical Rust Rules (for Claude):**
- Audio thread: `#![forbid(alloc)]` in process() path, use `assert_no_alloc`. No mutex, no `println!`, no `Vec::new()`
- All params: `AtomicF32` + 1-pole smoother (`ParamSmoother { target, current, coeff }`)
- Graph: Petgraph-like `AudioGraph { nodes: Vec<Node>, edges: Vec<Edge> }` sorted topologically each time graph changes (on engine thread, not audio thread)
- PDC: Each node reports `latency_samples()`. Engine computes max latency, adds delay compensation to other paths using `rubato` + delay lines
- State: Triple-buffer for UI->Engine. `Command` enum is only way UI mutates engine

---

## 3. Phased Roadmap — Implementation Order for Claude

### Phase 0: Foundation (Weeks 1-4)
- [ ] `cargo xtask` setup, CI for mac/win/linux + wasm
  - Done: `cargo xtask check|wasm-check|rt-check` (fmt, clippy -D warnings, tests, no-alloc, cargo-deny, wasm32) passes on Windows.
  - Linux: x86_64 release binary cross-builds from Windows (Zig + `cargo-zigbuild`, glibc 2.31+) and is packaged in `dist/prowave-linux-x86_64.tar.gz` with a full user README; audio thread asks for real-time priority (cpal `realtime`); window class set for desktop integration. Audio start falls back across devices/sample formats; the unsaved-changes prompt is in-app (no zenity); clippy `-D warnings` + test compilation pass for the Linux target; real DT_NEEDED is only libasound + libc. Compiled but never run on a Linux machine.
  - macOS: cannot be built on Windows (needs Apple's SDK). `.github/workflows/release.yml` builds a universal ad-hoc-signed `.app` and the Linux tarball on GitHub runners.
  - Pending: `.github/workflows/ci.yml` and `release.yml` written but never run (repo has no git remote yet); macOS unverified, Linux untested at runtime; native PipeWire/JACK back-ends, AppImage/.deb/Flatpak packaging.
- [ ] Lock-free primitives: `rtrb` SPSC queue, triple-buffer, `basedrop` for real-time safe drop
  - Done: `rtrb` command queue + garbage queue (old songs freed off the audio thread); `assert_no_alloc` guard on `process()` in debug builds and tests.
  - Pending: triple-buffer, `basedrop`.
- [x] Logging `tracing`, crash handler `human-panic`, `metrics` for DSP load
  - Done: `tracing` in the engine; `tracing-subscriber` in the app (`RUST_LOG`, default `warn,prowave=info`); `human-panic` crash reports (release builds).
  - DSP load: each audio callback is timed against its real-time budget and the peak is published through a lock-free atomic (`Shared::take_dsp_load`); the LCD CPU meter shows it (red at >=90%, % on hover). Atomics are used instead of the `metrics` crate (no exporter needed yet).
- [ ] Basic `cpal` playback: sine wave, measure RTL. `prowave-engine` example `beep`
  - Done: `beep` plays 440 Hz through the engine and reports device output latency (output side only; true RTL needs a loopback).
- **DoD:** `cargo run -p prowave-engine --example beep` plays 440Hz <20ms on all OSes
  - Status: NOT met on Windows/WASAPI shared mode: 480-frame callbacks, ~32 ms reported output latency (cpal has no WASAPI exclusive mode). Needs the cpal `asio` feature. macOS/Linux not measured yet (Linux/ALSA should reach 128-frame buffers with real-time priority).

### Phase 1: Audio Engine Core + Track System (Weeks 5-10)
- [ ] Transport: sample-accurate clock, tempo map (tempo + time signature automation), SMPTE sync
  - Done (engine): `prowave-engine/src/tempo_map.rs` — tempo events (5-990 BPM, quarter-note tempo like Logic) and signature events, beat<->seconds, bars/beats; the transport advances through tempo changes per sample, audio clips are positioned by real time, the metronome follows the signature (accent on bar 1). Sent with `Command::SetTempoMap` (old map freed off the audio thread).
  - Pending: tempo curves (only step changes now), Tempo/Signature tracks in the UI + project file (the app still sends one tempo + signature), SMPTE sync; the app's own bar/beat and SMPTE display should use `TempoMap` (it ignores the signature denominator).
- [ ] Track types: Audio, Instrument, Aux, Master, VCA, Folder/Summing Stacks, External MIDI
- [ ] Region system: Audio regions (backed by `symphonia` decoder wav/aiff/caf/mp3/flac), MIDI regions, Pattern regions, Drummer regions
- [ ] Non-destructive editing: clip gain, fade in/out with curves (equal-power, linear, S-curve), crossfade, Reverse, Normalize (non-destructive)
- [ ] Disk streaming: memory-mapped + prefetch thread, 1000 tracks streaming test
- [ ] Project file `.prowave`: zip with `project.json` (rkyv), `media/`, `bounces/`
- **DoD:** Import 24 tracks 48k/24-bit, play without dropouts

### Phase 2: MIDI & Sequencer (Weeks 11-18)
- [ ] MIDI 1.0/2.0 parser, MPE, Articulation ID, SysEx
- [ ] Piano Roll: full Logic parity (Quantize, Velocity, Brush, Time Handles, MIDI Draw for CC, Scale Quantize, Humanize, Flam, Note Color)
- [ ] Score Editor v0: render notes with `staff` crate, basic printing, MusicXML export
- [ ] Step Sequencer: Pattern region with rows, per-step Chance, Repeat, Tie, Gate, Loop Direction
- [ ] Event List, Step Editor, MIDI Transform (conditions + operations), MIDI FX chain (Arp, Chord Trigger, Scripter via Rhai)
- [ ] Drummer Track v0: 2 drummers, Complexity/Loudness X/Y, Follow Track, Convert to MIDI
- **DoD:** Create Drummer + Bass (MIDI) + Keys, quantize, bounce

### Phase 3: Flex & Smart Tempo (Weeks 19-24) — Logic's secret sauce
- [ ] Smart Tempo Editor: detect tempo from audio, edit downbeats, multitrack analysis
- [ ] Flex Time: Implement 5 modes - Slicing (use `aubio` onset detection), Rhythmic, Monophonic (use `rubberband` or `signalsmith`), Polyphonic (Elastique-like via `rubberband`), Tempophone, Speed
- [ ] Flex Pitch: Pitch detection (YIN via `aubio` or `pitch-detection` crate), per-note editing, Gain, Drift, Vibrato, Formant
- [ ] Groove Track: master track that other tracks follow (extract groove, apply)
- [ ] Audio Track Editor integration
- **DoD:** Import vocal, Flex Pitch correct to scale, Flex Time quantize to groove

### Phase 4: Mixer & Channel Strip (Weeks 25-30)
- [ ] Mixer UI: faders, pan (stereo pan with left/right puck), inserts (15 slots), sends (8 slots, pre/post), VCA, Groups, Sends on Fader
- [ ] DSP: Channel EQ (8-band with FFT analyzer - use `rustfft` + `egui` plot), Compressor (6 models: Platinum, VCA, FET, Vintage VCA/FET/Opto with sidechain), Gate, Limiter, Overdrive
- [ ] Automation: Track vs Region, Modes Read/Touch/Latch/Write/Trim, Bezier curves, Automation Quick Access, Sample-accurate parameter automation
- [ ] Bus summing: 64-bit double, PDC full, Low Latency Mode
- **DoD:** Mix 24 tracks with EQ/Comp on each, automation, PDC verified null test

### Phase 5: Instruments - Part 1 (Weeks 31-40)
- [ ] Sampler (EXS24 clone): Zone/Group editor UI, Velocity layers, Round Robin, Mod Matrix 10 slots, Filter (LP/HP/BP), LFOs, Import SFZ/EXS
- [ ] Quick Sampler: drag-drop, Slice/One-Shot/Classic, auto-slice by transient/beat, Record, Trim
- [ ] Drum Machine Designer: 48 pads, each pad = Quick Sampler + strip
- [ ] Drum Kit Designer: acoustic kits with multi-mic (Close, Overhead, Room, Bleed controls)
- [ ] ES2: Vector synth clone - 3 osc (Saw, Pulse, Sine with FM), Filter, Distortion, Vector Envelope, Router 10 slots. Must match Logic presets byte-for-byte parameter wise
- [ ] Vintage Series: B3 Organ (drawbars 9 + percussion + Leslie with Horn/Rotor speed, distortion), Vintage Clav (D6 pickup), Vintage E-Piano
- [ ] Retro Synth: 4 engines Analog/Sync/Wavetable/FM in one UI
- **DoD:** Load Sampler instrument, play with MIDI, patch saving

### Phase 6: Instruments - Part 2 & FX Suite (Weeks 41-50)
- [ ] Sculpture: physical modeling (string model with Object, Material, Pick, etc.)
- [ ] Alchemy-Lite: 2 sources (VA + Sampler), Morph pad, MSEG, Arp, Granular basic (for v1.0)
- [ ] Ultrabeat: 25 voices drum synth, built-in step sequencer
- [ ] Studio Strings/Horns: Articulation ID system (key switch, channel, velocity)
- [ ] FX full suite: Space Designer (convolution reverb with IR loading, envelope, filter, length), ChromaVerb (14 models), Delay Designer (visual tap editor, 26 taps, pitch/filter per tap), Tape Delay, Echo, Chorus, Ensemble, Flanger, Phaser, Tremolo, AutoFilter, Ringshifter, Distortion II, Bitcrusher, Overdrive, Pedalboard (35 pedals), Amp Designer (amps + cabs + mics), Match EQ, Multipressor, Exciter, SubBass, Voice Transformer, etc.
- [ ] Mastering Assistant: rule-based chain (EQ -> Comp -> Limiter) + LUFS targeting -14 LUFS, analyze using `ebur128` crate
- **DoD:** A/B test Channel EQ curve vs Logic (within 0.5dB), Space Designer IR load

### Phase 7: Plugin Hosting & Sandboxing (Weeks 51-56)
- [ ] CLAP host full (note, params, latency, state), VST3 host via `vst3-sys`, AU host on macOS via `coreaudio-sys`
- [ ] Out-of-process sandbox: separate process, SHM ring buffer (1ms latency overhead max), seccomp (Linux), seatbelt (macOS), AppContainer (Win). Heartbeat, crash isolation, auto-restart
- [ ] Plugin scanner with quarantine, timeout 5s per plugin, blacklist
- [ ] Plug-in Manager UI showing latency, type, manufacturer
- **DoD:** Load Vital, Serum, Kontakt without crashing main DAW if plugin crashes

### Phase 8: UI - The Logic Look + Live Loops (Weeks 57-66)
- [ ] Main window: Tracks area (with Track Headers, Track Inspector left, Library right), Mixer (separate window or bottom), Editors (Piano Roll bottom, Score, Audio Editor)
- [ ] Library: Patches (Channel Strip + Instrument + FX), save/load, search
- [ ] Smart Controls: 8 knobs per patch, mapping editor (map knob to multiple params with range scaling), Learn
- [ ] Key Commands: 1200+ commands, searchable, customizable, Presets, Conflict detection
- [ ] Live Loops: Grid (Cells, Scenes, Trigger modes, Quantize Start), Record Live Loops performance to Tracks, Remix FX (X/Y pad with Gate, Filter, Repeater, Reverse)
- [ ] Loop Browser: Apple Loops metadata parsing (key, tempo, style), drag-drop with auto tempo/key adaptation, All Files Browser
- [ ] Video Track: Movie Track with filmstrip, SMPTE, Detect Cuts, Export Audio to Movie
- **DoD:** 5 Logic users can do basic tracking/mixing without manual

### Phase 9: WASM, Collaboration & Polish (Weeks 67-74)
- [ ] Compile `prowave-engine` to WASM + AudioWorklet (128-sample blocks, SharedArrayBuffer + Atomics)
- [ ] `prowave-wasm`: OPFS for project storage, Web UI via `egui_web` (same egui code), Web Remote (like Logic Remote but via browser/WebSocket)
- [ ] Collaboration: Project file with Ed25519 signing, simple CRDT for arrangement (optional)
- [ ] Export: Bounce Project/Region/Track in Place (offline/real-time, normalize, dither Apogee UV22HR via `r8brain`), Export All Tracks as Audio, Export MIDI, MusicXML, AAF, ADM BWF (Atmos)
- [ ] Performance: SIMD (`wide`, `std::simd`), benchmarks vs Logic/Reaper
- **DoD:** Open in Chrome/Firefox, play 16-track project <30ms latency, bounce to WAV

---

## 4. Future Options / v2.0+ (Not in v1.0 but architect for it)

- **Dolby Atmos / Spatial Audio:** 7.1.4 beds, 128 objects, ADM BWF export, Binaural monitoring, 3D panner UI (like Logic's Atmos panner)
- **AI Session Players v2:** Bass Player, Keyboard Player, Drummer with style transfer (train small ONNX models in Rust via `burn` or `ort`)
- **Stem Splitter:** Demucs model via `tract` (separate Drums/Bass/Vocals/Other) — Logic 11 feature
- **AI Mastering Assistant v2:** Full AI chain with reference track matching
- **Cloud Collaboration:** Real-time collab with CRDT (y-crdt in Rust), comments, version history
- **Notation Pro:** Full engraving (like Dorico), MusicXML import/export advanced, tablature editor
- **MPE / MIDI 2.0 Full:** Per-note expression editing in Piano Roll, MIDI 2.0 Property Exchange
- **Modular Environment (Logic's Environment):** Visual patching environment for MIDI routing (like Max) — rebuild as node graph with `egui_nodes`
- **MainStage Integration:** Perform mode, Patch switching without glitch
- **Mobile / iPad:** iOS build via `winit` + Metal, touch-optimized UI, Apple Pencil for automation
- **Marketplace:** User patches, instruments, IRs sharing, with Rust-based package manager
- **Scripting:** User scripting via Rhai or WASM (like Scripter MIDI FX but for whole DAW)
- **ARA2 Support:** Celemony Melodyne-style integration for Flex Pitch

---

## 5. Acceptance Criteria for Claude

For each feature, Claude must produce:

1.  **Trait Impl:** `struct ChannelEq { ... } impl DspNode { fn process(&mut self, ...) { ... } fn latency(&self) -> usize { ... } }`
2.  **Param Layout:** Exactly match Logic's parameter names and ranges (e.g., Compressor Threshold -50 to 0 dB, Ratio 1:1 to 20:1). Use `nih-plug` params for compatibility
3.  **Preset Test:** Load Logic preset JSON (we will provide `logic-presets/` corpus) and verify output within 0.5dB or 1 cent pitch
4.  **No Alloc Test:** `#[test] fn no_alloc() { assert_no_alloc::assert_no_alloc(|| { let mut node = ChannelEq::new(); node.process(&inputs, &mut outputs); }); }`
5.  **WASM Build:** `cargo check -p prowave-dsp --target wasm32-unknown-unknown` must pass

---

## 6. Performance Targets

| Metric | Target |
|---|---|
| RTL @ 128 samples / 48kHz | macOS <8ms, Win ASIO <10ms, Linux JACK <8ms |
| 100 tracks + 200 plugins | CPU <70% on M1 Pro / i7-12700 |
| WASM | 32 tracks stereo in Chrome with Worklet |
| Engine alloc on audio thread | 0 per callback |
| PDC accuracy | Sample-accurate, null test -90dB |

---

Next step for Claude: Start Phase 0. `cargo new --lib crates/prowave-engine && cargo xtask init`
Then implement transport clock with `Transport { sample_rate: f64, tempo_map: TempoMap, position: SamplePosition }`


---

## 10. Official Logic Pro .logicx Support (New Requirement)

ProWave MUST open and save official Apple Logic Pro .logicx projects on Linux and macOS, while being super lightweight in pure Rust.

See `LOGICX_FORMAT_SPEC.md` for full byte-level spec for Claude.

**Summary:**
- .logicx is a macOS bundle (directory) with `Alternatives/000/ProjectData` binary blob (magic 23 47 C0 AB, reversed FourCC chunks like karT, gRuA, qeSM, GAME anchor for stock plugins, embedded JSON for Drummer)
- Parse via linear scan, no central header table, guard against false positives (printable ASCII, length caps)
- 5 record shapes: AU triple (3x FourCC), GAME (stock plugins), JSON (Drummer/Bass Player), Channel-strip records (16-byte name + 8-byte descriptor), Region records (0x61 0xFF + 24 NULs + u16 LE len)
- Write via seed synthesis method (from LogicProFormatWriter): embed 62KB donor seeds (audio_base.seed, mixed_base.seed, infra.json.gz), clone and patch records, don't hand-craft
- Linux: No AU, map Alchemy -> ProWave Alchemy-lite, Channel EQ -> ProWave EQ, etc. Show "replaced" UI
- macOS: AU hosting via coreaudio-sys, CoreAudio backend, Metal via wgpu
- Lightweight: <50MB binary, <150MB RAM idle, <400ms cold start vs Logic 1.2GB/1.5GB/3-4s, achieved via mimalloc + LTO + strip + lazy loading + rkyv zero-copy + memmap disk streaming

**Crate:** `crates/prowave-logicx-import` with public API `open_logicx(path) -> ProWaveProject` and `save_logicx(project, path)`

**Tests:** Empty project, Drummer project, audio+MIDI project, round-trip, fuzz
