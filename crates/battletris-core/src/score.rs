//! Score, funds, line counts, and bazaar threshold tracking.

use crate::board::LineClearOutcome;

/// Combined lines between bazaar entries.
pub const BAZAAR_LINE_THRESHOLD: u32 = 20;

/// Per-player score/economy state owned by the deterministic core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerScore {
    score: i32,
    funds: i32,
    lines: u32,
}

impl PlayerScore {
    /// Returns the display score.
    #[must_use]
    pub const fn score(self) -> i32 {
        self.score
    }

    /// Returns spendable funds.
    #[must_use]
    pub const fn funds(self) -> i32 {
        self.funds
    }

    /// Returns total lines cleared by this player.
    #[must_use]
    pub const fn lines(self) -> u32 {
        self.lines
    }

    /// Adds to display score, used by fast-drop scoring.
    pub fn add_score(&mut self, score_delta: i32) {
        self.score += score_delta;
    }

    /// Adds spendable funds. Negative values are allowed for legacy economy effects.
    pub fn add_funds(&mut self, funds_delta: i32) {
        self.funds += funds_delta;
    }

    /// Sets spendable funds after a committed economy transaction.
    pub fn set_funds(&mut self, funds: i32) {
        self.funds = funds;
    }

    /// Spends funds when enough are available.
    pub fn spend_funds(&mut self, amount: i32) -> bool {
        if amount < 0 || self.funds < amount {
            return false;
        }

        self.funds -= amount;
        true
    }

    /// Applies a line clear outcome to funds and line count.
    pub fn apply_line_clear(&mut self, outcome: &LineClearOutcome) {
        self.funds += outcome.funds;
        self.lines += outcome.lines_cleared;
    }
}

/// Legacy bazaar wrap detector based on both players' cumulative line totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BazaarTracker {
    lines_until_bazaar: u32,
}

impl Default for BazaarTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BazaarTracker {
    /// Creates a tracker at the initial legacy threshold.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lines_until_bazaar: BAZAAR_LINE_THRESHOLD,
        }
    }

    /// Returns lines remaining until the next bazaar modulo wrap.
    #[must_use]
    pub const fn lines_until_bazaar(self) -> u32 {
        self.lines_until_bazaar
    }

    /// Observes cumulative player line totals and returns true when the threshold wraps.
    pub fn observe(&mut self, local_lines: u32, opponent_lines: u32) -> bool {
        let combined = local_lines + opponent_lines;
        let remainder = combined % BAZAAR_LINE_THRESHOLD;
        let next = if remainder == 0 {
            BAZAAR_LINE_THRESHOLD
        } else {
            BAZAAR_LINE_THRESHOLD - remainder
        };
        let triggered = next > self.lines_until_bazaar;
        self.lines_until_bazaar = next;
        triggered
    }
}

#[cfg(test)]
mod tests {
    use super::{BazaarTracker, PlayerScore, BAZAAR_LINE_THRESHOLD};
    use crate::board::LineClearOutcome;

    #[test]
    fn line_clear_updates_funds_and_line_count_without_display_score() {
        let mut score = PlayerScore::default();
        score.add_score(28);

        score.apply_line_clear(&LineClearOutcome {
            lines_cleared: 2,
            funds: 12,
            happy_missed: 1,
            cleared_rows: vec![26, 27],
        });

        assert_eq!(score.score(), 28);
        assert_eq!(score.funds(), 12);
        assert_eq!(score.lines(), 2);
    }

    #[test]
    fn bazaar_tracker_uses_legacy_modulo_wrap_cases() {
        let mut tracker = BazaarTracker::new();
        assert_eq!(tracker.lines_until_bazaar(), BAZAAR_LINE_THRESHOLD);

        assert!(!tracker.observe(19, 0));
        assert_eq!(tracker.lines_until_bazaar(), 1);
        assert!(tracker.observe(19, 1));
        assert_eq!(tracker.lines_until_bazaar(), BAZAAR_LINE_THRESHOLD);

        let mut tracker = BazaarTracker::new();
        assert!(!tracker.observe(18, 0));
        assert!(tracker.observe(18, 2));

        let mut tracker = BazaarTracker::new();
        assert!(!tracker.observe(19, 0));
        assert!(tracker.observe(19, 4));
        assert_eq!(tracker.lines_until_bazaar(), 17);

        let mut tracker = BazaarTracker::new();
        assert!(!tracker.observe(39, 0));
        assert!(tracker.observe(39, 2));
        assert_eq!(tracker.lines_until_bazaar(), 19);
    }
}
