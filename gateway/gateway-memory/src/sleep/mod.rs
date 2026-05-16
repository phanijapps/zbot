//! Sleep-time memory components — moved here from gateway/gateway-execution/src/sleep/
//! during the gateway-memory crate extraction (Phase B).

pub mod belief_contradiction_detector;
pub mod belief_network_activity;
pub mod belief_propagator;
pub mod belief_synthesizer;
pub mod compactor;
pub mod conflict_resolver;
pub mod corrections_abstractor;
pub mod decay;
pub mod orphan_archiver;
pub mod pattern_extractor;
pub mod pruner;
pub mod synthesizer;
pub mod verifier;
pub mod worker;

// Convenience re-exports so `crate::sleep::Compactor` etc. resolve inside
// gateway-memory (used by `worker.rs` and the `services` factory). External
// callers still hit the crate-root re-exports in `lib.rs`.
pub use belief_contradiction_detector::{
    BeliefContradictionConfig, BeliefContradictionDetector, ContradictionDetectionStats,
    ContradictionJudgeLlm, ContradictionJudgeResponse, JudgeDecision, LlmContradictionJudge,
};
pub use belief_network_activity::{
    RecentBeliefNetworkActivity, TimestampedContradictionStats, TimestampedPropagationStats,
    TimestampedSynthesisStats, RECENT_CAPACITY,
};
pub use belief_propagator::{BeliefPropagationStats, BeliefPropagator};
pub use belief_synthesizer::{
    BeliefSynthesisLlm, BeliefSynthesisStats, BeliefSynthesizer, LlmBeliefSynthesizer,
    SynthesisLlmResponse,
};
pub use compactor::{CompactionStats, Compactor, PairwiseVerifier};
pub use conflict_resolver::{
    ConflictJudgeLlm, ConflictResolver, ConflictResponse, ConflictStats, LlmConflictJudge,
};
pub use corrections_abstractor::{
    AbstractionLlm, AbstractionStats, CorrectionsAbstractor, LlmCorrectionsAbstractor,
};
pub use decay::{DecayConfig, DecayEngine, PruneCandidate};
pub use orphan_archiver::{OrphanArchiver, OrphanArchiverStats};
pub use pattern_extractor::{
    LlmPatternExtractor, PatternExtractLlm, PatternExtractor, PatternResponse, PatternStats,
    PatternStep,
};
pub use pruner::{PruneStats, Pruner};
pub use synthesizer::{
    LlmSynthesizer, SynthesisInput, SynthesisLlm, SynthesisResponse, SynthesisStats, Synthesizer,
};
pub use verifier::LlmPairwiseVerifier;
pub use worker::{CycleStats, SleepOps, SleepTimeWorker};
