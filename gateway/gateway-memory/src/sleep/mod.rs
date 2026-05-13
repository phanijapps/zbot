//! Sleep-time memory components — moved here from gateway/gateway-execution/src/sleep/
//! during the gateway-memory crate extraction (Phase B).

pub mod compactor;
pub mod conflict_resolver;
pub mod corrections_abstractor;
pub mod decay;
pub mod orphan_archiver;
pub mod pruner;
pub mod synthesizer;
