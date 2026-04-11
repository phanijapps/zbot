//! End-to-end integration tests for the ward execution pipeline.
//!
//! Tests the infrastructure that keeps breaking:
//! 1. Ward scaffolding scoped to recommended skills
//! 2. ralph.py task runner lifecycle
//! 3. Subagent context construction (lean, not bloated)
//! 4. Crash report includes ralph.py status
//! 5. Callback structured result detection
//! 6. Intent injection SDLC pattern
//! 7. Loop detector doesn't kill productive agents

use std::path::Path;
use tempfile::TempDir;

// ============================================================================
// 1. WARD SCAFFOLDING — SCOPED TO RECOMMENDED SKILLS
// ============================================================================

/// Scaffolding should only create directories from RECOMMENDED skills,
/// not all skills on disk. Bug: life-os dirs appeared in financial-analysis ward.
#[test]
fn test_scaffolding_scoped_to_recommended_skills() {
    let dir = TempDir::new().unwrap();
    let skills_dir = dir.path().join("skills");

    // Create coding skill with ward_setup
    let coding_dir = skills_dir.join("coding");
    std::fs::create_dir_all(&coding_dir).unwrap();
    std::fs::write(
        coding_dir.join("SKILL.md"),
        r#"---
name: coding
description: Code stuff
ward_setup:
  directories:
    - core/
    - output/
    - specs/
---
Instructions here
"#,
    )
    .unwrap();

    // Create life-os skill with ward_setup (should NOT apply to coding wards)
    let lifeos_dir = skills_dir.join("life-os");
    std::fs::create_dir_all(&lifeos_dir).unwrap();
    std::fs::write(
        lifeos_dir.join("SKILL.md"),
        r#"---
name: life-os
description: Life stuff
ward_setup:
  directories:
    - daily/
    - weekly/
    - projects/
    - areas/
---
Instructions here
"#,
    )
    .unwrap();

    // Scaffold a ward with ONLY coding skill recommended
    let ward_dir = dir.path().join("wards").join("financial-analysis");
    std::fs::create_dir_all(&ward_dir).unwrap();

    // Simulate scoped scaffolding (only from coding skill)
    let setups = gateway_execution::invoke::stream::collect_ward_setups_for_skills(
        &skills_dir,
        &["coding".to_string()],
    );
    gateway_execution::middleware::ward_scaffold::scaffold_ward(
        &ward_dir,
        "financial-analysis",
        &setups,
    );

    // Coding dirs should exist
    assert!(ward_dir.join("core").is_dir(), "core/ should be created");
    assert!(
        ward_dir.join("output").is_dir(),
        "output/ should be created"
    );
    assert!(ward_dir.join("specs").is_dir(), "specs/ should be created");

    // Life-os dirs should NOT exist
    assert!(
        !ward_dir.join("daily").exists(),
        "daily/ should NOT be created — wrong skill"
    );
    assert!(
        !ward_dir.join("weekly").exists(),
        "weekly/ should NOT be created — wrong skill"
    );
    assert!(
        !ward_dir.join("projects").exists(),
        "projects/ should NOT be created — wrong skill"
    );
    assert!(
        !ward_dir.join("areas").exists(),
        "areas/ should NOT be created — wrong skill"
    );
}

/// Scaffolding with life-os skill should create life-os dirs, not coding dirs.
#[test]
fn test_scaffolding_lifeos_skill_creates_lifeos_dirs() {
    let dir = TempDir::new().unwrap();
    let skills_dir = dir.path().join("skills");

    let lifeos_dir = skills_dir.join("life-os");
    std::fs::create_dir_all(&lifeos_dir).unwrap();
    std::fs::write(
        lifeos_dir.join("SKILL.md"),
        r#"---
name: life-os
description: Life stuff
ward_setup:
  directories:
    - daily/
    - weekly/
    - projects/
---
Instructions here
"#,
    )
    .unwrap();

    let ward_dir = dir.path().join("wards").join("personal-life");
    std::fs::create_dir_all(&ward_dir).unwrap();

    let setups = gateway_execution::invoke::stream::collect_ward_setups_for_skills(
        &skills_dir,
        &["life-os".to_string()],
    );
    gateway_execution::middleware::ward_scaffold::scaffold_ward(
        &ward_dir,
        "personal-life",
        &setups,
    );

    assert!(ward_dir.join("daily").is_dir());
    assert!(ward_dir.join("weekly").is_dir());
    assert!(ward_dir.join("projects").is_dir());
    assert!(
        !ward_dir.join("core").exists(),
        "core/ should NOT exist — coding skill not recommended"
    );
}

// ============================================================================
// 2. RALPH.PY TASK RUNNER
// ============================================================================

/// ralph.py should process tasks in dependency order.
#[test]
fn test_ralph_next_respects_dependencies() {
    let dir = TempDir::new().unwrap();

    // Copy ralph.py from shared ward
    let ralph_src = Path::new("/home/videogamer/Documents/zbot/wards/shared/ralph.py");
    if !ralph_src.exists() {
        eprintln!("Skipping ralph.py test — shared/ralph.py not found");
        return;
    }
    let ralph_dst = dir.path().join("ralph.py");
    std::fs::copy(ralph_src, &ralph_dst).unwrap();

    // Create tasks.json with dependencies
    let tasks_json = dir.path().join("tasks.json");
    std::fs::write(&tasks_json, r#"{
        "tasks": [
            {"id": 1, "action": "create", "file": "core/a.py", "description": "Module A", "acceptance": "importable", "depends_on": [], "status": "pending"},
            {"id": 2, "action": "create", "file": "core/b.py", "description": "Module B", "acceptance": "importable", "depends_on": [1], "status": "pending"},
            {"id": 3, "action": "run", "command": "python3 test.py", "description": "Run test", "acceptance": "exit 0", "depends_on": [1, 2], "status": "pending"}
        ]
    }"#).unwrap();

    // Next should return task 1 (no deps)
    let output = std::process::Command::new("python3")
        .args(["ralph.py", "next", "tasks.json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"id\": 1"),
        "First task should be id 1, got: {}",
        stdout
    );

    // Complete task 1
    let output = std::process::Command::new("python3")
        .args(["ralph.py", "complete", "tasks.json", "1"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("marked complete"));

    // Next should return task 2 (dep on 1 satisfied)
    let output = std::process::Command::new("python3")
        .args(["ralph.py", "next", "tasks.json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"id\": 2"),
        "Second task should be id 2, got: {}",
        stdout
    );

    // Complete task 2
    std::process::Command::new("python3")
        .args(["ralph.py", "complete", "tasks.json", "2"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Next should return task 3 (deps 1,2 satisfied)
    let output = std::process::Command::new("python3")
        .args(["ralph.py", "next", "tasks.json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"id\": 3"),
        "Third task should be id 3, got: {}",
        stdout
    );

    // Complete task 3
    std::process::Command::new("python3")
        .args(["ralph.py", "complete", "tasks.json", "3"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Next should return done
    let output = std::process::Command::new("python3")
        .args(["ralph.py", "next", "tasks.json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"done\": true"),
        "Should be done, got: {}",
        stdout
    );
}

/// Failed task should block dependents.
#[test]
fn test_ralph_fail_blocks_dependents() {
    let dir = TempDir::new().unwrap();

    let ralph_src = Path::new("/home/videogamer/Documents/zbot/wards/shared/ralph.py");
    if !ralph_src.exists() {
        return;
    }
    std::fs::copy(ralph_src, dir.path().join("ralph.py")).unwrap();

    std::fs::write(dir.path().join("tasks.json"), r#"{
        "tasks": [
            {"id": 1, "action": "create", "file": "a.py", "description": "A", "acceptance": "ok", "depends_on": [], "status": "pending"},
            {"id": 2, "action": "run", "command": "python3 a.py", "description": "Run A", "acceptance": "exit 0", "depends_on": [1], "status": "pending"}
        ]
    }"#).unwrap();

    // Fail task 1
    std::process::Command::new("python3")
        .args(["ralph.py", "fail", "tasks.json", "1", "syntax error"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Next should report blocked
    let output = std::process::Command::new("python3")
        .args(["ralph.py", "next", "tasks.json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("blocked_by_failures"),
        "Should be blocked, got: {}",
        stdout
    );

    // Status should show 1 failed, 1 pending
    let output = std::process::Command::new("python3")
        .args(["ralph.py", "status", "tasks.json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Failed: 1"),
        "Should show 1 failed, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Pending: 1"),
        "Should show 1 pending, got: {}",
        stdout
    );
}

// ============================================================================
// 3. SUBAGENT CONTEXT — LEAN, NOT BLOATED
// ============================================================================

/// Executor subagent rules should be under 300 bytes.
#[test]
fn test_subagent_rules_are_lean() {
    let rules = gateway_execution::invoke::setup::subagent_rules(
        gateway_execution::invoke::setup::SubagentRole::Executor,
    );
    let byte_count = rules.len();
    assert!(
        byte_count < 300,
        "Executor rules should be under 300 bytes, got {} bytes:\n{}",
        byte_count,
        rules
    );
}

/// Reviewer rules should include RESULT format.
#[test]
fn test_reviewer_rules_include_result_format() {
    let rules = gateway_execution::invoke::setup::subagent_rules(
        gateway_execution::invoke::setup::SubagentRole::Reviewer,
    );
    assert!(
        rules.contains("RESULT: APPROVED"),
        "Reviewer rules must mention RESULT: APPROVED"
    );
    assert!(
        rules.contains("RESULT: DEFECTS"),
        "Reviewer rules must mention RESULT: DEFECTS"
    );
}

/// Role detection should identify review tasks.
#[test]
fn test_role_detection() {
    use gateway_execution::invoke::setup::{detect_subagent_role, SubagentRole};

    assert_eq!(
        detect_subagent_role("code-agent", "Build the data pipeline"),
        SubagentRole::Executor
    );
    assert_eq!(
        detect_subagent_role("code-agent", "Review code against specs"),
        SubagentRole::Reviewer
    );
    assert_eq!(
        detect_subagent_role("data-analyst", "Validate output quality"),
        SubagentRole::Reviewer
    );
    assert_eq!(
        detect_subagent_role("data-analyst", "Run the analysis script"),
        SubagentRole::Executor
    );
    assert_eq!(
        detect_subagent_role("code-agent", "Evaluate the implementation"),
        SubagentRole::Reviewer
    );
}

// ============================================================================
// 4. CALLBACK STRUCTURED RESULT DETECTION
// ============================================================================

/// Callback should detect APPROVED and add action hint.
#[test]
fn test_callback_detects_approved() {
    let msg = gateway_execution::delegation::format_callback_message(
        "code-agent",
        "Code looks good. All tests pass.\n\nRESULT: APPROVED",
        "conv-123",
    );
    assert!(msg.contains("APPROVED"), "Should contain APPROVED");
    assert!(
        msg.contains("Proceed to the next node"),
        "Should suggest proceeding"
    );
}

/// Callback should detect DEFECTS and include defect list.
#[test]
fn test_callback_detects_defects() {
    let msg = gateway_execution::delegation::format_callback_message(
        "data-analyst",
        "Found issues.\n\nRESULT: DEFECTS\n- output.json: RSI value is -5 (severity: high)\n- data.csv: Only 10 rows (severity: medium)",
        "conv-123",
    );
    assert!(msg.contains("DEFECTS found"), "Should mention DEFECTS");
    assert!(
        msg.contains("RSI value is -5"),
        "Should include defect details"
    );
    assert!(
        msg.contains("Re-delegate to coding agent"),
        "Should suggest re-delegation"
    );
}

/// Callback without RESULT marker should not add action hints.
#[test]
fn test_callback_without_result_no_action() {
    let msg = gateway_execution::delegation::format_callback_message(
        "code-agent",
        "Here is the analysis of the data.\nIt shows interesting patterns.",
        "conv-123",
    );
    assert!(
        !msg.contains("Action:"),
        "Should not contain Action hint for plain responses"
    );
}

// ============================================================================
// 5. INTENT INJECTION — SDLC PATTERN
// ============================================================================

/// Graph approach should inject SDLC pattern.
#[test]
fn test_intent_injection_sdlc_for_graph() {
    use gateway_execution::middleware::intent_analysis::*;

    let analysis = IntentAnalysis {
        primary_intent: "stock analysis".to_string(),
        hidden_intents: vec!["fetch options data".to_string()],
        recommended_skills: vec!["coding".to_string()],
        recommended_agents: vec!["code-agent".to_string()],
        ward_recommendation: WardRecommendation {
            action: "create_new".to_string(),
            ward_name: "financial-analysis".to_string(),
            subdirectory: Some("stocks/amd".to_string()),
            structure: Default::default(),
            reason: "domain match".to_string(),
        },
        execution_strategy: ExecutionStrategy {
            approach: "graph".to_string(),
            graph: None,
            explanation: "Complex analysis".to_string(),
        },
        rewritten_prompt: String::new(),
    };

    let injection = format_intent_injection(&analysis, None, None);

    // Graph approach should route to planner-agent
    assert!(
        injection.contains("## Task Analysis"),
        "Graph approach should include task analysis"
    );
    assert!(injection.contains("Goal:"), "Should include the goal");
    assert!(
        injection.contains("planner-agent"),
        "Should route to planner for graph tasks"
    );
    assert!(
        injection.contains("Ward Rule:"),
        "Should include ward discipline"
    );
}

/// Simple approach should NOT inject SDLC pattern.
#[test]
fn test_intent_injection_no_sdlc_for_simple() {
    use gateway_execution::middleware::intent_analysis::*;

    let analysis = IntentAnalysis {
        primary_intent: "greeting".to_string(),
        hidden_intents: vec![],
        recommended_skills: vec![],
        recommended_agents: vec![],
        ward_recommendation: WardRecommendation {
            action: "use_existing".to_string(),
            ward_name: "scratch".to_string(),
            subdirectory: None,
            structure: Default::default(),
            reason: "simple".to_string(),
        },
        execution_strategy: ExecutionStrategy {
            approach: "simple".to_string(),
            graph: None,
            explanation: "Quick question".to_string(),
        },
        rewritten_prompt: String::new(),
    };

    let injection = format_intent_injection(&analysis, None, None);

    assert!(
        !injection.contains("SDLC Pattern"),
        "Simple approach should NOT include SDLC"
    );
    assert!(
        !injection.contains("tasks.json"),
        "Simple approach should NOT mention tasks.json"
    );
}

/// Ward rules should not have hardcoded domain examples.
#[test]
fn test_ward_rules_domain_agnostic() {
    use gateway_execution::middleware::intent_analysis::*;

    let analysis = IntentAnalysis {
        primary_intent: "test".to_string(),
        hidden_intents: vec![],
        recommended_skills: vec![],
        recommended_agents: vec![],
        ward_recommendation: WardRecommendation {
            action: "create_new".to_string(),
            ward_name: "test".to_string(),
            subdirectory: None,
            structure: Default::default(),
            reason: "test".to_string(),
        },
        execution_strategy: ExecutionStrategy {
            approach: "simple".to_string(),
            graph: None,
            explanation: "test".to_string(),
        },
        rewritten_prompt: String::new(),
    };

    let injection = format_intent_injection(&analysis, None, None);

    // Should NOT have financial domain terms
    assert!(
        !injection.contains("SPY"),
        "Ward rules should not mention SPY"
    );
    assert!(
        !injection.contains("ohlcv"),
        "Ward rules should not mention ohlcv"
    );
    assert!(
        !injection.contains("RSI"),
        "Ward rules should not mention RSI"
    );
    assert!(
        !injection.contains("yfinance"),
        "Ward rules should not mention yfinance"
    );
}

// ============================================================================
// 7. AUTO_UPDATE_AGENTS_MD
// ============================================================================

/// AGENTS.md should include ralph.py task runner section when ralph.py exists.
#[test]
fn test_agents_md_includes_ralph_when_present() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path();
    let ward_dir = vault_dir.join("wards").join("test-ward");
    std::fs::create_dir_all(&ward_dir).unwrap();

    // Create ralph.py
    std::fs::write(
        ward_dir.join("ralph.py"),
        "#!/usr/bin/env python3\nprint('ralph')",
    )
    .unwrap();

    // Create minimal AGENTS.md
    std::fs::write(
        ward_dir.join("AGENTS.md"),
        "# test-ward\n\n## Purpose\nTest\n",
    )
    .unwrap();

    // Create lang configs dir
    let lang_dir = vault_dir.join("config").join("wards");
    std::fs::create_dir_all(&lang_dir).unwrap();

    // Run auto-update
    gateway_execution::runner::auto_update_agents_md_with_lang_configs(
        vault_dir,
        "test-ward",
        &lang_dir,
    );

    let content = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
    assert!(
        content.contains("Task Runner"),
        "AGENTS.md should have Task Runner section when ralph.py exists"
    );
    assert!(content.contains("ralph.py"), "Should reference ralph.py");
}

/// AGENTS.md should NOT include ralph.py section when it doesn't exist.
#[test]
fn test_agents_md_no_ralph_when_absent() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path();
    let ward_dir = vault_dir.join("wards").join("test-ward");
    std::fs::create_dir_all(&ward_dir).unwrap();

    std::fs::write(
        ward_dir.join("AGENTS.md"),
        "# test-ward\n\n## Purpose\nTest\n",
    )
    .unwrap();

    let lang_dir = vault_dir.join("config").join("wards");
    std::fs::create_dir_all(&lang_dir).unwrap();

    gateway_execution::runner::auto_update_agents_md_with_lang_configs(
        vault_dir,
        "test-ward",
        &lang_dir,
    );

    let content = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
    assert!(
        !content.contains("Task Runner"),
        "AGENTS.md should NOT have Task Runner section without ralph.py"
    );
}

/// AGENTS.md should reference memory-bank/, not memory/.
#[test]
fn test_agents_md_uses_memory_bank_not_memory() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path();
    let ward_dir = vault_dir.join("wards").join("test-ward");
    std::fs::create_dir_all(&ward_dir).unwrap();
    std::fs::create_dir_all(ward_dir.join("memory-bank")).unwrap();
    std::fs::write(ward_dir.join("memory-bank").join("ward.md"), "# Knowledge").unwrap();

    std::fs::write(
        ward_dir.join("AGENTS.md"),
        "# test-ward\n\n## Purpose\nTest\n",
    )
    .unwrap();

    let lang_dir = vault_dir.join("config").join("wards");
    std::fs::create_dir_all(&lang_dir).unwrap();

    gateway_execution::runner::auto_update_agents_md_with_lang_configs(
        vault_dir,
        "test-ward",
        &lang_dir,
    );

    let content = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
    assert!(
        content.contains("memory-bank/"),
        "Should reference memory-bank/"
    );
    assert!(
        !content.contains("memory/ward.md"),
        "Should NOT reference old memory/ path"
    );
}
