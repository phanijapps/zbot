//! Recall config types — moved to `gateway-memory` crate.
//! This file remains as a re-export shim for backward compatibility.

pub use gateway_memory::{
    GraphTraversalConfig, KgDecayConfig, MidSessionRecallConfig, PredictiveRecallConfig,
    RecallConfig, SessionOffloadConfig, TemporalDecayConfig,
};
