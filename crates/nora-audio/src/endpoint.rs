use crate::AudioError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointConfig {
    pub sample_rate: u32,
    pub frame_samples: usize,
    pub start_threshold: f32,
    pub stop_threshold: f32,
    pub min_speech_ms: u32,
    pub end_silence_ms: u32,
    pub pre_roll_ms: u32,
    pub max_utterance_ms: u32,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000,
            frame_samples: 512,
            start_threshold: 0.5,
            stop_threshold: 0.35,
            min_speech_ms: 96,
            end_silence_ms: 320,
            pre_roll_ms: 160,
            max_utterance_ms: 30_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    Silence,
    MaxDuration,
    EndOfStream,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EndpointEvent {
    SpeechStart {
        start_sample: u64,
    },
    SpeechEnd {
        start_sample: u64,
        end_sample: u64,
        reason: EndReason,
    },
}

/// Hysteresis endpointing over VAD probabilities with bounded scalar state.
/// It does not decide user identity, command meaning, or effect authorization.
pub struct EndpointDetector {
    config: EndpointConfig,
    position: u64,
    candidate_start: Option<u64>,
    active_start: Option<u64>,
    silence_start: Option<u64>,
    paused: bool,
}

impl EndpointDetector {
    pub fn new(config: EndpointConfig) -> Result<Self, AudioError> {
        let frame = config.frame_samples as u64;
        let to_samples = |ms: u32| u64::from(ms) * u64::from(config.sample_rate) / 1000;
        if ![8_000, 16_000, 24_000, 44_100, 48_000].contains(&config.sample_rate)
            || config.frame_samples == 0
            || config.frame_samples > config.sample_rate as usize
            || !config.start_threshold.is_finite()
            || !config.stop_threshold.is_finite()
            || !(0.0..=1.0).contains(&config.start_threshold)
            || !(0.0..=config.start_threshold).contains(&config.stop_threshold)
            || config.min_speech_ms == 0
            || config.end_silence_ms == 0
            || config.max_utterance_ms <= config.min_speech_ms
            || config.max_utterance_ms > 120_000
            || config.pre_roll_ms > 2_000
            || config.end_silence_ms > config.max_utterance_ms
            || frame > to_samples(config.min_speech_ms)
            || to_samples(config.pre_roll_ms) + to_samples(config.min_speech_ms) + frame
                > to_samples(config.max_utterance_ms)
        {
            return Err(AudioError::Configuration(
                "invalid endpoint thresholds or durations",
            ));
        }
        Ok(Self {
            config,
            position: 0,
            candidate_start: None,
            active_start: None,
            silence_start: None,
            paused: false,
        })
    }

    fn samples(&self, ms: u32) -> u64 {
        u64::from(ms) * u64::from(self.config.sample_rate) / 1000
    }
    pub fn position_samples(&self) -> u64 {
        self.position
    }
    pub fn is_speaking(&self) -> bool {
        self.active_start.is_some()
    }

    pub fn reset(&mut self) {
        self.position = 0;
        self.candidate_start = None;
        self.active_start = None;
        self.silence_start = None;
    }

    /// Pausing discards the current uncommitted endpoint and starts a new
    /// relative sample timeline on resume. It never silently completes a goal.
    pub fn set_paused(&mut self, paused: bool) {
        if paused != self.paused {
            self.reset();
            self.paused = paused;
        }
    }

    pub fn push_probability(
        &mut self,
        probability: f32,
    ) -> Result<Option<EndpointEvent>, AudioError> {
        if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
            return Err(AudioError::InvalidProbability);
        }
        if self.paused {
            return Ok(None);
        }
        let frame_start = self.position;
        self.position = self
            .position
            .checked_add(self.config.frame_samples as u64)
            .ok_or(AudioError::Configuration("sample timeline exhausted"))?;
        if let Some(start) = self.active_start {
            if probability >= self.config.stop_threshold {
                self.silence_start = None;
            } else {
                self.silence_start.get_or_insert(frame_start);
            }
            let silence_end = self.silence_start.filter(|silence| {
                self.position - *silence >= self.samples(self.config.end_silence_ms)
            });
            let reason = if silence_end.is_some() {
                Some(EndReason::Silence)
            } else if self.position - start >= self.samples(self.config.max_utterance_ms) {
                Some(EndReason::MaxDuration)
            } else {
                None
            };
            if let Some(reason) = reason {
                let end = if reason == EndReason::Silence {
                    silence_end.unwrap_or(self.position)
                } else {
                    self.position
                };
                self.active_start = None;
                self.silence_start = None;
                self.candidate_start = None;
                return Ok(Some(EndpointEvent::SpeechEnd {
                    start_sample: start,
                    end_sample: end.max(start),
                    reason,
                }));
            }
        } else if probability >= self.config.start_threshold {
            let candidate = *self.candidate_start.get_or_insert(frame_start);
            if self.position - candidate >= self.samples(self.config.min_speech_ms) {
                let start = candidate.saturating_sub(self.samples(self.config.pre_roll_ms));
                self.active_start = Some(start);
                self.candidate_start = None;
                return Ok(Some(EndpointEvent::SpeechStart {
                    start_sample: start,
                }));
            }
        } else {
            self.candidate_start = None;
        }
        Ok(None)
    }

    pub fn finish(&mut self) -> Option<EndpointEvent> {
        let event = self.active_start.map(|start| EndpointEvent::SpeechEnd {
            start_sample: start,
            end_sample: self.silence_start.unwrap_or(self.position).max(start),
            reason: EndReason::EndOfStream,
        });
        self.active_start = None;
        self.candidate_start = None;
        self.silence_start = None;
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn detector() -> EndpointDetector {
        EndpointDetector::new(EndpointConfig {
            sample_rate: 16_000,
            frame_samples: 160,
            min_speech_ms: 20,
            end_silence_ms: 30,
            pre_roll_ms: 10,
            max_utterance_ms: 100,
            ..EndpointConfig::default()
        })
        .unwrap()
    }
    #[test]
    fn noise_bursts_do_not_start_and_hysteresis_holds_speech() {
        let mut detector = detector();
        assert_eq!(detector.push_probability(0.9).unwrap(), None);
        assert_eq!(detector.push_probability(0.1).unwrap(), None);
        assert_eq!(detector.push_probability(0.8).unwrap(), None);
        assert_eq!(
            detector.push_probability(0.9).unwrap(),
            Some(EndpointEvent::SpeechStart { start_sample: 160 })
        );
        assert_eq!(detector.push_probability(0.4).unwrap(), None);
        for _ in 0..2 {
            assert_eq!(detector.push_probability(0.1).unwrap(), None);
        }
        assert_eq!(
            detector.push_probability(0.1).unwrap(),
            Some(EndpointEvent::SpeechEnd {
                start_sample: 160,
                end_sample: 800,
                reason: EndReason::Silence
            })
        );
    }
    #[test]
    fn continuous_speech_is_bounded_and_flush_is_not_duplicated() {
        let mut detector = detector();
        let mut ended = None;
        for _ in 0..12 {
            let event = detector.push_probability(0.9).unwrap();
            if matches!(event, Some(EndpointEvent::SpeechEnd { .. })) {
                ended = event;
                break;
            }
        }
        assert!(matches!(
            ended,
            Some(EndpointEvent::SpeechEnd {
                reason: EndReason::MaxDuration,
                ..
            })
        ));
        assert_eq!(detector.finish(), None);
    }
    #[test]
    fn pause_reset_and_invalid_probability_preserve_contracts() {
        let mut detector = detector();
        assert_eq!(
            detector.push_probability(f32::NAN),
            Err(AudioError::InvalidProbability)
        );
        assert_eq!(detector.position_samples(), 0);
        detector.push_probability(0.9).unwrap();
        detector.set_paused(true);
        for _ in 0..5 {
            assert_eq!(detector.push_probability(0.9).unwrap(), None);
        }
        assert_eq!(detector.position_samples(), 0);
        detector.set_paused(false);
        detector.push_probability(0.9).unwrap();
        detector.push_probability(0.9).unwrap();
        assert!(matches!(
            detector.finish(),
            Some(EndpointEvent::SpeechEnd {
                reason: EndReason::EndOfStream,
                ..
            })
        ));
        assert!(detector.finish().is_none());
    }
    #[test]
    fn invalid_thresholds_and_unbounded_durations_are_rejected() {
        assert!(
            EndpointDetector::new(EndpointConfig {
                stop_threshold: 0.9,
                ..EndpointConfig::default()
            })
            .is_err()
        );
        assert!(
            EndpointDetector::new(EndpointConfig {
                max_utterance_ms: 120_001,
                ..EndpointConfig::default()
            })
            .is_err()
        );
        assert!(
            EndpointDetector::new(EndpointConfig {
                frame_samples: 16_000,
                max_utterance_ms: 100,
                min_speech_ms: 20,
                pre_roll_ms: 0,
                end_silence_ms: 30,
                ..EndpointConfig::default()
            })
            .is_err()
        );
    }

    #[test]
    fn repeated_pause_state_does_not_restart_an_active_turn() {
        let mut detector = detector();
        detector.push_probability(0.9).unwrap();
        detector.set_paused(false);
        assert!(matches!(
            detector.push_probability(0.9).unwrap(),
            Some(EndpointEvent::SpeechStart { .. })
        ));
    }
}
