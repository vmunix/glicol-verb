# GlicolVerb

A live-coding guitar pedal VST3/CLAP plugin built with Rust. Write [Glicol](https://glicol.org) DSP code, click "Update", and hear changes instantly.

> **Note**: This is a hobby project for learning Rust and audio processing. Not intended for use yet!

## Status

Phase 2 complete - live coding with Glicol engine and GUI work! Try effects like:
- `out: ~input >> mul 0.5` - halve volume
- `out: ~input >> lpf 1000.0 1.0` - low-pass filter
- `out: ~input >> plate 0.5` - plate reverb

## Build

```bash
# Clone with submodules
git clone --recursive https://github.com/your/glicol-verb.git

# Or if already cloned, init submodules
git submodule update --init --recursive

# Build plugin
cargo xtask bundle glicol_verb --release
```

Outputs to `target/bundled/`.

## Submodules

This project uses git submodules for reference documentation:
- `vendor/glicol` - Glicol source code for API reference
- `vendor/baseview-latest` - Patched baseview with macOS crash fix

## Testing

This plugin requires a host application (DAW or plugin host) for testing. Recommended options:
- **Carla** (`brew install carla`) - lightweight open-source plugin host
- **REAPER** - full DAW, free to evaluate

Load `target/bundled/glicol_verb.clap` or `.vst3` in your host of choice.

### Test Audio

A synthetic guitar test file is included at `test_audio/test_guitar.wav` (5 seconds, Karplus-Strong synthesized E minor arpeggio). To regenerate:

```bash
cd tools/gen_test_audio && cargo run --release
```

## Docs

- [spec.md](spec.md) - Design decisions and API reference
- [plan.md](plan.md) - Implementation phases and architecture
