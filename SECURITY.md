# SECURITY.md — ProWave 

## 1. Overview
`prowave` is a DAW that handles untrusted inputs by design: audio files from internet (wav, aiff, caf, mp3, flac, Apple Loops), MIDI files, third-party plugins (CLAP/VST3/AU), project files (`.prowave`, optional `.logicx` import), IRs for Space Designer, sampler instruments (SFZ, EXS), and user patches. A compromise = code execution with mic access, disk access, network exfiltration.

Our posture: **Rust memory safety is necessary but not sufficient.** We need sandboxing, parsing guards, and supply chain integrity.

## 2. Threat Model

| Threat | Actor | Impact | Mitigation |
| :--- | :--- | :--- | :--- |
| **Malicious audio file** (wav with huge chunk, mp3 with XXE, Apple Loop with malformed metadata, IR with huge size) | Internet download, Loop Browser | RCE via decoder, DoS (OOM), zip bomb | `symphonia` (Rust, no C FFI), chunk size limits, `rkyv` validation, IR size limit 500MB |
| **Malicious plugin** (cracked VST, CLAP with RCE) | Cracked plugins, malicious CLAP | RCE, keylogging, mic exfiltration, ransomware | Out-of-process sandbox, no FS/network by default, explicit user consent for FS access, quarantine |
| **Malicious project file** (.prowave zip slip, .logicx import with path traversal, sampler instrument with path traversal) | Shared project from internet, collab | RCE via deserialization, path traversal, overwrite `~/.ssh/` | Schema validation, Zip Slip check (`path.starts_with(canonical_project_dir)`), no `unsafe` deserialization, limit file count |
| **Malicious MIDI file** (SysEx bomb, huge file) | MIDI file import | DoS, RCE via Scripter (Rhai) | SysEx size limit, Rhai sandbox (no FS/network), MIDI file size limit 100MB |
| **Supply chain** (crates.io compromise) | Malicious crate | Backdoor | `cargo-deny`, `cargo-vet`, pinned deps, `Cargo.lock` committed, `deny.toml` bans build.rs network |
| **WASM escape** (XSS via project name, OPFS leak) | Browser context, shared project | XSS, OPFS leak, SharedArrayBuffer leak | CSP `script-src 'self' wasm-unsafe-eval`, COOP/COEP headers, no eval, worklet isolation, sanitize project names |

## 3. Secure Coding Rules

### 3.1 Rust-Specific
- Workspace root: `#![forbid(unsafe_code)]` by default. Allow `unsafe` ONLY in:
  - `prowave-engine/src/driver/cpal_shim.rs` (audio driver FFI)
  - `prowave-plugin-host/src/ffi/clap.rs`, `vst3.rs`, `au.rs` (plugin FFI)
  - `prowave-dsp/src/simd.rs` (SIMD intrinsics)
  - Each `unsafe` block must have `// SAFETY:` comment + `cargo miri` test + comment explaining why safe
- Forbid `unwrap()` / `expect()` in engine, dsp, instruments, plugin-host: `clippy::unwrap_used` = deny, `clippy::expect_used` = deny
- All audio buffers validated: `debug_assert!(buffer.len() <= MAX_BLOCK_SIZE)` where `MAX_BLOCK_SIZE = 8192`
- No `std::process::Command` except in `xtask` and sandbox launcher
- No `std::fs` in audio thread crates

### 3.2 Audio File Parsing (Critical Attack Surface) — Clone Logic's robustness
We parse untrusted files on a **non-audio, non-UI thread** with strict limits:
```rust
const MAX_WAV_CHUNK_SIZE: u64 = 500 * 1024 * 1024; // 500 MB
const MAX_SAMPLE_RATE: u32 = 192_000;
const MAX_CHANNELS: u16 = 64;
const MAX_IR_SIZE: u64 = 500 * 1024 * 1024;
const MAX_PROJECT_FILES: usize = 10_000;
const MAX_MIDI_FILE_SIZE: u64 = 100 * 1024 * 1024;

fn decode(path: &Path) -> Result<AudioBuffer> {
    let file = File::open(path)?;
    let limit_reader = file.take(MAX_WAV_CHUNK_SIZE);
    // symphonia with probe that rejects > MAX_SAMPLE_RATE, > MAX_CHANNELS
}
```
- Use `symphonia` (pure Rust) over `ffmpeg` (C FFI)
- Fuzz all parsers with `cargo fuzz` — corpus in `/fuzz/corpora/` (wav, aiff, caf, mp3, flac, midi, prowave project)
- Zip projects `.prowave`: Prevent Zip Slip via `path.starts_with(canonical_project_dir)` + canonicalize + reject `..` + limit file count to `MAX_PROJECT_FILES`
- Apple Loops: Parse metadata with `symphonia` + custom Apple Loops parser (fuzzed)
- Sampler instruments (SFZ, EXS): Parse with size limits, path traversal check for sample references

### 3.3 Plugin Sandboxing — The Biggest Risk (Logic runs plugins in-process, we will NOT)
```
[ ProWave Main Process ] <--SHM ringbuf (rtrb) + IPC (postcard)--> [ Plugin Sandbox Process per plugin ]
                                                                        seccomp-bpf (Linux) + landlock
                                                                        seatbelt (macOS) + hardened runtime
                                                                        AppContainer + Low IL (Windows)
                                                                        No FS except temp dir, No network, No mic except via DAW SHM
                                                                        5s scan timeout, heartbeat, auto-restart on crash
```

- **Linux:** `seccomp` filter (allow only read/write/futex/mmap), `landlock` FS restriction, optional separate UID, no network
- **macOS:** `sandbox-exec` profile, hardened runtime + `com.apple.security.app-sandbox` for sandbox process only, no JIT in main, JIT only in sandbox if needed for VST3
- **Windows:** Low integrity level + `JOB_OBJECT_SECURITY_NO_GC` + no admin, no network
- Each plugin crash is isolated: engine detects via heartbeat timeout (2s), restarts plugin, project keeps playing, logs to `~/.prowave-studio/logs/sandbox.log`
- Plugin scanner runs in sandbox too, with 5s timeout per plugin. Quarantine on 3 repeated crashes, stored in `quarantine.json`
- Permissions UI: Like Logic, user must approve FS access for sample-based plugins (Kontakt, Sampler with external samples). Explicit prompt: "Vital wants to access ~/Documents - Allow? Deny? Allow Once?" with Remember checkbox

### 3.4 Scripter MIDI FX Security (Rhai/WASM scripting)
Logic's Scripter uses JavaScript. We will use Rhai (embedded, Rust) for MIDI FX scripting:
- Sandbox: No FS, no network, no `eval`, max execution time 10ms per block, max memory 1MB
- For WASM scripting: WASM sandbox with no WASI FS/network except logging
- Fuzz Rhai scripts with `cargo fuzz`

### 3.5 WebAssembly Security
- WASM module runs inside `AudioWorkletGlobalScope` — no DOM access, no `fetch`, no `XMLHttpRequest`
- Build with ES modules + CSP: `script-src 'self' wasm-unsafe-eval; object-src 'none'; base-uri 'self'`
- Require COOP/COEP for `SharedArrayBuffer`: `Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp`, `Cross-Origin-Resource-Policy: same-origin`
- No `eval`, no `Function()` constructor, no `new Function()`
- OPFS project storage: validate all files before load, same limits as native, sanitize filenames (no `..`, no `/`)
- WASM plugins: capability-based, no WASI `fd_write` except logging to console
- Web Remote (Logic Remote clone): WebSocket with authentication token, no unauthenticated control

## 4. Platform Hardening

**macOS:**
- Hardened Runtime, entitlements: `com.apple.security.device.audio-input`, `com.apple.security.files.user-selected.read-write` only, no `com.apple.security.network.client` in main (only in updater process)
- Notarize all binaries, stapled
- AU hosting requires `com.apple.security.cs.allow-unsigned-executable-memory` ONLY in sandbox process, NOT in main DAW process
- Gatekeeper: Developer ID cert

**Windows:**
- ASLR, CFG, DEP, CET Shadow Stack enabled (`/DYNAMICBASE /GUARD:CF /NXCOMPAT /CETCOMPAT`)
- Code signing with EV cert, timestamped
- Installer does not require admin (per-user install to `%LOCALAPPDATA%`), no service

**Linux:**
- No SUID, no SUID binaries
- Flatpak manifest with minimal permissions: `--socket=pulseaudio --filesystem=xdg-Music:rw --filesystem=xdg-Videos:ro` (for video track)
- AppArmor profile optional

## 5. Supply Chain

- Commit `Cargo.lock` (for binary)
- `cargo deny check advisories bans licenses sources` in CI — fail on `GPL-3.0` in engine (we are MIT/Apache-2.0), fail on unmaintained crates, fail on crates with known advisories
- `cargo vet` — vet all new crates, maintained in `supply-chain/config.toml`, require 2 audits for critical crates (symphonia, cpal, rtrb)
- No build scripts that download from network (`deny.toml` bans `build.rs` network access: `allow = []`)
- Reproducible builds: `cargo xtask build --locked`, verify with `cargo-deterministic`
- Dependency update: Renovate bot, but require `cargo vet` pass

## 6. Cryptography & Project Files

- Project file `.prowave` is a zip: `project.json` (rkyv + serde), `media/` (wavs), `sampler_instruments/` (SFZ/EXS), `irs/` (Space Designer IRs), `bounces/`, `plugins.json` (plugin states)
- `project.json` signed with Ed25519 if user enables signing (prevents tampering in collab, verify on load)
- No encryption at rest for v1.0 (DAW projects are large, encryption overhead high), but media can be optionally encrypted via `age` crate for sharing (password-protected bounce)
- Keychain: Use OS keychain for storing plugin licenses? No, let plugins handle themselves, we don't store licenses

## 7. Privacy

- No telemetry by default. Opt-in only, with explicit dialog on first run: "Help improve ProWave by sending anonymous usage stats? No/Yes"
- If opt-in: Use `metrics` crate, no PII, no audio data, only counts (e.g., "Channel EQ used", "Project with 24 tracks bounced")
- Mic access indicator: Always show red dot + "● REC" when engine is capturing, like Logic
- No network calls from engine. Update check is separate process (`prowave-updater`) + opt-in, checks GitHub releases
- GDPR: No data collection without consent, data deletion request honored

## 8. Vulnerability Disclosure

**Please report vulnerabilities responsibly.**

Email: security@prowave.studio (replace with real when repo public)
Or open a private security advisory on GitHub: Security tab -> Report a vulnerability

We follow 90-day disclosure (Google Project Zero style):
1. Acknowledge within 48h
2. Patch within 14 days for RCE, 30 days for DoS, 90 days for low severity
3. Credit reporter in `SECURITY.md` Hall of Fame (opt-in), with CVE if applicable
4. Publish advisory after patch

### What to include in report:
- Crate + version, OS + version, Rust version, repro file (wav/midi/project) minimal, steps to reproduce
- If plugin-related: plugin name + version, OS, sandbox logs from `~/.prowave-studio/logs/sandbox.log` + `quarantine.json`
- If WASM-related: browser + version, console logs, project file
- CVSS score if known

## 9. Security Testing Checklist (for qa-agent and Claude)

- [ ] `cargo fuzz run wav_parser -- -max_total_time=300` (corpus: wav, aiff, caf, mp3, flac)
- [ ] `cargo fuzz run midi_parser -- -max_total_time=300`
- [ ] `cargo fuzz run project_deser -- -max_total_time=300` (zip with path traversal, huge files)
- [ ] `cargo fuzz run apple_loops_parser`
- [ ] `cargo fuzz run sampler_instrument_parser` (SFZ, EXS)
- [ ] `cargo xtask audit` (cargo audit + deny + vet)
- [ ] Plugin crash test: load 100 random plugins from quarantine corpus (include Vital, Serum, Kontakt, FabFilter), ensure main process stays alive, no FD leak
- [ ] WASM CSP test: `npm run test:csp` — ensure no unsafe-eval except wasm-unsafe-eval, no eval, no Function constructor
- [ ] Zip Slip test: project with `../../etc/passwd`, `/etc/passwd`, `C:\Windows\System32\` entries must be rejected
- [ ] Sandbox escape test: Try to open `/etc/passwd` from sandbox process, try to `socket()` from sandbox, must fail
- [ ] Rhai script timeout test: Infinite loop in Scripter MIDI FX must be killed after 10ms
- [ ] Supply chain: `cargo deny check` must pass, `cargo vet` must pass

## 10. Non-Goals / Known Risks We Accept (for transparency)

- We cannot fully prevent a malicious CLAP/VST3 plugin that is granted FS access (user clicked Allow) from stealing files. User consent is the boundary, like Logic. We will show explicit warning: "Allowing FS access gives plugin full read access to that folder. Only allow if you trust plugin."
- Real-time audio requires `mlock` / high priority thread (`SCHED_FIFO` on Linux, `THREAD_TIME_CONSTRAINT_POLICY` on macOS, `AvSetMmThreadCharacteristics` on Windows), which increases DoS risk if engine hangs (audio thread spin). Watchdog thread will kill and restart audio thread if >2x block time (e.g., 2*128/48000 = 5.3ms) for 3 consecutive callbacks
- We cannot prevent user from loading a 10-hour long wav file that OOMs. We have 500MB limit, but user can override in settings with warning
- WASM SharedArrayBuffer requires COOP/COEP headers, which break some third-party integrations (e.g., YouTube embed). We will document this

---
If you find `unsafe` without `// SAFETY:` comment, open a PR immediately. Security > performance. Audio thread safety > features.
