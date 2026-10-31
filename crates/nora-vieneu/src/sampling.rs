// Modified Rust translation of VieNeu v3 Turbo sampling/repetition behavior.
// Original: Pham Nguyen Ngoc Bao, Apache-2.0; see NOTICE.
use std::collections::VecDeque;

use rand::{Rng, SeedableRng, rngs::SmallRng};
use serde::{Deserialize, Serialize};

use crate::{Result, invalid};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingConfig {
    pub temperature: f32,
    pub top_k: usize,
    pub top_p: f32,
    pub repetition_penalty: f32,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            temperature: 0.8,
            top_k: 25,
            top_p: 0.95,
            repetition_penalty: 1.2,
        }
    }
}

impl SamplingConfig {
    pub fn validate(&self) -> Result<()> {
        if !self.temperature.is_finite()
            || self.temperature < 0.0
            || !self.top_p.is_finite()
            || self.top_p <= 0.0
            || self.top_p > 1.0
            || !self.repetition_penalty.is_finite()
            || self.repetition_penalty <= 0.0
        {
            return Err(invalid("invalid sampling parameters"));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ChannelHistory {
    counts: Vec<u16>,
    order: VecDeque<usize>,
    window: usize,
}

impl ChannelHistory {
    pub fn contains(&self, code: usize) -> bool {
        self.counts.get(code).is_some_and(|n| *n > 0)
    }

    pub fn stored_frames(&self) -> usize {
        self.order.len()
    }

    fn add(&mut self, code: usize) -> Result<()> {
        if code >= self.counts.len() {
            return Err(invalid("history code outside vocabulary"));
        }
        if self.window == 0 {
            // Permanent membership mode is still bounded by vocabulary size.
            self.counts[code] = 1;
            return Ok(());
        }
        if self.order.len() == self.window
            && let Some(old) = self.order.pop_front()
        {
            self.counts[old] -= 1;
        }
        self.order.push_back(code);
        self.counts[code] += 1;
        Ok(())
    }
}

#[derive(Debug)]
pub struct RepetitionHistory {
    channels: Vec<ChannelHistory>,
}

impl RepetitionHistory {
    pub fn new(channels: usize, vocab: usize, window: usize) -> Result<Self> {
        if channels == 0 || channels > 64 || vocab == 0 || vocab > 65_536 || window > 4096 {
            return Err(invalid("invalid repetition history bounds"));
        }
        Ok(Self {
            channels: (0..channels)
                .map(|_| ChannelHistory {
                    counts: vec![0; vocab],
                    order: VecDeque::with_capacity(window),
                    window,
                })
                .collect(),
        })
    }

    pub fn channel(&self, channel: usize) -> Result<&ChannelHistory> {
        self.channels
            .get(channel)
            .ok_or_else(|| invalid("history channel out of range"))
    }

    pub fn add(&mut self, channel: usize, code: usize) -> Result<()> {
        self.channels
            .get_mut(channel)
            .ok_or_else(|| invalid("history channel out of range"))?
            .add(code)
    }
}

/// Reuses candidate/probability buffers. The RNG is for audio sampling only,
/// not credentials/security. NumPy and SmallRng seeded waveforms are not bitwise
/// equivalent; `sample_with_uniform` supports distribution/parity fixtures.
pub struct Sampler {
    capacity: usize,
    candidates: Vec<(usize, f64)>,
    probabilities: Vec<f64>,
    rng: SmallRng,
}

impl Sampler {
    pub fn new(capacity: usize, seed: u64) -> Result<Self> {
        if capacity == 0 || capacity > 65_536 {
            return Err(invalid("invalid sampling vocabulary"));
        }
        Ok(Self {
            capacity,
            candidates: Vec::with_capacity(capacity),
            probabilities: Vec::with_capacity(capacity),
            rng: SmallRng::seed_from_u64(seed),
        })
    }

    pub fn sample(
        &mut self,
        logits: &[f32],
        config: SamplingConfig,
        history: Option<&ChannelHistory>,
    ) -> Result<usize> {
        let uniform = self.rng.random::<f64>();
        self.sample_with_uniform(logits, config, history, uniform)
    }

    pub fn sample_with_uniform(
        &mut self,
        logits: &[f32],
        config: SamplingConfig,
        history: Option<&ChannelHistory>,
        uniform: f64,
    ) -> Result<usize> {
        config.validate()?;
        if logits.is_empty()
            || logits.len() > self.capacity
            || !uniform.is_finite()
            || !(0.0..1.0).contains(&uniform)
            || logits.iter().any(|v| !v.is_finite())
        {
            return Err(invalid("invalid logits/uniform draw"));
        }
        self.candidates.clear();
        for (index, &value) in logits.iter().enumerate() {
            let mut value = f64::from(value);
            if history.is_some_and(|h| h.contains(index)) {
                if value < 0.0 {
                    value *= f64::from(config.repetition_penalty);
                } else {
                    value /= f64::from(config.repetition_penalty);
                }
            }
            self.candidates.push((index, value));
        }
        if config.temperature == 0.0 {
            // First index wins a tie, matching numpy.argmax.
            return Ok(self
                .candidates
                .iter()
                .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
                .map_or(0, |v| v.0));
        }
        let k = if config.top_k > 0 {
            config.top_k.min(logits.len())
        } else {
            logits.len()
        };
        let descending =
            |a: &(usize, f64), b: &(usize, f64)| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0));
        if k < self.candidates.len() {
            self.candidates.select_nth_unstable_by(k, descending);
            self.candidates.truncate(k);
        }
        self.candidates.sort_unstable_by(descending);
        let max = self.candidates[0].1;
        self.probabilities.clear();
        let mut sum = 0.0;
        for &(_, score) in &self.candidates {
            let p = ((score - max) / f64::from(config.temperature)).exp();
            self.probabilities.push(p);
            sum += p;
        }
        if !sum.is_finite() || sum <= 0.0 {
            return Err(invalid("invalid softmax mass"));
        }
        for p in &mut self.probabilities {
            *p /= sum;
        }
        let mut prior = 0.0;
        let mut kept = 0;
        let mut mass = 0.0;
        for &p in &self.probabilities {
            if prior >= f64::from(config.top_p) {
                break;
            }
            prior += p;
            mass += p;
            kept += 1;
        }
        let threshold = uniform * mass;
        let mut cumulative = 0.0;
        for i in 0..kept {
            cumulative += self.probabilities[i];
            if threshold < cumulative {
                return Ok(self.candidates[i].0);
            }
        }
        Ok(self.candidates[kept - 1].0)
    }

    pub fn scratch_capacity(&self) -> usize {
        self.candidates.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nucleus_keeps_crossing_token_and_top_k_precedes_it() {
        let mut s = Sampler::new(4, 0).unwrap();
        let cfg = SamplingConfig {
            temperature: 1.0,
            top_k: 2,
            top_p: 0.9,
            repetition_penalty: 1.0,
        };
        let logits = [0.0, 1.0, 2.0, -3.0];
        assert_eq!(s.sample_with_uniform(&logits, cfg, None, 0.0).unwrap(), 2);
        assert_eq!(s.sample_with_uniform(&logits, cfg, None, 0.99).unwrap(), 1);
        let cfg = SamplingConfig { top_p: 0.5, ..cfg };
        assert_eq!(s.sample_with_uniform(&logits, cfg, None, 0.999).unwrap(), 2);
    }

    #[test]
    fn greedy_penalty_sign_and_first_tie() {
        let mut s = Sampler::new(3, 0).unwrap();
        let cfg = SamplingConfig {
            temperature: 0.0,
            repetition_penalty: 2.0,
            ..Default::default()
        };
        let mut hist = RepetitionHistory::new(1, 3, 2).unwrap();
        hist.add(0, 0).unwrap();
        assert_eq!(
            s.sample_with_uniform(&[3.0, 2.0, 1.0], cfg, Some(hist.channel(0).unwrap()), 0.5)
                .unwrap(),
            1
        );
        assert_eq!(
            s.sample_with_uniform(&[-1.0, -1.0, -2.0], cfg, None, 0.5)
                .unwrap(),
            0
        );
        assert_eq!(
            s.sample_with_uniform(
                &[-1.0, -1.5, -2.0],
                cfg,
                Some(hist.channel(0).unwrap()),
                0.5
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn sliding_window_counts_duplicates_correctly() {
        let mut h = RepetitionHistory::new(2, 4, 2).unwrap();
        for code in [1, 1, 2] {
            h.add(0, code).unwrap();
        }
        assert!(h.channel(0).unwrap().contains(1));
        h.add(0, 3).unwrap();
        assert!(!h.channel(0).unwrap().contains(1));
        assert!(!h.channel(1).unwrap().contains(2));
        assert_eq!(h.channel(0).unwrap().stored_frames(), 2);
        assert!(h.add(0, 4).is_err());
    }

    #[test]
    fn sampler_reuses_capacity_and_rejects_nonfinite_values() {
        let mut s = Sampler::new(4, 42).unwrap();
        let cap = s.scratch_capacity();
        for _ in 0..100 {
            s.sample(&[1.0, 2.0, 3.0, 4.0], Default::default(), None)
                .unwrap();
        }
        assert_eq!(cap, s.scratch_capacity());
        for values in [&[][..], &[f32::NAN][..], &[f32::INFINITY][..]] {
            assert!(s.sample(values, Default::default(), None).is_err());
        }
        assert!(
            s.sample_with_uniform(&[1.0], Default::default(), None, 1.0)
                .is_err()
        );
    }
}
