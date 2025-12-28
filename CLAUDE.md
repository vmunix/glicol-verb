# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GlicolVerb is a live-coding guitar pedal VST3/CLAP plugin built with Rust. Users write Glicol DSP code in a text editor, click "Update", and the audio processing changes instantly without dropouts.

**Current Status**: Phase 1 complete (pass-through audio with GUI). Phase 2-4 pending (Glicol integration, parameter injection, polish).

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

## Testing

Testing requires a plugin host (no standalone mode). Use one of:
- **Carla** (`brew install carla`) - lightweight plugin host
- **REAPER** - full DAW, free to evaluate

Load `target/bundled/glicol_verb.clap` or `.vst3` in the host.

## Architecture

### Thread Model
- **GUI Thread**: egui editor with code text field and parameter sliders
- **Audio Thread**: NIH-plug `process()` callback, real-time safe
- **Communication**: `crossbeam_channel` for code updates (GUI→Audio), NIH-plug smoothed params for sliders

### Signal Flow (Target Architecture)
```
DAW Input (variable buffer) → Input Ring Buffer → Glicol Engine (128 samples) → Output Ring Buffer → DAW Output
```

The `BufferBridge` in `src/engine/buffer_bridge.rs` handles the variable-to-fixed block size conversion required because DAWs use variable buffer sizes (64-512) but Glicol processes fixed 128-sample blocks.

### Parameter Injection System
Users reference named variables in Glicol code (`~drive`, `~knob1`). The `ParamInjector` (to be implemented) prepends definitions based on slider values:
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

## Reference Documentation

- `spec.md`: Design decisions, Glicol/NIH-plug API reference, example Glicol code
- `plan.md`: Implementation phases, task checklists, architecture diagrams
