# ProWave engine stress checks

## 24-track streaming

Run on an otherwise idle machine:

```sh
cargo test -p prowave-engine --test multitrack_streaming -- --ignored --nocapture
```

The test generates 24 distinct stereo PCM WAV files at 48 kHz/24-bit in a temporary
directory. It opens them through `AudioFile`, streams them on the real background
disk thread, and renders five seconds in 256-frame blocks paced against a wall clock.
Playback exceeds the per-clip prefetch capacity, exercising continuous refill.

Every output sample is compared with an independently calculated mix. Assertions
check finite output, sample error below 0.000001, zero streaming underruns, and the
final transport position. Debug builds also detect allocations/deallocations inside
the callback. Timing is reported, not asserted, because OS scheduling varies.

This test is ignored by default because it depends on wall-clock scheduling. It is
not a sound-device latency test, an app import test, or a cold-disk benchmark: newly
generated files can be served from the operating system's file cache. It does not
alone establish the complete Phase 1 acceptance criterion across platforms.

Windows debug result, 2026-10-19: 240,000 frames, zero underruns, maximum sample error
approximately 0.000000001, average render load 8.24%, maximum callback 1.0479 ms,
zero callbacks above the 5.333 ms budget. These are one local run's measurements,
not guaranteed performance targets. Temporary audio files are removed after the test.
