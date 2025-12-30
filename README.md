# GlicolVerb

A Glicol live-coding VST3/CLAP plugin built with Rust and
[NIH-plug](https://github.com/robbert-vdh/nih-plug). Write
[Glicol](https://glicol.org) DSP code, click "Update", and hear changes
instantly.

**Note**: This is a hobby project for learning Rust and audio
processing. Feedback is welcome, but know that it's very much a work in
progress!

## Status

<img width="900" height="598" alt="SCR-20251230-kdtc" src="https://github.com/user-attachments/assets/2655d4d5-8344-43ca-8e66-bfe8229edf27" />


Phase 2 complete - live coding with Glicol engine and GUI work! Try effects like:
- `out: ~input >> mul 0.5` - halve volume
- `out: ~input >> lpf 1000.0 1.0` - low-pass filter
- `out: ~input >> plate 0.3` - plate reverb

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

Load `target/bundled/glicol_verb.vst3` in your host of choice.

### Test Audio

A synthetic guitar test file is included at `test_audio/test_guitar.wav` (5 seconds, Karplus-Strong synthesized E minor arpeggio). To regenerate:

```bash
cd tools/gen_test_audio && cargo run --release
```

## License Note

The *code* here is MIT licensed, but the VST3 interface used by NIH-plug is
GPLv3. Which implies that the compiled VST3 plugin needs to comply with
the terms of the GPLv3 license. The CLAP interface does not drag GPLv3 along for
the ride, so that's a way out if you can't deal with GPLv3 for some reason.

## Docs

- [spec.md](spec.md) - Design decisions and API reference
- [plan.md](plan.md) - Implementation phases and architecture
