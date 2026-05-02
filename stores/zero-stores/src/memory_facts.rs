//! Memory-fact persistence trait.
//!
//! The trait itself lives in the dependency-light `zero-stores-traits`
//! crate so that consumers deep in the dep graph (notably `agent-tools`,
//! which sits below `gateway-database -> agent-runtime`) can pull in the
//! trait without inheriting `zero-stores`' transitive dependency on
//! `knowledge-graph` (which would close a cycle through
//! `gateway-database`).
//!
//! This module re-exports the public surface unchanged so existing
//! callers that import `zero_stores::MemoryFactStore` continue to work.

pub use zero_stores_traits::{
    MemoryAggregateStats, MemoryFactStore, MemoryHealthMetrics, SkillIndexRow, StrategyFactInsert,
    StrategyFactMatch,
};
