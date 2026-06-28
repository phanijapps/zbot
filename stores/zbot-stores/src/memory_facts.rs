//! Memory-fact persistence trait.
//!
//! The trait itself lives in the dependency-light `zbot-stores-traits`
//! crate so that consumers deep in the dep graph (notably `agent-tools`,
//! which sits below `gateway-database -> agent-runtime`) can pull in the
//! trait without inheriting `zbot-stores`' transitive dependency on
//! `knowledge-graph` (which would close a cycle through
//! `gateway-database`).
//!
//! This module re-exports the public surface unchanged so existing
//! callers that import `zbot_stores::MemoryFactStore` continue to work.

pub use zbot_stores_traits::{
    MemoryAggregateStats, MemoryFactStore, MemoryHealthMetrics, SkillIndexRow, StrategyFactInsert,
    StrategyFactMatch,
};
