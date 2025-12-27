# GlicolVerb

A live-coding guitar pedal VST3/CLAP plugin built with Rust. Write [Glicol](https://glicol.org) DSP code, click "Update", and hear changes instantly.

> **Note**: This is a hobby project for learning Rust and audio processing. Not intended for use yet!

## Status

Phase 1 complete (pass-through audio with GUI). Glicol integration pending.

## Build

```bash
cargo xtask bundle glicol_verb --release
```

Outputs to `target/bundled/`.

## Docs

- [spec.md](spec.md) - Design decisions and API reference
- [plan.md](plan.md) - Implementation phases and architecture
