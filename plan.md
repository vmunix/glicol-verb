# GlicolVerb Implementation Plan

## Overview
Build a VST3/CLAP live-coding guitar pedal that embeds the Glicol audio DSP engine. Users write Glicol code in a text editor, click "Update", and the DSP changes instantly.

**Key Design Decisions:**
- **Glicol API**: High-level `glicol` crate with `Engine<N>::new()` and `update_with_code()`
- **Parameter Strategy**: Named variables (`~knob1`, `~drive`, etc.) in code that UI params inject
- **Audio Config**: Mono input → Stereo output
- **Editor**: Basic egui TextEdit (no syntax highlighting initially)

---

## File Structure

```
glicol-verb/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Plugin struct, Plugin trait, process(), exports
│   ├── params.rs           # GlicolVerbParams with all FloatParams
│   ├── editor.rs           # egui GUI: text editor + parameter sliders
│   ├── engine/
│   │   ├── mod.rs          # Module exports
│   │   ├── wrapper.rs      # GlicolWrapper - safe abstraction over Engine
│   │   ├── buffer_bridge.rs # Ring buffers: DAW variable → Glicol fixed 128
│   │   └── param_injector.rs # Inject ~knob1 values into code strings
│   ├── messages.rs         # CodeMessage, StatusMessage types
│   └── state.rs            # Serializable plugin state
└── bundled/                # Built plugin output
```

---

## Implementation Phases

### Phase 1: Minimal Viable Plugin ✅ COMPLETE
**Goal**: Plugin loads in DAW, audio passes through

- [x] Create `Cargo.toml` with dependencies
- [x] Implement basic `GlicolVerb` struct with `Plugin` trait
- [x] Create pass-through audio using ring buffers (no Glicol yet)
- [x] Add basic egui editor with text field and "Update" button
- [x] Verify plugin loads and passes audio unchanged
- [x] Bundle VST3 and CLAP plugins

**Files**: `Cargo.toml`, `src/lib.rs`, `src/params.rs`, `src/editor.rs`
**Output**: `target/bundled/glicol_verb.vst3`, `target/bundled/glicol_verb.clap`

### Phase 2: Glicol Integration
**Goal**: Live coding works with basic code

- [ ] Integrate `glicol::Engine<128>` with wrapper
- [ ] Implement `BufferBridge` (DAW variable blocks → Glicol 128-sample blocks)
- [ ] Add code sending from GUI → audio thread via ring buffer
- [ ] Handle `~input` for live audio input
- [ ] Test: `~out: ~input >> mul 0.5` should halve volume

**Files**: `src/engine/wrapper.rs`, `src/engine/buffer_bridge.rs`, `src/messages.rs`

### Phase 3: Parameter System
**Goal**: UI knobs map to Glicol variables via code injection

- [ ] Add `FloatParam` definitions: `knob1-4`, `drive`, `feedback`, `mix`, `rate`
- [ ] Implement `ParamInjector` to prepend `~knob1: sig 0.5` to user code
- [ ] Add `ParamSlider` widgets to GUI's right panel
- [ ] Test DAW automation of parameters
- [ ] Verify smooth parameter changes with `SmoothingStyle::Linear(10.0)`

**Files**: `src/params.rs`, `src/engine/param_injector.rs`, `src/editor.rs`

### Phase 4: Polish and Error Handling
**Goal**: Production-ready stability

- [ ] Add code validation (check for `~out:`, warn if no `~input`)
- [ ] Implement status messages (success/error feedback in GUI)
- [ ] Handle buffer underrun gracefully (output silence, report)
- [ ] Add help text showing available variables
- [ ] Test state persistence (save/load DAW project)

**Files**: All files, focus on `src/engine/wrapper.rs`, `src/editor.rs`

---

## Core Architecture

### Signal Flow
```
DAW Input (variable: 64-512 samples)
    ↓
Input Ring Buffer (2048 samples)
    ↓
[While buffer >= 128 samples]
    Pop 128 → Glicol Engine → Push 128
    ↓
Output Ring Buffer (stereo, 2048 each)
    ↓
DAW Output (variable size)
```

### Thread Communication
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

### Parameter Injection
User writes:
```
~out: ~input >> mul ~drive >> tanh
```

ParamInjector prepends (when `drive = 2.0`):
```
~drive: sig 2.0
~out: ~input >> mul ~drive >> tanh
```

**Adding new parameter** requires only:
1. Add `FloatParam` to `GlicolVerbParams`
2. Add `self.param_injector.set("name", value)` in audio thread
3. Add `ParamSlider` to GUI

---

## Key Dependencies

```toml
[dependencies]
nih_plug = { git = "https://github.com/robbert-vdh/nih-plug.git", features = ["assert_process_allocs"] }
nih_plug_egui = { git = "https://github.com/robbert-vdh/nih-plug.git" }
glicol = "0.13"
ringbuf = "0.3"
parking_lot = "0.12"
```

---

## Critical Code Patterns

### Plugin Definition (lib.rs)
```rust
pub struct GlicolVerb {
    params: Arc<GlicolVerbParams>,
    engine_wrapper: GlicolWrapper<128>,
    buffer_bridge: BufferBridge,
    param_injector: ParamInjector,
    code_receiver: ringbuf::Consumer<CodeMessage>,
    status_sender: ringbuf::Producer<StatusMessage>,
}

impl Plugin for GlicolVerb {
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(1),   // Mono in
        main_output_channels: NonZeroU32::new(2),  // Stereo out
        ..AudioIOLayout::const_default()
    }];
    // ...
}
```

### Parameters (params.rs)
```rust
#[derive(Params)]
pub struct GlicolVerbParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "dry_wet"]
    pub dry_wet: FloatParam,

    #[id = "knob1"]
    pub knob1: FloatParam,

    #[id = "drive"]
    pub drive: FloatParam,

    #[persist = "glicol-code"]
    pub code: Arc<parking_lot::RwLock<String>>,
}
```

### Buffer Bridge (engine/buffer_bridge.rs)
```rust
pub struct BufferBridge {
    input_consumer: Consumer<f32>,
    output_left_producer: Producer<f32>,
    output_right_producer: Producer<f32>,
    input_block: [f32; 128],
}

impl BufferBridge {
    pub fn has_block(&self) -> bool {
        self.input_consumer.len() >= 128
    }
    pub fn pop_input_block(&mut self) -> &[f32] { ... }
    pub fn push_output(&mut self, left: &[f32], right: &[f32]) { ... }
}
```

### Param Injector (engine/param_injector.rs)
```rust
pub struct ParamInjector {
    values: HashMap<String, f32>,
}

impl ParamInjector {
    pub fn set(&mut self, name: &str, value: f32);
    pub fn inject(&self, user_code: &str) -> String {
        // Prepends "~name: sig value" for each referenced param
    }
}
```

---

## GUI Layout

```
+------------------------------------------------------------------+
|  [Update]  | Status: OK / Error: syntax error at line 3          |
+------------------------------------------------------------------+
|                                    |                             |
|  Code Editor (70%)                 |  Parameters (30%)           |
|  +--------------------------+      |  +----------------------+   |
|  | ~out: ~input             |      |  | Core                 |   |
|  |   >> mul ~drive          |      |  |   Dry/Wet   [====]   |   |
|  |   >> tanh                |      |  |   Input Gain [===]   |   |
|  |   >> delay 0.3           |      |  |   Output Gain [==]   |   |
|  |   >> mul ~feedback       |      |  +----------------------+   |
|  |                          |      |  | Mappable (~knob1-4)  |   |
|  +--------------------------+      |  |   Knob 1-4 sliders   |   |
|                                    |  +----------------------+   |
|                                    |  | Effects              |   |
|                                    |  |   Drive, Feedback,   |   |
|                                    |  |   Mix, Rate sliders  |   |
+------------------------------------------------------------------+
| Available: ~input, ~knob1-4, ~drive, ~feedback, ~mix, ~rate      |
+------------------------------------------------------------------+
```

---

## Error Handling

| Error | Detection | Response |
|-------|-----------|----------|
| Invalid Glicol code | `update_with_code()` failure | Keep old code, show error in GUI |
| Buffer underrun | Output ring buffer empty | Output silence, log warning |
| Buffer overrun | Input ring buffer full | Drop oldest samples |
| Empty code | Whitespace-only string | Reject update, show error |
| Missing ~out: | Code validation | Reject update, require output chain |

---

## Files to Create (in order)

1. `Cargo.toml` - Dependencies and build config
2. `src/lib.rs` - Plugin struct, trait impl, exports
3. `src/params.rs` - All parameters and defaults
4. `src/messages.rs` - CodeMessage, StatusMessage enums
5. `src/engine/mod.rs` - Module exports
6. `src/engine/buffer_bridge.rs` - Ring buffer management
7. `src/engine/wrapper.rs` - Glicol Engine wrapper
8. `src/engine/param_injector.rs` - Parameter injection
9. `src/editor.rs` - egui GUI implementation

---

## Testing Checkpoints

- [ ] Phase 1: Plugin loads in DAW, audio passes through
- [ ] Phase 2: `~out: ~input >> mul 0.5` halves volume
- [ ] Phase 3: Moving `drive` slider changes distortion in `~out: ~input >> mul ~drive >> tanh`
- [ ] Phase 4: Save/reload project preserves code and parameters

---

## Future Enhancements (Not in Initial Scope)

- Syntax highlighting (custom egui widget with layouter)
- Preset system (save/load code + parameter sets)
- MIDI mapping for parameters
- Stereo input support
- Undo/redo for code editor
- Waveform/spectrum visualization
