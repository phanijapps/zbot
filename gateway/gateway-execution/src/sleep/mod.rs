//! Sleep-time worker — hourly background maintenance of the knowledge graph.
//!
//! Operations: compact duplicates, decay stale non-archival entities,
//! prune orphan candidates. Archival entities are exempt from every op.
//!
//! Tasks 5, 6, and 7 extend this module with `decay`, `pruner`, and `worker`.

pub mod compactor;
pub mod decay;
pub mod pruner;
pub mod worker;

pub use compactor::{CompactionStats, Compactor, PairwiseVerifier};
pub use decay::{DecayConfig, DecayEngine, PruneCandidate};
pub use pruner::{PruneStats, Pruner};
pub use worker::SleepTimeWorker;
