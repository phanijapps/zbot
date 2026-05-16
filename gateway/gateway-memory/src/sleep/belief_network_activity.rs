//! In-memory recorder for recent Belief Network worker cycles (Phase B-6).
//!
//! The sleep-time worker writes one timestamped snapshot per cycle for each
//! belief-network sub-worker (synthesizer / contradiction-detector /
//! propagator). The HTTP layer reads from this recorder to surface the
//! "Belief Network" panel in the Observatory page.
//!
//! Storage is intentionally in-memory and bounded — on restart the
//! history resets. Persistence is a v2 concern; the current data shape is
//! oriented toward live UI consumption, not durable analytics.

use std::collections::VecDeque;
use std::sync::Mutex;

use chrono::{DateTime, Utc};

use crate::sleep::{BeliefPropagationStats, BeliefSynthesisStats, ContradictionDetectionStats};

/// Maximum number of recent cycles retained per worker. Old entries are
/// evicted FIFO once the cap is exceeded.
pub const RECENT_CAPACITY: usize = 20;

/// One timestamped synthesizer-cycle snapshot.
#[derive(Debug, Clone)]
pub struct TimestampedSynthesisStats {
    pub timestamp: DateTime<Utc>,
    pub stats: BeliefSynthesisStats,
}

/// One timestamped contradiction-detector-cycle snapshot.
#[derive(Debug, Clone)]
pub struct TimestampedContradictionStats {
    pub timestamp: DateTime<Utc>,
    pub stats: ContradictionDetectionStats,
}

/// One timestamped propagator snapshot (per propagation event).
#[derive(Debug, Clone)]
pub struct TimestampedPropagationStats {
    pub timestamp: DateTime<Utc>,
    pub stats: BeliefPropagationStats,
}

/// Shared in-memory recorder for the three belief-network workers.
///
/// All `record_*` methods are infallible and use a non-poisoning lock
/// recovery path so a panic in one cycle cannot poison the recorder for
/// later cycles.
#[derive(Debug, Default)]
pub struct RecentBeliefNetworkActivity {
    synthesis: Mutex<VecDeque<TimestampedSynthesisStats>>,
    contradiction: Mutex<VecDeque<TimestampedContradictionStats>>,
    propagation: Mutex<VecDeque<TimestampedPropagationStats>>,
}

impl RecentBeliefNetworkActivity {
    /// Create an empty recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one synthesizer cycle. Drops the oldest entry when at cap.
    pub fn record_synthesis(&self, stats: BeliefSynthesisStats) {
        let entry = TimestampedSynthesisStats {
            timestamp: Utc::now(),
            stats,
        };
        push_capped(&self.synthesis, entry);
    }

    /// Record one contradiction-detector cycle.
    pub fn record_contradiction(&self, stats: ContradictionDetectionStats) {
        let entry = TimestampedContradictionStats {
            timestamp: Utc::now(),
            stats,
        };
        push_capped(&self.contradiction, entry);
    }

    /// Record one propagator invocation.
    pub fn record_propagation(&self, stats: BeliefPropagationStats) {
        let entry = TimestampedPropagationStats {
            timestamp: Utc::now(),
            stats,
        };
        push_capped(&self.propagation, entry);
    }

    /// Snapshot of recent synthesizer cycles, oldest-first.
    pub fn synthesis_history(&self) -> Vec<TimestampedSynthesisStats> {
        snapshot(&self.synthesis)
    }

    /// Snapshot of recent contradiction-detector cycles, oldest-first.
    pub fn contradiction_history(&self) -> Vec<TimestampedContradictionStats> {
        snapshot(&self.contradiction)
    }

    /// Snapshot of recent propagator events, oldest-first.
    pub fn propagation_history(&self) -> Vec<TimestampedPropagationStats> {
        snapshot(&self.propagation)
    }
}

fn push_capped<T>(slot: &Mutex<VecDeque<T>>, entry: T) {
    if let Ok(mut guard) = slot.lock() {
        guard.push_back(entry);
        while guard.len() > RECENT_CAPACITY {
            guard.pop_front();
        }
    }
}

fn snapshot<T: Clone>(slot: &Mutex<VecDeque<T>>) -> Vec<T> {
    slot.lock()
        .map(|g| g.iter().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesis_history_capped_at_recent_capacity() {
        let act = RecentBeliefNetworkActivity::new();
        for i in 0..(RECENT_CAPACITY + 5) {
            act.record_synthesis(BeliefSynthesisStats {
                beliefs_synthesized: i as u64,
                ..Default::default()
            });
        }
        let hist = act.synthesis_history();
        assert_eq!(hist.len(), RECENT_CAPACITY);
        // Oldest 5 entries should have been evicted: history starts at 5.
        assert_eq!(hist.first().unwrap().stats.beliefs_synthesized, 5);
        assert_eq!(
            hist.last().unwrap().stats.beliefs_synthesized,
            (RECENT_CAPACITY + 4) as u64
        );
    }

    #[test]
    fn contradiction_history_capped_at_recent_capacity() {
        let act = RecentBeliefNetworkActivity::new();
        for i in 0..(RECENT_CAPACITY + 3) {
            act.record_contradiction(ContradictionDetectionStats {
                pairs_examined: i as u64,
                ..Default::default()
            });
        }
        let hist = act.contradiction_history();
        assert_eq!(hist.len(), RECENT_CAPACITY);
        assert_eq!(hist.first().unwrap().stats.pairs_examined, 3);
    }

    #[test]
    fn propagation_history_capped_at_recent_capacity() {
        let act = RecentBeliefNetworkActivity::new();
        for i in 0..(RECENT_CAPACITY + 2) {
            act.record_propagation(BeliefPropagationStats {
                beliefs_invalidated: i as u64,
                ..Default::default()
            });
        }
        let hist = act.propagation_history();
        assert_eq!(hist.len(), RECENT_CAPACITY);
        assert_eq!(hist.first().unwrap().stats.beliefs_invalidated, 2);
    }

    #[test]
    fn empty_recorder_returns_empty_history() {
        let act = RecentBeliefNetworkActivity::new();
        assert!(act.synthesis_history().is_empty());
        assert!(act.contradiction_history().is_empty());
        assert!(act.propagation_history().is_empty());
    }
}
