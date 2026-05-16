//! Belief-contradiction persistence trait.
//!
//! The trait itself lives in the dependency-light `zero-stores-traits`
//! crate so that consumers deep in the dep graph (notably `agent-tools`)
//! can pull in the trait without inheriting `zero-stores`' transitive
//! dependency on `knowledge-graph`.
//!
//! This module re-exports the public surface unchanged so callers can
//! import `zero_stores::BeliefContradictionStore` if they already depend
//! on `zero-stores`.

pub use zero_stores_traits::{
    BeliefContradiction, BeliefContradictionStore, ContradictionType, Resolution,
};
