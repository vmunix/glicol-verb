# Project Spec: "GlicolVerb" - Live Coding Guitar Pedal VST

## 1. Project Overview
**Goal:** Build a VST3 audio plugin that processes live guitar input using the **Glicol** audio programming language.
**Core Feature:** The plugin interface will contain a text editor. The user writes Glicol code (e.g., `~out: ~input >> delay 0.5`), clicks "Update," and the DSP changes instantly without audio dropouts.
**Target Platform:** Windows/macOS/Linux (VST3 & CLAP).

---

## 2. Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Glicol API** | High-level `glicol` crate | Simpler API with `Engine<N>::new()` and `update_with_code()`. Good for text-based live coding. |
| **Parameter Strategy** | Named variables in code | Users reference `~knob1`, `~drive` in their code. Plugin injects values. Clean separation of UI and DSP logic. |
| **Audio Config** | Mono in → Stereo out | Guitar input is mono, but stereo output for DAW compatibility and future stereo effects. |
| **Editor Style** | Basic egui TextEdit | Start simple. Syntax highlighting can be added later as enhancement. |
| **Thread Communication** | Lock-free ring buffers | Real-time safe. No mutex contention on audio thread. |

---

## 3. Technology Stack

| Component | Tool / Crate | Reason |
|-----------|--------------|--------|
| **Plugin Framework** | `nih_plug` | Modern standard for Rust audio plugins. Handles VST3/CLAP wrapping, state management, real-time safety. |
| **GUI Library** | `nih_plug_egui` | Immediate mode GUI. Perfect for embedding a code editor and parameter sliders. |
| **Audio Engine** | `glicol` (high-level) | DSP backend with `Engine<128>` for fixed block processing. Supports hot-swapping via `update_with_code()`. |
| **Buffering** | `ringbuf` | Lock-free ring buffer to bridge DAW variable buffers (64-512 samples) to Glicol's fixed 128-sample blocks. |
| **Thread Safety** | `parking_lot` | Fast RwLock for shared state (code string persistence). |

### Dependencies (Cargo.toml)
```toml
[dependencies]
nih_plug = { git = "https://github.com/robbert-vdh/nih-plug.git", features = ["assert_process_allocs"] }
nih_plug_egui = { git = "https://github.com/robbert-vdh/nih-plug.git" }
glicol = "0.13"
ringbuf = "0.4"
parking_lot = "0.12"
crossbeam-channel = "0.5"  # Thread-safe channels for code messages
```

---

## 4. Architecture

### A. Signal Flow
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

### B. Thread Communication
```
GUI Thread                        Audio Thread
    │                                 │
    │  CodeMessage::UpdateCode(str)   │
    │ ─────────────────────────────→  │  (lock-free ring buffer)
    │                                 │
    │  StatusMessage::Success/Error   │
    │ ←─────────────────────────────  │  (lock-free ring buffer)
    │                                 │
    │  Parameter values (Arc<Params>) │
    │ ←───────────────────────────→   │  (NIH-plug smoothed params)
```

### C. Parameter Injection System
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

This approach:
- Keeps user code clean and readable
- Makes adding new parameters trivial (add FloatParam + injection line)
- Allows DAW automation of parameters
- Parameters smoothed at sample rate for click-free changes

### D. DSP Module Framework

Native Rust DSP modules process audio before/after the Glicol engine:

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

**EQ Module** (`src/dsp/eq.rs`):
- 3-band parametric EQ using biquad filters
- Low shelf (20-500 Hz, ±12 dB)
- Mid peak (200-8000 Hz, ±12 dB, Q 0.5-4.0)
- High shelf (2000-20000 Hz, ±12 dB)

**Delay Module** (`src/dsp/delay.rs`):
- Stereo delay with circular buffer (up to 2 seconds)
- Feedback (0-95%)
- High-cut filter for tape-like warmth
- Wet/dry mix control

---

## 5. Glicol API Reference

### Engine Creation
```rust
use glicol::Engine;

let mut engine = Engine::<128>::new();  // 128-sample block size
engine.set_sr(44100);                    // Set sample rate
engine.set_bpm(120.0);                   // Optional BPM for tempo-synced effects
```

### Code Updates (Hot-Swapping)
```rust
// Glicol handles diffing internally using LCS algorithm
engine.update_with_code("~out: ~input >> mul 0.5");
```

### Audio Processing
```rust
// Input: Vec of mono channel slices
let input_vec = vec![&input_samples[..]];

// Process and get output
let output = engine.next_block(input_vec);

// Output is Vec<Vec<f32>> - access channels
let left = &output[0];
let right = if output.len() > 1 { &output[1] } else { &output[0] };
```

### Special Nodes
- `~input` - External audio input (fed via `next_block()`)
- `sig <value>` - Constant signal generator
- Reference: https://glicol.org/reference

---

## 6. NIH-plug API Reference

### Plugin Trait
```rust
impl Plugin for GlicolVerb {
    const NAME: &'static str = "GlicolVerb";
    const VENDOR: &'static str = "...";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(1),   // Mono in
        main_output_channels: NonZeroU32::new(2),  // Stereo out
        ..AudioIOLayout::const_default()
    }];

    fn params(&self) -> Arc<dyn Params>;
    fn editor(&mut self, _: AsyncExecutor<Self>) -> Option<Box<dyn Editor>>;
    fn initialize(&mut self, _: &AudioIOLayout, config: &BufferConfig, _: &mut impl InitContext<Self>) -> bool;
    fn process(&mut self, buffer: &mut Buffer, _: &mut AuxiliaryBuffers, _: &mut impl ProcessContext<Self>) -> ProcessStatus;
}
```

### Parameters with Derive Macro
```rust
#[derive(Params)]
pub struct GlicolVerbParams {
    #[id = "dry_wet"]
    pub dry_wet: FloatParam,

    #[persist = "glicol-code"]  // Persisted but not automatable
    pub code: Arc<parking_lot::RwLock<String>>,
}
```

### GUI with egui
```rust
use nih_plug_egui::{create_egui_editor, EguiState};

create_egui_editor(
    params.editor_state.clone(),
    (),
    |_, _| {},
    move |ctx, setter, _| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(nih_plug_egui::widgets::ParamSlider::for_param(&params.dry_wet, setter));
        });
    },
)
```

Reference: https://nih-plug.robbertvanderhelm.nl/

---

## 7. Available Parameters

### Core Parameters
| Parameter | ID | Range | Description |
|-----------|-----|-------|-------------|
| Dry/Wet | `dry_wet` | 0.0-1.0 | Mix between original and processed signal |
| Input Gain | `input_gain` | -30 to +30 dB | Boost/cut input before processing |
| Output Gain | `output_gain` | -30 to +30 dB | Boost/cut final output |

### Mappable Parameters (for use in Glicol code as ~name)
| Parameter | ID | Range | Suggested Use |
|-----------|-----|-------|---------------|
| Knob 1-4 | `knob1`-`knob4` | 0.0-1.0 | General purpose |
| Drive | `drive` | 1.0-10.0 | Distortion amount |
| Feedback | `feedback` | 0.0-0.95 | Delay feedback |
| Mix | `mix` | 0.0-1.0 | Effect mix |
| Rate | `rate` | 0.1-20.0 | LFO/modulation rate (Hz) |

### EQ Module Parameters
| Parameter | ID | Range | Description |
|-----------|-----|-------|-------------|
| EQ Bypass | `eq_bypass` | bool | Bypass EQ processing |
| Low Freq | `eq_low_freq` | 20-500 Hz | Low shelf frequency |
| Low Gain | `eq_low_gain` | ±12 dB | Low shelf gain |
| Mid Freq | `eq_mid_freq` | 200-8000 Hz | Mid peak frequency |
| Mid Gain | `eq_mid_gain` | ±12 dB | Mid peak gain |
| Mid Q | `eq_mid_q` | 0.5-4.0 | Mid peak Q (bandwidth) |
| High Freq | `eq_high_freq` | 2000-20000 Hz | High shelf frequency |
| High Gain | `eq_high_gain` | ±12 dB | High shelf gain |

### Delay Module Parameters
| Parameter | ID | Range | Description |
|-----------|-----|-------|-------------|
| Delay Bypass | `delay_bypass` | bool | Bypass delay processing |
| Delay Time | `delay_time` | 1-2000 ms | Delay time |
| Delay Feedback | `delay_feedback` | 0-95% | Feedback amount |
| Delay Mix | `delay_mix` | 0-100% | Wet/dry mix |
| Delay High-Cut | `delay_highcut` | 1000-20000 Hz | High-cut filter frequency |

---

## 8. Example Glicol Code

**Note**: Use `out:` (no tilde) for the output chain. `~out:` creates a reference that doesn't connect to output!

### Pass-through
```
out: ~input
```

### Simple Distortion
```
out: ~input >> mul ~drive >> lpf 3000.0 0.5
```

### Delay with Feedback
```
out: ~input >> delayms 250.0 >> mul 0.6 >> add ~input
```

### Tremolo
```
out: ~input >> mul ~lfo
~lfo: sin ~rate >> mul 0.5 >> add 0.5
```

### Filter Sweep (Auto-Wah)
```
out: ~input >> lpf ~freq 0.7
~freq: sin ~rate >> mul 2000.0 >> add 2500.0
```

---

## 9. Error Handling

| Error | Detection | Response |
|-------|-----------|----------|
| Invalid Glicol code | `update_with_code()` failure | Keep old code running, show error in GUI |
| Buffer underrun | Output ring buffer empty | Output silence, warn user |
| Buffer overrun | Input ring buffer full | Drop oldest samples (graceful degradation) |
| Empty code | Whitespace-only string | Reject update, show error |
| Missing ~out: | Code validation | Reject update, require output chain |

---

## 10. Future Enhancements

- **Syntax Highlighting**: Custom egui widget with layouter for Glicol keywords
- **Preset System**: Save/load code + parameter sets
- **MIDI Mapping**: Map MIDI CC to parameters
- **Stereo Input**: Process L/R independently
- **Undo/Redo**: Code editor history
- **Visualization**: Waveform/spectrum display
- **More Parameters**: Add as needed (depth, width, tone, etc.)

---

## 11. Reference Projects

- **glicol-vst**: https://github.com/glicol/glicol-vst - Official Glicol VST
- **dattorro-vst-rs**: https://github.com/chaosprint/dattorro-vst-rs - Reverb VST using glicol_synth + egui
- **NIH-plug examples**: https://github.com/robbert-vdh/nih-plug/tree/master/plugins
- **Glicol reference**: https://glicol.org/reference
