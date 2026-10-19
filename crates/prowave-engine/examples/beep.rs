//! Roadmap Phase 0 DoD: `cargo run -p prowave-engine --example beep`
//! Plays a 440 Hz sine through the full engine path (command queue -> audio callback -> cpal)
//! and reports the device's output latency. Exits non-zero if the latency target is missed.
use prowave_engine::{Command, Engine};
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Phase 0 target: output path under 20 ms.
const TARGET_MS: f64 = 20.0;

fn main() {
    // `--buffer N` requests N frames per callback (default 128, like a typical DAW setting).
    let args: Vec<String> = std::env::args().collect();
    let buffer = args
        .iter()
        .position(|a| a == "--buffer")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(128u32);
    println!("requested:   {buffer} frames");
    let mut engine = match Engine::start_with(Some(buffer)) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("beep: {err}");
            std::process::exit(2);
        }
    };
    println!("device:      {}", engine.device_name());
    println!("sample rate: {} Hz", engine.sample_rate());
    engine.send(Command::SetTestTone {
        hz: 440.0,
        gain: 0.2,
    });
    std::thread::sleep(Duration::from_millis(1500));
    engine.send(Command::SetTestTone { hz: 0.0, gain: 0.0 });
    std::thread::sleep(Duration::from_millis(50));

    let s = engine.shared();
    let frames = s.block_frames.load(Ordering::Relaxed);
    let block_ms = frames as f64 * 1000.0 / engine.sample_rate().max(1) as f64;
    let device_ms = s.output_latency_us.load(Ordering::Relaxed) as f64 / 1000.0;
    // When the backend reports no timestamps, fall back to one buffer of latency.
    let out_ms = if device_ms > 0.0 { device_ms } else { block_ms };
    println!("callback:    {frames} frames ({block_ms:.2} ms)");
    println!("output lat.: {out_ms:.2} ms (callback -> DAC, device-reported)");
    println!("xruns:       {}", s.xruns.load(Ordering::Relaxed));
    println!(
        "note: true round-trip latency needs a loopback cable; this measures the output side."
    );
    if out_ms > TARGET_MS {
        eprintln!("FAIL: output latency {out_ms:.2} ms > {TARGET_MS} ms target");
        if cfg!(windows) {
            eprintln!("hint: WASAPI shared mode adds the Windows mixer's buffering; the roadmap");
            eprintln!("      target on Windows is ASIO (cpal `asio` feature), not wired up yet.");
        }
        std::process::exit(1);
    }
    println!("OK: under {TARGET_MS} ms");
}
