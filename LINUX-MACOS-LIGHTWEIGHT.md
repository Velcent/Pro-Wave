# ProWave Studio — Linux & macOS Lightweight Implementation Spec
**Goal: Super leve, Rust puro, funciona em Linux e macOS, 10-20x mais leve que Logic Pro oficial**

## 1. Por que ProWave é leve

Logic Pro: 1.2GB app + 70GB library, Objective-C++ com 30 anos de legado, CoreData, NSKeyedArchiver, carrega tudo na abertura, 1.5GB RAM idle, 3-4s cold start, só macOS.

ProWave:
- **Binary:** <50MB single binary (Rust release + LTO + strip + mimalloc)
- **RAM idle:** <150MB com 0 tracks (vs 1.5GB Logic)
- **Cold start:** <400ms (vs 3-4s) — lazy loading, no ObjC runtime, rkyv zero-copy
- **Library:** <100MB essenciais (IRs, wavetables), não 70GB. Opcional: ler de `/Library/Application Support/Logic/` se usuário tem Logic instalado
- **Engine:** Mesmo core `prowave-engine` para native e WASM, sem duplicação

## 2. Linux — Como fazer funcionar

### Audio I/O
- Crate `cpal` com backends: ALSA (default), JACK (pro), PipeWire (via ALSA/JACK compat)
- Não usar CoreAudio, não usar AU. Só CLAP (primário, open, Rust-friendly) e VST3 via `vst3-sys`
- Latência: ALSA 128 samples @ 48kHz = ~8ms RTL, JACK <8ms
- Permissões: sem root, Flatpak com `--socket=pulseaudio --filesystem=xdg-Music:rw`

### Dependências Linux
- `libasound2`, `libjack`, `vulkan` (via wgpu)
- Não precisa de `libcoreaudio`, `AudioToolbox`
- Build: `cargo build --release --target x86_64-unknown-linux-gnu`, `cargo build --target aarch64-unknown-linux-gnu` para ARM Linux

### UI
- `winit` + `wgpu` com Vulkan (Linux) — não Metal
- `egui` — immediate mode, funciona igual no Linux e macOS
- Sem Finder bundle magic: `.logicx` é só diretório no Linux, `std::fs::read_dir`

### Testes Linux
- `cargo test --workspace -- --nocapture` no Ubuntu 22.04 + Fedora
- `cargo xtask check` roda clippy, fmt, miri (para unsafe blocks)

## 3. macOS — Como fazer funcionar

### Audio I/O
- `cpal` com CoreAudio backend
- Suporte a AU via `coreaudio-sys` + `rust-au` (ou `coreaudio-rs`). Só no macOS, com feature flag `#[cfg(target_os = "macos")]`
- Aggregate devices (como Logic)
- Metal via wgpu (wgpu escolhe Metal automaticamente no macOS)

### Bundle .logicx no macOS
- `.logicx` é package (diretório com extension que Finder mostra como arquivo)
- Detectar via `is_dir && contains Alternatives/ProjectData`
- Não precisa de `xattr`, mas para salvar .logicx que Finder mostra como package, criar dir com extensão `.logicx` é suficiente. Opcional: set `com.apple.FinderInfo` package bit via `xattr`

### Assinatura & Notarização
- Hardened Runtime: entitlements `com.apple.security.device.audio-input`, `com.apple.security.files.user-selected.read-write`
- AU hosting requer `com.apple.security.cs.allow-unsigned-executable-memory` SÓ no processo sandbox, NÃO no main
- Notarização: `cargo xtask bundle --release` + `codesign` + `notarytool`

### Universal Binary
- `cargo build --target x86_64-apple-darwin --target aarch64-apple-darwin`
- `lipo -create -output ProWaveStudio target/x86_64.../prowave target/aarch64.../prowave`

## 4. Otimizações de Leveza (Rust puro)

1. **Alocador:** `mimalloc` (ou `rpmalloc`) — 10-20% mais rápido que system malloc, menor fragmentação
2. **LTO + Strip:** `Cargo.toml`: `lto = true`, `strip = true`, `codegen-units = 1`
3. **Lazy Loading:** Não carregar todos os plugins na abertura. Só escanear ProjectData (linear scan), instanciar channel strip quando track selecionada
4. **Disk Streaming:** Audio files via `memmap2` + prefetch thread, não carregar tudo na RAM
5. **Zero-copy:** `rkyv` para `project.json`, não `serde_json` que aloca
6. **No GC:** Rust sem GC, sem pausas
7. **SIMD:** `wide` + `std::simd` para DSP, 4x mais rápido que scalar

### Cargo.toml (workspace root)
```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
panic = "abort"
opt-level = 3

[dependencies]
mimalloc = { version = "0.1", default-features = false }
```

### Main.rs
```rust
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    // <400ms startup: only init logger + parse args + create window
    // Defer loading project until after window shown
}
```

## 5. Crate Features para Linux/macOS

```toml
# crates/prowave-engine/Cargo.toml
[features]
default = []
macos-au = ["coreaudio-sys", "rust-au"] # only on macos
linux-jack = ["jack"] # only on linux

# Conditional compilation
#[cfg(target_os = "macos")]
mod au_host;
#[cfg(target_os = "linux")]
mod alsa_host;
```

## 6. Testes de Leveza (para QA)

- `cargo xtask bench -- --bench startup` — deve ser <400ms cold start
- `cargo xtask bench -- --bench ram-idle` — <150MB RSS com 0 tracks
- `cargo xtask bench -- --bench 24-tracks` — 24 tracks 48k/24-bit, CPU <15% no M1 Pro / i7-12700, RAM <300MB
- Comparar com Logic Pro: mesmo projeto 24 tracks no Logic = 1.2GB RAM, ProWave = 300MB

---

**Para Claude:** Começar com `prowave-engine` cross-platform (cpal + wgpu), depois `prowave-logicx-import` com feature `macos` e `linux` separadas. Usar `#[cfg]` para código específico.
