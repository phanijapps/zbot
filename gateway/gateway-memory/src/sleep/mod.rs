//! Sleep-time memory components — moved here from gateway/gateway-execution/src/sleep/
//! during the gateway-memory crate extraction (Phase B).

pub mod compactor;
pub mod conflict_resolver;
pub mod corrections_abstractor;
pub mod decay;
pub mod handoff_writer;
pub mod orphan_archiver;
pub mod pattern_extractor;
pub mod pruner;
pub mod synthesizer;
pub mod verifier;
pub mod worker;

// Convenience re-exports so `crate::sleep::Compactor` etc. resolve inside
// gateway-memory (used by `worker.rs` and the `services` factory). External
// callers still hit the crate-root re-exports in `lib.rs`.
pub use compactor::{CompactionStats, Compactor, PairwiseVerifier};
pub use conflict_resolver::{
    ConflictJudgeLlm, ConflictResolver, ConflictResponse, ConflictStats, LlmConflictJudge,
};
pub use corrections_abstractor::{
    AbstractionLlm, AbstractionStats, CorrectionsAbstractor, LlmCorrectionsAbstractor,
};
pub use decay::{DecayConfig, DecayEngine, PruneCandidate};
pub use handoff_writer::{
    read_handoff_block, should_inject, HandoffEntry, HandoffInput, HandoffLlm, HandoffWriter,
    LlmHandoffWriter,
};
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
