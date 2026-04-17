//! # Session Context Writers
//!
//! Auto-population hooks that transform session lifecycle events into
//! queryable facts in the ctx namespace. Each public function in
//! [`writer`] writes one kind of ctx fact at the right lifecycle moment.
//!
//! Agents never call these directly — the runtime wires them to events
//! (session created, intent analyzed, planner returned, subagent
//! `respond()`, session archived). Agents read the written facts via
//! `memory(action="get_fact", key="ctx.<sid>.<sub_key>")`.
//!
//! See `docs/specs/2026-04-17-session-ctx-memory-bundle.md` for the
//! full data model, key namespace, and ownership rules.

pub mod writer;
