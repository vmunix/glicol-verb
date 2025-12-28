use hound::{WavSpec, WavWriter};
use std::f32::consts::PI;

const SAMPLE_RATE: u32 = 44100;

fn main() {
    let spec = WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let output_path = "test_guitar.wav";
    let mut writer = WavWriter::create(output_path, spec).expect("Failed to create WAV file");

    // Generate a simple chord progression: E minor arpeggio
    // Notes: E2 (82Hz), G2 (98Hz), B2 (123Hz), E3 (165Hz)
    let notes = [
        (82.41, 0.0),   // E2 at 0s
        (98.00, 0.3),   // G2 at 0.3s
        (123.47, 0.6),  // B2 at 0.6s
        (164.81, 0.9),  // E3 at 0.9s
        (196.00, 1.2),  // G3 at 1.2s
        (246.94, 1.5),  // B3 at 1.5s
        (329.63, 1.8),  // E4 at 1.8s
        (246.94, 2.4),  // B3 at 2.4s
        (196.00, 2.7),  // G3 at 2.7s
        (164.81, 3.0),  // E3 at 3.0s
        (123.47, 3.3),  // B2 at 3.3s
        (98.00, 3.6),   // G2 at 3.6s
        (82.41, 3.9),   // E2 at 3.9s
    ];

    let duration_secs = 5.0;
    let total_samples = (SAMPLE_RATE as f32 * duration_secs) as usize;
    let mut output = vec![0.0f32; total_samples];

    // Generate each note using Karplus-Strong synthesis
    for (freq, start_time) in notes {
        let start_sample = (start_time * SAMPLE_RATE as f32) as usize;
        let note_samples = karplus_strong(freq, 1.8, 0.996); // longer decay, high damping

        for (i, &sample) in note_samples.iter().enumerate() {
            let idx = start_sample + i;
            if idx < total_samples {
                output[idx] += sample * 0.4; // Mix level
            }
        }
    }

    // Normalize and add slight compression
    let max_val = output.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    if max_val > 0.0 {
        for sample in &mut output {
            *sample /= max_val;
            // Soft clipping for natural compression
            *sample = (*sample * 0.8).tanh();
        }
    }

    // Write to WAV
    for sample in output {
        let amplitude = (sample * i16::MAX as f32) as i16;
        writer.write_sample(amplitude).expect("Failed to write sample");
    }

    writer.finalize().expect("Failed to finalize WAV");
    println!("Generated: {}", output_path);
    println!("Duration: {}s, Sample rate: {}Hz, Mono", duration_secs, SAMPLE_RATE);
}

/// Karplus-Strong plucked string synthesis
/// Returns samples for a single note
fn karplus_strong(frequency: f32, duration_secs: f32, decay: f32) -> Vec<f32> {
    let num_samples = (SAMPLE_RATE as f32 * duration_secs) as usize;
    let period = (SAMPLE_RATE as f32 / frequency) as usize;

    // Initialize with noise burst (simulates pick attack)
    let mut buffer: Vec<f32> = (0..period)
        .map(|i| {
            // Mix of noise and initial harmonic content
            let noise = (simple_hash(i as u32) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let harmonic = (2.0 * PI * i as f32 / period as f32).sin();
            noise * 0.7 + harmonic * 0.3
        })
        .collect();

    let mut output = Vec::with_capacity(num_samples);
    let mut index = 0;

    // Attack envelope
    let attack_samples = (0.003 * SAMPLE_RATE as f32) as usize; // 3ms attack

    for i in 0..num_samples {
        let sample = buffer[index];

        // Apply attack envelope
        let envelope = if i < attack_samples {
            i as f32 / attack_samples as f32
        } else {
            1.0
        };

        output.push(sample * envelope);

        // Lowpass filter (average with next sample) + decay
        let next_index = (index + 1) % period;
        buffer[index] = (buffer[index] + buffer[next_index]) * 0.5 * decay;

        index = next_index;
    }

    output
}

/// Simple deterministic hash for reproducible "random" noise
fn simple_hash(mut x: u32) -> u32 {
    x = x.wrapping_mul(0x45d9f3b);
    x ^= x >> 16;
    x = x.wrapping_mul(0x45d9f3b);
    x ^= x >> 16;
    x
}
