# GlicolVerb Development Guide

This document covers architecture details, API references, and future plans for GlicolVerb.

For quick-start build commands and project overview, see [CLAUDE.md](./CLAUDE.md).

---

## Project Status

**Current State**: Core implementation complete (Phase 4B). All fundamental features working:

- ✅ VST3/CLAP plugin loads in DAWs
- ✅ Live Glicol code editing with hot-swap
- ✅ Parameter injection system (`~drive`, `~rate`, etc.)
- ✅ DSP modules: 3-band EQ, stereo delay with feedback
- ✅ Code validation and error feedback
- ✅ Buffer underrun handling
- ✅ State persistence (save/load DAW projects)
- ✅ Collapsible accordion UI with recipe presets

---

## Architecture

### Signal Flow

```
DAW Input (variable: 64-512 samples, mono)
    ↓
Input Gain (smoothed)
    ↓
EQ Module (3-band: low shelf @ 200Hz, mid peak @ 1kHz, high shelf @ 4kHz)
    ↓
Input Ring Buffer (2048 samples capacity)
    ↓
[While buffer >= 128 samples]
    Pop 128 samples → Glicol Engine → Push 128 samples (stereo)
    ↓
Output Ring Buffers (L/R, 2048 samples each)
    ↓
Delay Module (stereo delay with feedback + high-cut filter)
    ↓
Dry/Wet Mix
    ↓
Output Gain (smoothed)
    ↓
DAW Output (variable size, stereo)
```

### Thread Communication

```
GUI Thread                        Audio Thread
    │                                 │
    │  CodeMessage::UpdateCode(str)   │
    │ ─────────────────────────────→  │  (crossbeam bounded channel)
    │                                 │
    │  Parameter values (Arc<Params>) │
    │ ←───────────────────────────→   │  (NIH-plug smoothed params)
```

### Parameter Injection

User writes Glicol code referencing named variables:
```
out: ~input >> mul ~drive >> lpf 3000.0 0.5
```

The ParamInjector prepends definitions based on current slider values:
```
~drive: sig 2.5
~rate: sig 1.0
out: ~input >> mul ~drive >> lpf 3000.0 0.5
```

**Adding a new parameter** requires:
1. Add `FloatParam` to `GlicolVerbParams` in `src/params.rs`
2. Add `self.param_injector.set("name", value)` in audio thread
3. Add `ParamSlider` to GUI in `src/editor.rs`

### DSP Module Framework

Native Rust modules process audio before/after Glicol:

```rust
pub type StereoSample = (f32, f32);

pub trait DspModule {
    fn process(&mut self, input: StereoSample) -> StereoSample;
    fn set_sample_rate(&mut self, sample_rate: f32);
    fn reset(&mut self);
    fn set_bypass(&mut self, bypass: bool);
    fn is_bypassed(&self) -> bool;
}
```

**EQ Module** (`src/dsp/eq.rs`): 3-band parametric using biquad filters
**Delay Module** (`src/dsp/delay.rs`): Stereo delay with feedback + high-cut

---

## GUI Layout

```
+------------------------------------------------------------------+
| GlicolVerb    Live-coding guitar effects                          |
+------------------------------------------------------------------+
|                              |                                    |
| LEFT PANEL (fixed 220px)     | RIGHT PANEL (flexible)             |
| +-------------------------+  | +--------------------------------+ |
| | CORE                    |  | | Glicol Code          [Reset]   | |
| |   Dry/Wet    [====]     |  | |            [Update]            | |
| |   Input      [====]     |  | | +----------------------------+ | |
| |   Output     [====]     |  | | | out: ~input >> lpf 1000.0  | | |
| +-------------------------+  | | |   >> plate ~mix            | | |
| | GLICOL (Use in code)    |  | | +----------------------------+ | |
| |   ~drive     [====]     |  | | Variables: ~input ~drive ...  | | |
| |   ~rate      [====]     |  | +--------------------------------+ |
| |   ~mix       [====]     |  |                                    |
| |   ~feedback  [====]     |  | [▼ Effects Lab]                    |
| +-------------------------+  | | RECIPES (click to load)          |
|                              | | [Amp] [Tremolo] [Filter] ...     |
|                              | | [▼ BUILDING BLOCKS]              |
|                              | | Filters: [lpf] [hpf] [onepole]   |
+------------------------------------------------------------------+
```

---

## API Reference

### Glicol Engine

```rust
use glicol::Engine;

let mut engine = Engine::<128>::new();  // 128-sample block size
engine.set_sr(44100);                    // Set sample rate
engine.set_bpm(120.0);                   // Optional BPM

// Hot-swap code (Glicol diffs internally)
engine.update_with_code("out: ~input >> mul 0.5");

// Process audio
let input_vec = vec![&input_samples[..]];
let output = engine.next_block(input_vec);
let left = &output[0];
let right = if output.len() > 1 { &output[1] } else { &output[0] };
```

### NIH-plug Patterns

```rust
// Parameters with derive macro
#[derive(Params)]
pub struct GlicolVerbParams {
    #[id = "dry_wet"]           // DAW-automatable
    pub dry_wet: FloatParam,

    #[persist = "glicol-code"]  // Persisted but not automatable
    pub code: Arc<parking_lot::RwLock<String>>,
}

// Audio I/O layout
const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
    main_input_channels: NonZeroU32::new(1),   // Mono in
    main_output_channels: NonZeroU32::new(2),  // Stereo out
    ..AudioIOLayout::const_default()
}];
```

### Available Parameters

#### Core Parameters
| Parameter | ID | Range | Description |
|-----------|-----|-------|-------------|
| Dry/Wet | `dry_wet` | 0.0-1.0 | Mix between original and processed |
| Input Gain | `input_gain` | -30 to +30 dB | Boost/cut input |
| Output Gain | `output_gain` | -30 to +30 dB | Boost/cut output |

#### Glicol Parameters (use as `~name` in code)
| Parameter | ID | Range | Suggested Use |
|-----------|-----|-------|---------------|
| Drive | `drive` | 1.0-10.0 | Distortion amount |
| Rate | `rate` | 0.1-20.0 | LFO/modulation rate (Hz) |
| Mix | `mix` | 0.0-1.0 | Effect mix |
| Feedback | `feedback` | 0.0-0.95 | Delay feedback |
| Knob 1-4 | `knob1`-`knob4` | 0.0-1.0 | General purpose |

#### EQ Module
| Parameter | ID | Range |
|-----------|-----|-------|
| EQ Bypass | `eq_bypass` | bool |
| Low Freq | `eq_low_freq` | 20-500 Hz |
| Low Gain | `eq_low_gain` | ±12 dB |
| Mid Freq | `eq_mid_freq` | 200-8000 Hz |
| Mid Gain | `eq_mid_gain` | ±12 dB |
| Mid Q | `eq_mid_q` | 0.5-4.0 |
| High Freq | `eq_high_freq` | 2000-20000 Hz |
| High Gain | `eq_high_gain` | ±12 dB |

#### Delay Module
| Parameter | ID | Range |
|-----------|-----|-------|
| Delay Bypass | `delay_bypass` | bool |
| Delay Time | `delay_time` | 1-2000 ms |
| Delay Feedback | `delay_feedback` | 0-95% |
| Delay Mix | `delay_mix` | 0-100% |
| Delay High-Cut | `delay_highcut` | 1000-20000 Hz |

### Glicol Node Reference

| Category | Nodes | Example |
|----------|-------|---------|
| **Oscillators** | `sin`, `saw`, `squ`, `tri`, `noiz`, `imp` | `sin 440` |
| **Filters** | `lpf`, `hpf`, `onepole` | `lpf 1000.0 1.0` (cutoff, Q) |
| **Effects** | `plate`, `delayms`, `delayn` | `plate 0.5` (mix 0-1) |
| **Operators** | `mul`, `add` | `mul 0.5`, `add 100` |
| **Envelopes** | `envperc`, `adsr` | `envperc 0.01 0.1` |
| **Sequencing** | `seq`, `speed`, `choose` | `seq 60 _60 72 _` |
| **Synths** | `sawsynth`, `squsynth`, `trisynth` | `sawsynth 0.01 0.1` |
| **Drums** | `bd`, `sn`, `hh` | `bd 0.3` (decay) |
| **Sampling** | `sp`, `sampler` | `sp \sample_name` |
| **Utility** | `pan`, `balance`, `mix` | `pan 0.5` |
| **Scripting** | `meta` | Custom Rhai scripts |

Full API: `vendor/glicol/js/src/glicol-api.json`

---

## Example Glicol Code

**Critical**: Use `out:` (no tilde) for the output chain. `~out:` won't produce audio!

```glicol
// Pass-through
out: ~input

// Volume control
out: ~input >> mul 0.5

// Low-pass filter
out: ~input >> lpf 1000.0 1.0

// Plate reverb (50% wet)
out: ~input >> plate 0.5

// Tremolo with rate control
out: ~input >> mul ~lfo
~lfo: sin ~rate >> mul 0.5 >> add 0.5

// Filter sweep (auto-wah)
out: ~input >> lpf ~freq 0.7
~freq: sin ~rate >> mul 2000.0 >> add 2500.0

// Chain multiple effects
out: ~input >> lpf 2000.0 0.7 >> plate 0.3
```

---

## Error Handling

| Error | Detection | Response |
|-------|-----------|----------|
| Invalid Glicol code | `update_with_code()` failure | Keep old code, show error in GUI |
| Buffer underrun | Output ring buffer empty | Output silence, log warning |
| Buffer overrun | Input ring buffer full | Drop oldest samples |
| Empty code | Whitespace-only string | Reject update, show error |
| Missing `out:` | Code validation | Reject update, require output chain |

---

## Testing Checklist

When making changes, verify:

- [ ] Plugin loads in DAW without crash
- [ ] Audio passes through with default code
- [ ] `out: ~input >> mul 0.5` halves volume
- [ ] Recipe presets load and work
- [ ] Parameter sliders affect sound after Update
- [ ] EQ bypass toggle works
- [ ] Delay bypass toggle works
- [ ] Save/reload DAW project preserves state

---

## Future Enhancements

### High Priority
- [ ] **Real-time parameter updates**: Currently params only apply on "Update" click
- [ ] **Syntax highlighting**: Custom egui widget with Glicol keyword highlighting
- [ ] **Better error messages**: Parse Glicol errors for user-friendly feedback

### Medium Priority
- [ ] **Preset system**: Save/load code + parameter combinations
- [ ] **MIDI mapping**: Map MIDI CC to parameters
- [ ] **Undo/redo**: Code editor history
- [ ] **Automated releases**: GitHub Actions workflow to build VST3/CLAP for macOS/Windows/Linux on tagged releases, attach as release assets
- [ ] **GitHub Pages site**: Simple download page hosted at `vmunix.github.io/glicol-verb` with install instructions and demo

### Lower Priority
- [ ] **Stereo input**: Process L/R independently
- [ ] **Visualization**: Waveform/spectrum display
- [ ] **Custom distortion node**: Native tanh/clip since Glicol lacks it

---

## Reference Links

- [NIH-plug docs](https://nih-plug.robbertvanderhelm.nl/)
- [Glicol reference](https://glicol.org/reference)
- [glicol-vst](https://github.com/glicol/glicol-vst) - Official Glicol VST
- [baseview issue #169](https://github.com/RustAudio/baseview/issues/169) - Text input limitations
