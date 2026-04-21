//! Executor trusts agent.thinking_enabled regardless of registry capability.

use gateway_execution::invoke::resolve_thinking_flag;

#[test]
fn thinking_enabled_respected_for_registry_unknown_model() {
    // The behaviour under test is pure: given thinking_enabled=true and a
    // model absent from the registry, the executor should forward the flag
    // as true. We test the helper that the executor uses so the assertion
    // is unit-level rather than a full invoke wiring.
    let out = resolve_thinking_flag(true, "some-unknown-model");
    assert!(out, "thinking_enabled must be respected for unknown models");
}

#[test]
fn thinking_enabled_false_stays_false() {
    let out = resolve_thinking_flag(false, "some-model");
    assert!(!out);
}
