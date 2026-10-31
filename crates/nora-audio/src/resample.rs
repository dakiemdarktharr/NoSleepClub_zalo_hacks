use crate::{AudioError, MAX_BUFFER_SAMPLES, finite};
use rubato::{FftFixedIn, Resampler};

enum Engine {
    Passthrough,
    Fft(Box<FftFixedIn<f32>>),
}

/// Band-limited mono conversion with reusable caller-owned output buffers.
///
/// Uses Rubato's FFT filter rather than decimation or linear interpolation.
/// Input is fixed-size; pair with FrameAssembler for arbitrary device chunks.
/// Reported output delay must be accounted for when deriving audio timestamps.
pub struct MonoResampler {
    engine: Engine,
    input_samples: usize,
    output_max: usize,
    delay: usize,
}

impl MonoResampler {
    pub fn new(
        input_rate: u32,
        output_rate: u32,
        input_samples: usize,
    ) -> Result<Self, AudioError> {
        const RATES: &[u32] = &[
            8_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000, 96_000, 192_000,
        ];
        if !RATES.contains(&input_rate)
            || !RATES.contains(&output_rate)
            || input_samples == 0
            || input_samples > 16_384
        {
            return Err(AudioError::Configuration(
                "unsupported resampling rate or chunk size",
            ));
        }
        if input_rate == output_rate {
            return Ok(Self {
                engine: Engine::Passthrough,
                input_samples,
                output_max: input_samples,
                delay: 0,
            });
        }
        let fft = FftFixedIn::<f32>::new(
            input_rate as usize,
            output_rate as usize,
            input_samples,
            1,
            1,
        )
        .map_err(|error| AudioError::Backend(error.to_string()))?;
        let output_max = fft.output_frames_max();
        if output_max > MAX_BUFFER_SAMPLES {
            return Err(AudioError::Configuration("resampler output exceeds budget"));
        }
        let delay = fft.output_delay();
        Ok(Self {
            engine: Engine::Fft(Box::new(fft)),
            input_samples,
            output_max,
            delay,
        })
    }

    pub fn input_samples(&self) -> usize {
        self.input_samples
    }
    pub fn output_samples_max(&self) -> usize {
        self.output_max
    }
    pub fn output_delay_samples(&self) -> usize {
        self.delay
    }

    pub fn process_into(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, AudioError> {
        if input.len() != self.input_samples {
            return Err(AudioError::InputLength {
                expected: self.input_samples,
                actual: input.len(),
            });
        }
        if output.len() < self.output_max {
            return Err(AudioError::OutputTooSmall {
                needed: self.output_max,
                provided: output.len(),
            });
        }
        finite(input)?;
        match &mut self.engine {
            Engine::Passthrough => {
                output[..input.len()].copy_from_slice(input);
                Ok(input.len())
            }
            Engine::Fft(engine) => {
                let (_, written) = engine
                    .process_into_buffer(&[input], &mut [&mut *output], None)
                    .map_err(|error| AudioError::Backend(error.to_string()))?;
                if finite(&output[..written]).is_err() {
                    engine.reset();
                    return Err(AudioError::NonFiniteSample);
                }
                Ok(written)
            }
        }
    }

    pub fn reset(&mut self) {
        if let Engine::Fft(engine) = &mut self.engine {
            engine.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_has_no_delay_or_shape_change() {
        let mut resampler = MonoResampler::new(16_000, 16_000, 3).unwrap();
        let mut output = [0.0; 3];
        assert_eq!(resampler.output_delay_samples(), 0);
        assert_eq!(
            resampler
                .process_into(&[0.1, -0.2, 0.3], &mut output)
                .unwrap(),
            3
        );
        assert_eq!(output, [0.1, -0.2, 0.3]);
        assert!(resampler.process_into(&[0.0], &mut output).is_err());
        assert!(resampler.process_into(&[0.0; 3], &mut []).is_err());
    }

    #[test]
    fn downsampling_rejects_out_of_band_tone_without_destroying_passband() {
        let tone_rms = |frequency: f32| {
            let mut resampler = MonoResampler::new(48_000, 16_000, 1440).unwrap();
            let mut input = vec![0.0; 1440];
            let mut output = vec![0.0; resampler.output_samples_max()];
            let mut square_sum = 0.0f64;
            let mut samples = 0usize;
            for chunk in 0..20 {
                for (index, value) in input.iter_mut().enumerate() {
                    let time = (chunk * 1440 + index) as f32 / 48_000.0;
                    *value = (std::f32::consts::TAU * frequency * time).sin() * 0.8;
                }
                let written = resampler.process_into(&input, &mut output).unwrap();
                if chunk > 2 {
                    square_sum += output[..written]
                        .iter()
                        .map(|v| f64::from(*v).powi(2))
                        .sum::<f64>();
                    samples += written;
                }
            }
            (square_sum / samples as f64).sqrt()
        };
        let passband = tone_rms(1000.0);
        let rejected = tone_rms(12_000.0);
        assert!(passband > 0.3, "passband rms {passband}");
        assert!(
            rejected < passband * 0.05,
            "out-of-band rms {rejected}, passband {passband}"
        );
    }

    #[test]
    fn reset_replays_same_state_and_preserves_buffer_capacity() {
        let mut resampler = MonoResampler::new(44_100, 16_000, 441).unwrap();
        let input = vec![0.25; 441];
        let mut output = vec![0.0; resampler.output_samples_max()];
        let initial_capacity = output.capacity();
        let written = resampler.process_into(&input, &mut output).unwrap();
        let first = output[..written].to_vec();
        resampler.reset();
        let repeated = resampler.process_into(&input, &mut output).unwrap();
        assert_eq!(first, output[..repeated]);
        assert_eq!(output.capacity(), initial_capacity);
        assert!(MonoResampler::new(0, 16_000, 1).is_err());
        assert!(MonoResampler::new(48_000, 16_000, 16_385).is_err());
    }
}
