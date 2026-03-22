# Apply Patch — Definitive Fix

## Problem

LLMs consistently fail to format apply_patch correctly:
1. **Missing `+` prefixes** in Add File blocks — `import yfinance` instead of `+import yfinance`
2. **Missing `*** Begin Patch`** — LLM puts heredoc and patch marker on wrong lines
3. **Opaque error messages** — `"invalid hunk header"` doesn't tell the LLM what to fix
4. **Agent spirals** — after 3-4 format failures, agent falls back to blocked `@"..."@` heredocs, spiraling until max iterations

Evidence: In sess-aaea1005, data-analyst tried apply_patch 4 times with format errors, then attempted `@"..."@` heredocs 8 times (all blocked), burning 47 of 49 iterations on file-writing failures.

## Root Cause Analysis

Our parser at line 136 of `apply_patch.rs`:
```rust
if let Some(added) = line.strip_prefix('+') {
    contents.push_str(added);
} else {
    break; // stops at first line without +
}
```

When the LLM writes `import yfinance` without `+`, the parser stops reading, produces an empty file, and the next line without `+` triggers `"invalid hunk header"` — a misleading error that sends the LLM into confusion.

OpenAI's reference implementation (`codex-rs/apply-patch/src/parser.rs`) has the same strict mode but includes a **lenient mode** that handles common LLM formatting errors. Their constant `PARSE_IN_STRICT_MODE = false` enables lenient parsing for all models.

## Design

### 1. Lenient Add File parsing

In the Add File parsing branch, accept lines without `+` prefix when they are clearly content lines (not patch markers):

```rust
// Current (strict):
if let Some(added) = line.strip_prefix('+') {
    contents.push_str(added);
} else {
    break;
}

// New (lenient):
if let Some(added) = line.strip_prefix('+') {
    contents.push_str(added);
} else if !is_patch_marker(line) {
    // Lenient: auto-fix missing + prefix
    contents.push_str(line);
} else {
    break;
}
```

Where `is_patch_marker` checks for `*** `, `@@`, `EOF` — lines that are clearly not file content.

### 2. Lenient boundary checking

If `*** Begin Patch` is not the first line:
- Check if any line in the first 3 lines contains `*** Begin Patch` — if so, skip preamble
- If the patch starts with content that looks like a heredoc (`<<'EOF'`), unwrap it (already implemented)
- If neither, give a clear error: `"Patch must start with '*** Begin Patch'. Got: '{first_line}'"`

### 3. Actionable error messages

Every error should tell the LLM exactly how to fix it:

| Current | New |
|---|---|
| `"invalid hunk at line 3: 'import yfinance' is not a valid hunk header"` | `"Add File lines must start with '+'. Example: '+import yfinance as yf'. Got bare line at position 3."` |
| `"First line must be '*** Begin Patch'"` | `"Patch format: apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: path\n+content\n*** End Patch\nEOF"` |
| `"invalid patch: ..."` (generic) | Include the exact template in every error so the LLM can self-correct |

### 4. Tooling shard — use OpenAI's official instructions

Replace our minimal apply_patch documentation with OpenAI's official `apply_patch_tool_instructions.md` content (adapted for our tool). This is battle-tested with GPT-4.1.

### 5. Coding skill — reinforce apply_patch examples

The coding skill shows apply_patch examples but they're minimal. Add a complete multi-line file creation example with proper `+` prefixes.

## File Changes

| File | Change |
|---|---|
| `runtime/agent-tools/src/tools/execution/apply_patch.rs` | Lenient Add File parsing, lenient boundary check, actionable error messages |
| `gateway/templates/shards/tooling_skills.md` | Replace apply_patch docs with OpenAI's official format |
| `~/Documents/zbot/skills/coding/SKILL.md` | Reinforce apply_patch examples |

## Testing

| Test | What it verifies |
|---|---|
| Add File with `+` prefixes | Standard format works (existing test) |
| Add File WITHOUT `+` prefixes | Lenient parsing auto-fixes (new test) |
| Add File with MIXED `+` and bare lines | Handles inconsistent formatting (new test) |
| Patch without `*** Begin Patch` on first line | Lenient boundary finds it on line 2-3 (new test) |
| Heredoc-wrapped patch | Already tested, verify still works |
| Update File with context | Unchanged behavior (existing test) |
| Error message content | Verify errors include fix examples (new test) |
| Ward root / variant enforcement | Still works with lenient parsing (existing test) |
