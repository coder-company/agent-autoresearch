use serde::{Deserialize, Serialize};

/// Escalation ladder for stuck recovery.
///
/// Replace brute-force retrying with graduated escalation:
/// - 3 consecutive discards → REFINE (adjust within strategy)
/// - 5 consecutive discards → PIVOT (fundamentally different approach)
/// - 2 PIVOTs without improvement → Web Search
/// - 3 PIVOTs without improvement → Soft Blocker (stop, report)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationState {
    pub consecutive_discards: u32,
    pub pivot_count: u32,
    pub pivots_since_last_keep: u32,
    pub last_action: EscalationAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationAction {
    None,
    Refine,
    Pivot,
    WebSearch,
    SoftBlocker,
}

impl Default for EscalationState {
    fn default() -> Self {
        Self {
            consecutive_discards: 0,
            pivot_count: 0,
            pivots_since_last_keep: 0,
            last_action: EscalationAction::None,
        }
    }
}

impl EscalationState {
    /// Record a successful keep — reset all counters.
    pub fn record_keep(&mut self) {
        self.consecutive_discards = 0;
        self.pivots_since_last_keep = 0;
        self.last_action = EscalationAction::None;
    }

    /// Record a discard and return the required escalation action.
    pub fn record_discard(&mut self) -> EscalationAction {
        self.consecutive_discards += 1;
        let action = self.evaluate();
        self.last_action = action;
        action
    }

    /// Record a crash (counts toward escalation the same as discard).
    pub fn record_crash(&mut self) -> EscalationAction {
        self.consecutive_discards += 1;
        let action = self.evaluate();
        self.last_action = action;
        action
    }

    /// Record a no-op (counts toward escalation).
    pub fn record_no_op(&mut self) -> EscalationAction {
        self.consecutive_discards += 1;
        let action = self.evaluate();
        self.last_action = action;
        action
    }

    /// Acknowledge that a PIVOT was executed.
    pub fn acknowledge_pivot(&mut self) {
        self.pivot_count += 1;
        self.pivots_since_last_keep += 1;
        self.consecutive_discards = 0; // Reset after pivot attempt
        self.last_action = EscalationAction::Pivot;
    }

    fn evaluate(&self) -> EscalationAction {
        if self.pivots_since_last_keep >= 3 {
            return EscalationAction::SoftBlocker;
        }
        if self.pivots_since_last_keep >= 2 && self.consecutive_discards >= 3 {
            return EscalationAction::WebSearch;
        }
        if self.consecutive_discards >= 5 {
            return EscalationAction::Pivot;
        }
        if self.consecutive_discards >= 3 {
            return EscalationAction::Refine;
        }
        EscalationAction::None
    }
}

/// Human-readable guidance for each escalation level.
impl EscalationAction {
    pub fn guidance(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::Refine => {
                "REFINE: Adjust within current strategy. Consult lessons, \
                 change parameters or target files, try a variation of what almost worked."
            }
            Self::Pivot => {
                "PIVOT: Abandon current strategy entirely. Re-read all context, \
                 choose a fundamentally different approach. Log the pivot."
            }
            Self::WebSearch => {
                "WEB SEARCH: Multiple pivots failed. Look for external solutions, \
                 research how others have solved similar problems."
            }
            Self::SoftBlocker => {
                "SOFT BLOCKER: 3+ pivots without progress. Stop the run and report \
                 that human review, broader scope, or a better metric is needed."
            }
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::SoftBlocker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escalation_ladder() {
        let mut state = EscalationState::default();

        // First 2 discards: nothing
        assert_eq!(state.record_discard(), EscalationAction::None);
        assert_eq!(state.record_discard(), EscalationAction::None);

        // 3rd discard: refine
        assert_eq!(state.record_discard(), EscalationAction::Refine);

        // 4th: still refine
        assert_eq!(state.record_discard(), EscalationAction::Refine);

        // 5th: pivot
        assert_eq!(state.record_discard(), EscalationAction::Pivot);

        // After pivot acknowledgment, counters reset
        state.acknowledge_pivot();
        assert_eq!(state.consecutive_discards, 0);
        assert_eq!(state.pivots_since_last_keep, 1);
    }

    #[test]
    fn test_keep_resets_everything() {
        let mut state = EscalationState {
            consecutive_discards: 4,
            pivots_since_last_keep: 2,
            ..EscalationState::default()
        };

        state.record_keep();
        assert_eq!(state.consecutive_discards, 0);
        assert_eq!(state.pivots_since_last_keep, 0);
    }

    #[test]
    fn test_soft_blocker_after_3_pivots() {
        let mut state = EscalationState {
            pivots_since_last_keep: 3,
            ..EscalationState::default()
        };

        // Any discard after 3 pivots without keep → soft blocker
        assert_eq!(state.record_discard(), EscalationAction::SoftBlocker);
    }

    #[test]
    fn test_repeated_pivots_escalate_to_web_search() {
        let mut state = EscalationState::default();

        for _ in 0..5 {
            state.record_discard();
        }
        assert_eq!(state.last_action, EscalationAction::Pivot);
        state.acknowledge_pivot();

        for _ in 0..5 {
            state.record_discard();
        }
        assert_eq!(state.last_action, EscalationAction::Pivot);
        state.acknowledge_pivot();

        assert_eq!(state.record_discard(), EscalationAction::None);
        assert_eq!(state.record_discard(), EscalationAction::None);
        assert_eq!(state.record_discard(), EscalationAction::WebSearch);
    }
}
