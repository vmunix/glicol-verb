# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GlicolVerb is a live-coding guitar pedal VST3/CLAP plugin built with Rust. Users write Glicol DSP code in a text editor, click "Update", and the audio processing changes instantly without dropouts.

**Current Status**: Phase 2 complete (live coding with Glicol engine and GUI work). Phase 3-4 pending (parameter injection, polish).

## Build Commands

```bash
# Build debug
cargo build

# Build release
cargo build --release

# Bundle VST3 and CLAP plugins (output: target/bundled/)
cargo xtask bundle glicol_verb --release
```

## Install Plugin (macOS)

```bash
cp -r target/bundled/glicol_verb.vst3 ~/Library/Audio/Plug-Ins/VST3/
cp -r target/bundled/glicol_verb.clap ~/Library/Audio/Plug-Ins/CLAP/
```

## Code Quality

Run these checks before committing (enforced by pre-commit hook):

```bash
# Check formatting (fix with: cargo fmt)
cargo fmt --check

# Lint check (warnings treated as errors for our code)
cargo clippy --package glicol_verb -- -D warnings
```

Note: Vendored dependencies (baseview, glicol) may emit warnings - these are expected and ignored by the pre-commit hook.

## Testing

Testing requires a plugin host (no standalone mode). Use one of:
- **Carla** (`brew install carla`) - lightweight plugin host
- **REAPER** - full DAW, free to evaluate

Load `target/bundled/glicol_verb.clap` or `.vst3` in the host.

**Test audio**: `test_audio/test_guitar.wav` - 5s Karplus-Strong synthesized guitar (E minor arpeggio).

## Architecture

### Thread Model
- **GUI Thread**: egui editor with code text field and parameter sliders
- **Audio Thread**: NIH-plug `process()` callback, real-time safe
- **Communication**: `crossbeam_channel` for code updates (GUI→Audio), NIH-plug smoothed params for sliders

### Signal Flow
```
DAW Input (variable buffer) → Input Ring Buffer → Glicol Engine (128 samples) → Output Ring Buffer → DAW Output
```

The `BufferBridge` in `src/engine/buffer_bridge.rs` handles the variable-to-fixed block size conversion required because DAWs use variable buffer sizes (64-512) but Glicol processes fixed 128-sample blocks.

### Parameter Injection System (Phase 3)
Users will reference named variables in Glicol code (`~drive`, `~knob1`). The `ParamInjector` (Phase 3) will prepend definitions based on slider values:
```
~drive: sig 2.0          # Injected by plugin
~out: ~input >> mul ~drive >> tanh   # User code
```

## Key Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | Plugin struct, NIH-plug trait impl, `process()` loop |
| `src/params.rs` | `GlicolVerbParams` with `#[derive(Params)]`, persisted code string |
| `src/editor.rs` | egui GUI with `create_egui_editor()`, code text field |
| `src/engine/wrapper.rs` | `GlicolWrapper` - safe abstraction over `glicol::Engine<128>` |
| `src/engine/buffer_bridge.rs` | Ring buffers bridging DAW↔Glicol block sizes |
| `src/messages.rs` | `CodeMessage` enum for GUI→Audio communication |

## Key Dependencies

- **nih_plug**: VST3/CLAP plugin framework
- **nih_plug_egui**: Immediate-mode GUI integration
- **glicol**: Audio DSP engine with live code hot-swapping
- **ringbuf**: Lock-free ring buffers for audio bridging
- **crossbeam-channel**: Thread-safe message passing

## Implementation Notes

- Audio config: Mono input → Stereo output
- Glicol block size: 128 samples (const generic `Engine<128>`)
- GUI→Audio: Use `crossbeam_channel` (not ringbuf) for code strings because the Sender must be `Sync`
- Parameters use `#[id = "name"]` for DAW automation, `#[persist = "name"]` for non-automatable state
- The code string is persisted via `Arc<parking_lot::RwLock<String>>` with `#[persist]`

## Known Issues

- **Text input limited**: baseview keyboard handling in plugin hosts can be unreliable. Use preset buttons as workaround. See [baseview #169](https://github.com/RustAudio/baseview/issues/169).
- **No distortion node**: Glicol lacks built-in `tanh`/`clip`. Options: use `mul` for overdrive, `meta` for custom waveshaping, or add custom node.
- **baseview patched**: Using local clone at `vendor/baseview-latest` to get macOS crash fix (PR #204). Update periodically.

## Glicol DSP Reference

Glicol source is available at `vendor/glicol/` (git submodule). Full API docs: `vendor/glicol/js/src/glicol-api.json`

### Syntax Basics

```glicol
out: ~input >> lpf 1000.0 1.0    // Output chain (connects to DAW)
~myref: sin 440                   // Reference chain (internal, doesn't output)
```

**Critical**: Use `out:` (no tilde) for the output node. `~out:` creates a reference that doesn't connect to output!

### Available Nodes

| Category | Nodes | Example |
|----------|-------|---------|
| **Oscillators** | `sin`, `saw`, `squ`, `tri`, `noiz`, `imp` | `sin 440` |
| **Filters** | `lpf`, `hpf`, `onepole` | `lpf 1000.0 1.0` (cutoff, Q) |
| **Effects** | `plate`, `delayms`, `delayn` | `plate 0.5` (mix 0-1) |
| **Operators** | `mul`, `add` | `mul 0.5`, `add 100` |
| **Envelopes** | `envperc`, `adsr` | `envperc 0.01 0.1` (attack, decay) |
| **Sequencing** | `seq`, `speed`, `choose` | `seq 60 _60 72 _` |
| **Synths** | `sawsynth`, `squsynth`, `trisynth` | `sawsynth 0.01 0.1` |
| **Drums** | `bd`, `sn`, `hh` | `bd 0.3` (decay) |
| **Sampling** | `sp`, `sampler` | `sp \sample_name` |
| **Utility** | `pan`, `balance`, `mix` | `pan 0.5` |
| **Scripting** | `meta` | `meta \`output = input.map(\|x\| x * 0.5); output\`` |

### Common Patterns

```glicol
// Pass-through
out: ~input

// Volume control
out: ~input >> mul 0.5

// Low-pass filter
out: ~input >> lpf 1000.0 1.0

// Plate reverb (50% wet)
out: ~input >> plate 0.5

// Delay effect
out: ~input >> delayms 250

// Chain multiple effects
out: ~input >> lpf 2000.0 0.7 >> plate 0.3

// FM synthesis
out: sin ~freq
~freq: sin 5 >> mul 50 >> add 440
```

### No Built-in Distortion

Glicol lacks `tanh`/`clip` nodes. Options:
- Use `mul` for simple overdrive: `mul 3.0` (will clip at DAW level)
- Use `meta` with Rhai script for custom waveshaping
- We may add a custom distortion node in Phase 4

## Reference Documentation

- `spec.md`: Design decisions, NIH-plug API reference
- `plan.md`: Implementation phases, task checklists
- `vendor/glicol/`: Glicol source code (submodule)
- `vendor/glicol/js/src/glicol-api.json`: Complete node API reference
