use crate::{AudioError, PcmRing};

/// Fixed-memory playback queue with generation-bound producers.
/// A barge-in invalidates all old TTS chunks and clears already queued speech.
pub struct PlaybackQueue {
    ring: PcmRing,
    generation: u64,
    closed: bool,
}

impl PlaybackQueue {
    pub fn new(capacity_samples: usize) -> Result<Self, AudioError> {
        Ok(Self {
            ring: PcmRing::new(capacity_samples)?,
            generation: 0,
            closed: false,
        })
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn queued_samples(&self) -> usize {
        self.ring.len()
    }
    pub fn memory_bytes(&self) -> usize {
        self.ring.memory_bytes()
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn enqueue(&mut self, generation: u64, samples: &[f32]) -> Result<(), AudioError> {
        if self.closed {
            return Err(AudioError::PlaybackClosed);
        }
        if generation != self.generation {
            return Err(AudioError::StaleGeneration);
        }
        self.ring.try_push(samples)
    }

    /// Always fills the output; silence is written on underflow or closed state.
    pub fn render_into(&mut self, output: &mut [f32]) -> usize {
        output.fill(0.0);
        self.ring.read_into(output)
    }

    pub fn interrupt(&mut self) -> Result<u64, AudioError> {
        self.ring.clear();
        if self.closed {
            return Err(AudioError::PlaybackClosed);
        }
        match self.generation.checked_add(1) {
            Some(generation) => {
                self.generation = generation;
                Ok(generation)
            }
            None => {
                self.closed = true;
                Err(AudioError::GenerationExhausted)
            }
        }
    }

    pub fn close(&mut self) {
        self.ring.clear();
        self.closed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn barge_in_rejects_late_chunks_and_flushes_playback() {
        let mut queue = PlaybackQueue::new(8).unwrap();
        let old = queue.generation();
        queue.enqueue(old, &[0.3, 0.4]).unwrap();
        let current = queue.interrupt().unwrap();
        assert_eq!(queue.enqueue(old, &[0.8]), Err(AudioError::StaleGeneration));
        let mut out = [9.0; 4];
        assert_eq!(queue.render_into(&mut out), 0);
        assert_eq!(out, [0.0; 4]);
        queue.enqueue(current, &[0.1]).unwrap();
        assert_eq!(queue.render_into(&mut out), 1);
        assert_eq!(out, [0.1, 0.0, 0.0, 0.0]);
    }
    #[test]
    fn bounded_backpressure_preserves_pending_audio() {
        let mut queue = PlaybackQueue::new(2).unwrap();
        queue.enqueue(0, &[0.1, 0.2]).unwrap();
        assert!(matches!(
            queue.enqueue(0, &[0.3]),
            Err(AudioError::BufferFull { .. })
        ));
        let mut out = [0.0; 2];
        queue.render_into(&mut out);
        assert_eq!(out, [0.1, 0.2]);
    }
    #[test]
    fn close_and_generation_exhaustion_fail_closed() {
        let mut queue = PlaybackQueue::new(2).unwrap();
        queue.generation = u64::MAX;
        queue.enqueue(u64::MAX, &[0.2]).unwrap();
        assert_eq!(queue.interrupt(), Err(AudioError::GenerationExhausted));
        assert_eq!(queue.queued_samples(), 0);
        assert_eq!(
            queue.enqueue(u64::MAX, &[0.2]),
            Err(AudioError::PlaybackClosed)
        );
    }
}
