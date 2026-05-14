//! MemoryServices — single-call factory that constructs every memory
//! subsystem component the gateway needs to wire on startup.
//!
//! Before Phase E, the gateway constructed each component imperatively
//! across ~100 lines of state/mod.rs. This factory takes all the
//! dependencies and returns a struct with `sleep_time_worker` ready to
//! store on `AppState`.

use std::sync::Arc;
use std::time::Duration;

use agent_runtime::llm::embedding::EmbeddingClient;
use zero_stores::KnowledgeGraphStore;
use zero_stores_traits::{
    CompactionStore, ConversationStore, EpisodeStore, MemoryFactStore, ProcedureStore,
};

use crate::intent_router::{IntentClassifier, IntentProfiles};
use crate::sleep::{
    Compactor, ConflictResolver, CorrectionsAbstractor, DecayConfig, DecayEngine, LlmConflictJudge,
    LlmCorrectionsAbstractor, LlmPairwiseVerifier, LlmPatternExtractor, LlmSynthesizer,
    OrphanArchiver, PairwiseVerifier, PatternExtractor, Pruner, SleepOps, SleepTimeWorker,
    Synthesizer,
};
use crate::{KgDecayConfig, MemoryLlmFactory};

/// Inputs needed to construct memory services.
///
/// The gateway populates this once during startup and hands it to
/// [`MemoryServices::new`]. Optional fields (e.g. `embedding_client`) are
/// passed through to the components that can take advantage of them; the
/// factory itself has no policy on when to enable a feature beyond what
/// the caller already decides.
pub struct MemoryServicesConfig {
    pub agent_id: String,
    pub interval: Duration,
    pub llm_factory: Arc<dyn MemoryLlmFactory>,
    pub kg_store: Arc<dyn KnowledgeGraphStore>,
    pub episode_store: Arc<dyn EpisodeStore>,
    pub memory_store: Arc<dyn MemoryFactStore>,
    pub compaction_store: Arc<dyn CompactionStore>,
    pub procedure_store: Arc<dyn ProcedureStore>,
    pub conversation_store: Arc<dyn ConversationStore>,
    pub embedding_client: Option<Arc<dyn EmbeddingClient>>,
    pub kg_decay_config: KgDecayConfig,
    pub corrections_abstractor_interval: Duration,
    pub conflict_resolver_interval: Duration,
    pub decay_config: DecayConfig,
    /// Optional semantic intent router classifier (MEM-008). When set,
    /// the gateway wires it onto `MemoryRecall` via
    /// `set_intent_classifier`. Currently this struct is the assembly
    /// point only; the recall path itself is wired in the state
    /// composition root, not inside this factory.
    pub intent_classifier: Option<Arc<dyn IntentClassifier>>,
    /// Optional per-intent profile bank (MEM-008). When set alongside
    /// `intent_classifier`, queries with a confident intent get a
    /// deep-merged overlay applied to their effective `RecallConfig`.
    pub intent_profiles: Option<Arc<IntentProfiles>>,
}

/// Bundle of ready-to-use memory subsystem handles.
///
/// Currently exposes only `sleep_time_worker` (the only thing the gateway
/// stores on `AppState` from this construction path). Other handles can
/// be added here as the memory crate grows.
pub struct MemoryServices {
    pub sleep_time_worker: Arc<SleepTimeWorker>,
}

impl MemoryServices {
    /// Construct every memory component, assemble [`SleepOps`], start the
    /// worker. One call replaces ~100 lines of imperative construction in
    /// the gateway.
    pub fn new(config: MemoryServicesConfig) -> Self {
        let MemoryServicesConfig {
            agent_id,
            interval,
            llm_factory,
            kg_store,
            episode_store,
            memory_store,
            compaction_store,
            procedure_store,
            conversation_store,
            embedding_client,
            kg_decay_config,
            corrections_abstractor_interval,
            conflict_resolver_interval,
            decay_config,
            // intent router fields are wired onto MemoryRecall by the
            // gateway's state composition root (MemoryServices owns the
            // sleep-time worker; MemoryRecall lives on AppState directly).
            intent_classifier: _,
            intent_profiles: _,
        } = config;

        let verifier: Option<Arc<dyn PairwiseVerifier>> =
            Some(Arc::new(LlmPairwiseVerifier::new(llm_factory.clone())));
        let compactor = Arc::new(Compactor::new(
            kg_store.clone(),
            compaction_store.clone(),
            verifier,
        ));

        let decay = Arc::new(DecayEngine::new(kg_store.clone(), decay_config));
        let pruner = Arc::new(Pruner::new(kg_store.clone(), compaction_store.clone()));

        let synth_llm = Arc::new(LlmSynthesizer::new(llm_factory.clone()));
        let synthesizer = Arc::new(Synthesizer::new(
            kg_store.clone(),
            episode_store.clone(),
            memory_store.clone(),
            compaction_store.clone(),
            synth_llm,
            embedding_client.clone(),
        ));

        let pattern_llm = Arc::new(LlmPatternExtractor::new(llm_factory.clone()));
        let pattern_extractor = Arc::new(PatternExtractor::new(
            episode_store.clone(),
            conversation_store.clone(),
            procedure_store.clone(),
            compaction_store.clone(),
            pattern_llm,
        ));

        let orphan_archiver = Arc::new(OrphanArchiver::new(
            kg_store.clone(),
            compaction_store.clone(),
        ));

        let corrections_llm = Arc::new(LlmCorrectionsAbstractor::new(llm_factory.clone()));
        let corrections_abstractor = Arc::new(CorrectionsAbstractor::new(
            memory_store.clone(),
            compaction_store.clone(),
            corrections_llm,
            corrections_abstractor_interval,
        ));

        let conflict_llm = Arc::new(LlmConflictJudge::new(llm_factory.clone()));
        let conflict_resolver = Arc::new(ConflictResolver::new(
            memory_store.clone(),
            compaction_store.clone(),
            conflict_llm,
            conflict_resolver_interval,
        ));

        let ops = SleepOps {
            synthesizer: Some(synthesizer),
            pattern_extractor: Some(pattern_extractor),
            orphan_archiver: Some(orphan_archiver),
            corrections_abstractor: Some(corrections_abstractor),
            conflict_resolver: Some(conflict_resolver),
        };

        let sleep_time_worker = Arc::new(SleepTimeWorker::start_with_ops(
            compactor,
            decay,
            pruner,
            ops,
            kg_decay_config,
            interval,
            agent_id,
        ));

        Self { sleep_time_worker }
    }
}
