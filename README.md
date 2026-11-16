# squawk

A low-level, realtime sound synthesis engine.

This repo is structured so you can develop on macOS (CoreAudio) and move to a 64-bit Raspberry Pi (Linux/aarch64) with minimal changes:

- Engine core: `crates/squawk-engine` (no allocations/locks in the audio callback path)
- Audio host app: `crates/squawk-app` (CPAL output stream)

## Realtime notes (sub-10ms)

To hit sub-10ms end-to-end latency, you typically need:

- 48kHz sample rate
- Small buffers (e.g. 128 frames ≈ 2.7ms; 64 frames ≈ 1.3ms)
- Avoid allocations/locks/syscalls in the audio callback
- On Linux: realtime scheduling + a low-latency kernel and/or `PREEMPT_RT` helps a lot

`squawk-app` requests a 128-frame buffer where the backend supports it, and falls back gracefully if not.

## Prereqs (macOS)

Rust toolchain via Homebrew `rustup`:

1. Install rustup:

   `brew install rustup`

2. Put Homebrew's `cargo` and `rustc` on your PATH. This formula is keg-only, and it no longer installs the `~/.cargo/bin` proxies (`rustup-init` is gone), so `source "$HOME/.cargo/env"` will not find `cargo`:

   `echo 'export PATH="$(brew --prefix rustup)/bin:$PATH"' >> ~/.zshrc`

3. Open a new terminal, then install the stable toolchain:

   `rustup default stable`

## Run on macOS

From the repo root, in a terminal that has the PATH line above:

`cargo run -p squawk-app --release`

## Optional: one-command checks (recommended)

If you install `just`, you can run the common “CI-ish” checks with a single command:

- Install: `brew install just`
- Run checks (format + clippy + tests): `just`
- Run the app: `just run-release`

By default the synth is gated off (silent) until you play a MIDI note.

The current sound engine is a single mass–spring–damper integrated with a semi-implicit Euler step each sample. It is designed so you can swap in other state-update models (multiple masses, waveguides, etc.) without touching the audio driver layer.

### MIDI

- The app will print detected MIDI inputs on startup.
- List inputs without starting audio:

  `cargo run -p squawk-app --release -- --list-midi`

- Select a port via CLI (preferred):

  `cargo run -p squawk-app --release -- --midi-port 1`

- Legacy fallback: set `MIDI_PORT` env (0-based) if you prefer.

### No MIDI device? Use your computer keyboard

Open the UI and enable **Computer keyboard → MIDI notes**.

- Modal voice: `A W S E D F T G Y H U J K` (chromatic, starting at C4; upper notes: `O L P ;`)
- Octave (modal only): Arrow Up/Down, or the Octave buttons
- Velocity: Arrow Left/Right, or the Velocity buttons
- Handpan: each column is one pad, low to high (`D3 A3 Bb3 C4 D4 E4 F4 A4`)
  - Loud: `Z X C V B N M ,`
  - Softer: `S D F G H J K L`
  - Softest: `E R T Y U I O P`

### Debug: pitch estimate (f0)

In the UI, open **Analysis (debug)** and enable **Estimate pitch (f0)**.

- The readout shows cents error relative to the **last Note On** (plus estimated $f_0$ and confidence).
- With **Track around last Note On** enabled, the estimator “locks” around that note’s expected frequency (lowest latency / most stable for debugging).

### Latency tuning (macOS)

- Use `--buffer-frames` to request smaller buffers (e.g. 64 or 128):

  `cargo run -p squawk-app --release -- --buffer-frames 64`

- Change sample rate if needed (default is 48k):

  `... -- --sample-rate 44100`

- If CoreAudio rejects the buffer size, we warn and fall back to the device default. Smaller buffers often require:
  - External USB/Thunderbolt audio interface, or
  - In **Audio MIDI Setup**, select the output device and set “Format” to the desired sample rate, then (if available) reduce the device buffer size.
- Close background apps that might grab exclusive audio control (video conf, screen recorders).

## Build + run on Raspberry Pi (64-bit)

The simplest workflow is to build on the Pi itself (it avoids cross-compiling headaches with ALSA/JACK system deps).

On the Pi (aarch64 OS recommended):

1. Install build deps:

   `sudo apt update && sudo apt install -y build-essential pkg-config libasound2-dev`

2. Install Rust via rustup (recommended):

   `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y`

3. Build + run:

   `source "$HOME/.cargo/env" && cargo build -p squawk-app --release && ./target/release/squawk-app`

### Low-latency tips (Pi/Linux)

- Set CPU governor:
  - `sudo apt install -y cpufrequtils`
  - `sudo cpufreq-set -g performance`
- If you use PipeWire, consider running via JACK compatibility or install JACK directly.
- For best results, use a low-latency kernel and enable realtime scheduling (rtkit).
- Request small buffers on launch:

  `source "$HOME/.cargo/env" && cargo run -p squawk-app --release -- --buffer-frames 64`

## Optional: cross-compile + deploy from macOS

Cross-compiling Linux audio targets can require a sysroot with ALSA headers/libs. If you want the workflow anyway:

- Build for `aarch64-unknown-linux-gnu` (you’ll need a working cross toolchain or Docker-based tooling)
- Deploy with:
  - `PI_HOST=pi.local PI_USER=pi ./scripts/pi_deploy.sh`
  - `PI_HOST=pi.local PI_USER=pi ./scripts/pi_run.sh`
