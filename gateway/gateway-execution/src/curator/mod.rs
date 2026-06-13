//! Phase C — LLM-driven ward consolidation curator.
//!
//! Composes the deterministic apply primitives in
//! [`gateway_services::WardCurator`] with an LLM "decide" step and procedure
//! re-keying. Lives in `gateway-execution` because the LLM client and
//! procedure store both live in this crate's dependency graph;
//! `gateway-services` deliberately stays clear of both so the apply step
//! can be tested without LLM machinery.

pub mod consolidate;

pub use consolidate::consolidate_wards;
