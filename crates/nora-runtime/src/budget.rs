use nora_llm::{GoalLimits, Usage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GoalUsage {
    pub calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub unresolved_input_tokens: u64,
    pub unresolved_output_tokens: u64,
}

/// Reservations are retained on failure/cancellation because server-side work
/// may already be billed. Count bytes conservatively as tokens, with overhead;
/// settle to authoritative provider usage only after a validated response.
pub struct CallBudget {
    limits: GoalLimits,
    pub usage: GoalUsage,
    pending: Option<(u64, u64)>,
}

impl CallBudget {
    pub fn new(limits: GoalLimits) -> Self {
        Self {
            limits,
            usage: GoalUsage::default(),
            pending: None,
        }
    }
    pub fn restore(limits: GoalLimits, usage: GoalUsage) -> crate::Result<Self> {
        if usage.calls > limits.max_calls
            || usage
                .input_tokens
                .checked_add(usage.unresolved_input_tokens)
                .is_none_or(|n| n > limits.max_input_tokens)
            || usage
                .output_tokens
                .checked_add(usage.unresolved_output_tokens)
                .is_none_or(|n| n > limits.max_output_tokens)
        {
            return Err("persisted usage exceeds current goal budget".into());
        }
        Ok(Self {
            limits,
            usage,
            pending: None,
        })
    }
    /// A failed/cancelled request has unknown server usage. Retain its reserved
    /// charge but permit a later explicit resume to spend the remaining budget.
    pub fn mark_unknown(&mut self) {
        self.pending.take();
    }
    pub fn reserve(&mut self, input_bytes: usize, output_ceiling: u32) -> crate::Result<()> {
        if self.pending.is_some() {
            return Err("previous LLM reservation remains unresolved".into());
        }
        let input: u64 = input_bytes.try_into()?;
        let input = input.checked_add(1024).ok_or("input budget overflow")?;
        let output = u64::from(output_ceiling);
        let calls = self
            .usage
            .calls
            .checked_add(1)
            .ok_or("call budget overflow")?;
        if calls > self.limits.max_calls
            || self
                .usage
                .input_tokens
                .checked_add(self.usage.unresolved_input_tokens)
                .and_then(|n| n.checked_add(input))
                .is_none_or(|n| n > self.limits.max_input_tokens)
            || self
                .usage
                .output_tokens
                .checked_add(self.usage.unresolved_output_tokens)
                .and_then(|n| n.checked_add(output))
                .is_none_or(|n| n > self.limits.max_output_tokens)
        {
            return Err("goal LLM budget exhausted; no additional request was sent".into());
        }
        self.usage.calls = calls;
        self.usage.unresolved_input_tokens += input;
        self.usage.unresolved_output_tokens += output;
        self.pending = Some((input, output));
        Ok(())
    }
    pub fn settle(&mut self, usage: Usage) -> crate::Result<()> {
        let (input, output) = self.pending.take().ok_or("no pending LLM reservation")?;
        self.usage.unresolved_input_tokens -= input;
        self.usage.unresolved_output_tokens -= output;
        self.usage.input_tokens = self
            .usage
            .input_tokens
            .checked_add(usage.input_tokens)
            .ok_or("input usage overflow")?;
        self.usage.output_tokens = self
            .usage
            .output_tokens
            .checked_add(usage.output_tokens)
            .ok_or("output usage overflow")?;
        if self.usage.input_tokens + self.usage.unresolved_input_tokens
            > self.limits.max_input_tokens
            || self.usage.output_tokens + self.usage.unresolved_output_tokens
                > self.limits.max_output_tokens
        {
            return Err("provider usage exceeded goal budget; no tool will execute".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_calls_retain_reservation_and_cannot_be_retried_for_free() {
        let mut b = CallBudget::new(GoalLimits {
            max_calls: 2,
            max_input_tokens: 5000,
            max_output_tokens: 3000,
        });
        b.reserve(500, 2048).unwrap();
        assert!(b.reserve(500, 2048).is_err());
        assert_eq!(b.usage.calls, 1);
        assert_eq!(b.usage.unresolved_output_tokens, 2048);
    }
    #[test]
    fn reasoning_is_not_double_counted_and_overbudget_blocks_effects() {
        let mut b = CallBudget::new(GoalLimits {
            max_calls: 2,
            max_input_tokens: 5000,
            max_output_tokens: 3000,
        });
        b.reserve(500, 2048).unwrap();
        b.settle(Usage {
            input_tokens: 100,
            output_tokens: 1000,
            total_tokens: 1100,
            reasoning_tokens: Some(900),
        })
        .unwrap();
        assert_eq!(b.usage.output_tokens, 1000);
        assert!(b.reserve(500, 2048).is_err());
    }
}
