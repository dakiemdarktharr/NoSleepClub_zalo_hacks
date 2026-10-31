use crate::{AudioError, finite};

/// Hard upper bound per Rust-owned PCM buffer (120 seconds at 16 kHz).
/// Product profiles should normally choose much smaller queues.
pub const MAX_BUFFER_SAMPLES: usize = 1_920_000;

fn storage(length: usize) -> Result<Vec<f32>, AudioError> {
    if length == 0 || length > MAX_BUFFER_SAMPLES {
        return Err(AudioError::Configuration(
            "PCM capacity outside supported bound",
        ));
    }
    let mut data = Vec::new();
    data.try_reserve_exact(length)
        .map_err(|_| AudioError::Allocation)?;
    data.resize(length, 0.0);
    Ok(data)
}

/// Fixed-capacity FIFO. A rejected write is atomic: it changes no queued samples.
/// No allocations occur during push/read/clear after construction.
pub struct PcmRing {
    data: Vec<f32>,
    head: usize,
    len: usize,
}

impl PcmRing {
    pub fn new(capacity: usize) -> Result<Self, AudioError> {
        Ok(Self {
            data: storage(capacity)?,
            head: 0,
            len: 0,
        })
    }

    pub fn capacity(&self) -> usize {
        self.data.len()
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn available(&self) -> usize {
        self.capacity() - self.len
    }
    pub fn memory_bytes(&self) -> usize {
        self.data.capacity() * size_of::<f32>()
    }

    pub fn try_push(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        finite(samples)?;
        if samples.len() > self.available() {
            return Err(AudioError::BufferFull {
                requested: samples.len(),
                available: self.available(),
            });
        }
        let tail = (self.head + self.len) % self.capacity();
        let first = samples.len().min(self.capacity() - tail);
        self.data[tail..tail + first].copy_from_slice(&samples[..first]);
        self.data[..samples.len() - first].copy_from_slice(&samples[first..]);
        self.len += samples.len();
        Ok(())
    }

    pub fn read_into(&mut self, output: &mut [f32]) -> usize {
        let count = output.len().min(self.len);
        let first = count.min(self.capacity() - self.head);
        output[..first].copy_from_slice(&self.data[self.head..self.head + first]);
        output[first..count].copy_from_slice(&self.data[..count - first]);
        // Remove data when consumed so an idle queue does not retain old speech.
        self.data[self.head..self.head + first].fill(0.0);
        self.data[..count - first].fill(0.0);
        self.head = (self.head + count) % self.capacity();
        self.len -= count;
        count
    }

    pub fn clear(&mut self) {
        self.data.fill(0.0);
        self.head = 0;
        self.len = 0;
    }
}

/// Accumulates arbitrary chunk boundaries into fixed-size borrowed frames.
/// Callback is synchronous and must copy a frame if it needs to retain it.
pub struct FrameAssembler {
    frame: Vec<f32>,
    used: usize,
}

impl FrameAssembler {
    pub fn new(frame_samples: usize) -> Result<Self, AudioError> {
        Ok(Self {
            frame: storage(frame_samples)?,
            used: 0,
        })
    }
    pub fn frame_samples(&self) -> usize {
        self.frame.len()
    }
    pub fn pending_samples(&self) -> usize {
        self.used
    }

    pub fn push(
        &mut self,
        mut samples: &[f32],
        mut emit: impl FnMut(&[f32]),
    ) -> Result<usize, AudioError> {
        finite(samples)?;
        let mut emitted = 0;
        while !samples.is_empty() {
            let count = (self.frame.len() - self.used).min(samples.len());
            self.frame[self.used..self.used + count].copy_from_slice(&samples[..count]);
            self.used += count;
            samples = &samples[count..];
            if self.used == self.frame.len() {
                emit(&self.frame);
                self.used = 0;
                emitted += 1;
            }
        }
        Ok(emitted)
    }

    /// Emits at most one zero-padded final frame and clears transient audio.
    pub fn finish(&mut self, mut emit: impl FnMut(&[f32])) -> bool {
        let emitted = self.used != 0;
        if emitted {
            self.frame[self.used..].fill(0.0);
            emit(&self.frame);
        }
        self.reset();
        emitted
    }

    pub fn reset(&mut self) {
        self.frame.fill(0.0);
        self.used = 0;
    }
}

/// Downmix interleaved finite PCM using a widened accumulator.
/// Does not allocate and never silently discards an incomplete channel frame.
pub fn downmix_interleaved(
    input: &[f32],
    channels: usize,
    output: &mut [f32],
) -> Result<usize, AudioError> {
    if channels == 0 || channels > 32 {
        return Err(AudioError::Configuration(
            "channels must be between 1 and 32",
        ));
    }
    if !input.len().is_multiple_of(channels) {
        return Err(AudioError::Configuration(
            "incomplete interleaved audio frame",
        ));
    }
    let count = input.len() / channels;
    if output.len() < count {
        return Err(AudioError::OutputTooSmall {
            needed: count,
            provided: output.len(),
        });
    }
    finite(input)?;
    for (index, frame) in input.chunks_exact(channels).enumerate() {
        output[index] =
            (frame.iter().map(|value| f64::from(*value)).sum::<f64>() / channels as f64) as f32;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_wraparound_and_atomic_overflow() {
        let mut ring = PcmRing::new(5).unwrap();
        ring.try_push(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        let mut two = [0.0; 2];
        assert_eq!(ring.read_into(&mut two), 2);
        assert_eq!(two, [1.0, 2.0]);
        ring.try_push(&[5.0, 6.0, 7.0]).unwrap();
        assert!(matches!(
            ring.try_push(&[8.0]),
            Err(AudioError::BufferFull { .. })
        ));
        let mut all = [0.0; 5];
        assert_eq!(ring.read_into(&mut all), 5);
        assert_eq!(all, [3.0, 4.0, 5.0, 6.0, 7.0]);
        assert!(ring.is_empty());
        assert!(ring.data.iter().all(|value| *value == 0.0));
    }

    #[test]
    fn invalid_samples_do_not_mutate_queue() {
        let mut ring = PcmRing::new(3).unwrap();
        ring.try_push(&[0.2]).unwrap();
        assert_eq!(ring.try_push(&[f32::NAN]), Err(AudioError::NonFiniteSample));
        assert_eq!(ring.len(), 1);
        ring.clear();
        assert_eq!(ring.available(), 3);
    }

    #[test]
    fn rejects_unbounded_buffers() {
        assert!(PcmRing::new(0).is_err());
        assert!(PcmRing::new(MAX_BUFFER_SAMPLES + 1).is_err());
    }

    #[test]
    fn frame_output_is_chunk_invariant_and_padding_is_explicit() {
        let input: Vec<f32> = (0..17).map(|i| i as f32).collect();
        let collect = |chunk_size| {
            let mut assembler = FrameAssembler::new(4).unwrap();
            let mut frames = Vec::new();
            for chunk in input.chunks(chunk_size) {
                assembler
                    .push(chunk, |frame| frames.push(frame.to_vec()))
                    .unwrap();
            }
            assert_eq!(assembler.pending_samples(), 1);
            assert!(assembler.finish(|frame| frames.push(frame.to_vec())));
            assert!(!assembler.finish(|_| panic!("no duplicate final frame")));
            frames
        };
        assert_eq!(collect(1), collect(7));
        assert_eq!(collect(7).last().unwrap(), &vec![16.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn invalid_frame_chunk_does_not_partially_consume() {
        let mut assembler = FrameAssembler::new(3).unwrap();
        assert!(assembler.push(&[0.1, f32::INFINITY], |_| {}).is_err());
        assert_eq!(assembler.pending_samples(), 0);
    }

    #[test]
    fn downmix_checks_shape_capacity_and_finiteness() {
        let mut output = [0.0; 2];
        assert_eq!(
            downmix_interleaved(&[1.0, -1.0, 0.8, 0.2], 2, &mut output).unwrap(),
            2
        );
        assert_eq!(output, [0.0, 0.5]);
        assert!(downmix_interleaved(&[1.0], 2, &mut output).is_err());
        assert!(downmix_interleaved(&[1.0], 0, &mut output).is_err());
        assert!(downmix_interleaved(&[f32::NAN], 1, &mut output).is_err());
        assert!(downmix_interleaved(&[1.0, 2.0], 1, &mut []).is_err());
    }
}
