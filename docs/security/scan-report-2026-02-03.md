# AgentZero Security Scan Report

**Date:** 2026-02-03
**Scanner:** Claude Code + cargo-audit + cargo-deny + npm audit
**Scope:** Full Rust backend + TypeScript/React frontend

---

## Executive Summary

| Category | Count | Status |
|----------|-------|--------|
| Critical Vulnerabilities | 1 | 🔴 Action Required |
| High Severity Issues | 8 | 🔴 Action Required |
| Medium Severity Issues | 6 | 🟡 Monitor |
| Low/Info Issues | 3 | 🔵 Track |
| Frontend Vulnerabilities | 0 | ✅ Clear |

**Overall Security Grade: C+**

---

## 1. CRITICAL VULNERABILITIES

### 🔴 RUSTSEC-2026-0007: Integer Overflow in `bytes` crate

**CVE ID:** GHSA-434x-w66g-qw3r
**Affected Package:** `bytes` v1.11.0
**Category:** Memory Corruption
**CVSS:** N/A (Potential for RCE)

**Description:**
In `BytesMut::reserve`, unchecked addition can cause `usize` overflow in release builds, leading to corrupted capacity values. Subsequent operations may create out-of-bounds slices causing undefined behavior.

**PoC:**
```rust
use bytes::BytesMut;

fn main() {
    let mut b = BytesMut::from(&b"hello world"[..]);
    let mut b2 = b.split_off(5);
    drop(b);  // Make b2 unique owner

    b2.reserve(usize::MAX - 6);  // Trigger overflow
    b2.put_u8(b'h');  // Potential UB/HBO
}
```

**Impact:** Memory corruption, potential arbitrary code execution

**Affected Components:**
- HTTP client/server (via `reqwest`, `axum`)
- WebSocket handling (via `tokio-tungstenite`)

**Remediation:**
```toml
# Update workspace dependencies
[workspace.dependencies]
bytes = { version = "1.11.1" }  # or latest
```

**Action:** Update to `bytes >= 1.11.1`

---

## 2. HIGH SEVERITY ISSUES

### 🔴 HIGH: No Authentication/Authorization

**Files Affected:**
- `gateway/src/http/mod.rs:33-41`
- `gateway/src/http/agents.rs` (all endpoints)
- `gateway/src/http/providers.rs` (all endpoints)

**Issue:** All API endpoints are publicly accessible without any authentication

**Evidence:**
```rust
// gateway/src/http/mod.rs
pub fn create_http_router(config: GatewayConfig, state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)  // ⚠️ Allows ANY origin
        .allow_methods(Any)
        .allow_headers(Any);
    // ... no auth middleware
}

// gateway/src/http/providers.rs:38-43
pub async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    match state.provider_service.list() {
        Ok(providers) => Json(providers).into_response(),  // ⚠️ Returns API keys!
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}
```

**Exposed Endpoints (all unauthenticated):**
```
GET    /api/agents           - List all agents
POST   /api/agents           - Create agent
PUT    /api/agents/:id       - Modify agent
DELETE /api/agents/:id       - Delete agent

GET    /api/providers        - ⚠️ EXPOSES LLM API KEYS
POST   /api/providers        - Create provider (with API key)
PUT    /api/providers/:id    - Modify provider (with API key)

GET    /api/conversations    - Access all conversations
POST   /api/gateway/submit   - Execute agent tasks

GET    /api/mcps             - Access MCP configs
GET    /api/connectors       - Access connector configs
```

**Impact:**
- Unauthorized access to all agent conversations
- Ability to create/modify/delete agents
- **Direct access to LLM API keys** stored in provider configs
- Ability to execute arbitrary agent tasks
- Access to MCP server configurations

**Recommendations:**
1. Implement JWT-based authentication
2. Add API key middleware for HTTP API
3. Implement role-based access control (RBAC)
4. Add session management
5. Add audit logging for all operations

---

### 🔴 HIGH: No TLS/HTTPS Support

**Files Affected:**
- `gateway/src/server.rs:106-125`
- `apps/cli/src/main.rs:95-96`

**Issue:** All traffic is unencrypted HTTP and WebSocket

**Evidence:**
```rust
// apps/cli/src/main.rs:95-96
let gateway_url = format!("http://{}:{}", cli.host, cli.port);
let ws_url = format!("ws://{}:{}", cli.host, cli.port - 1);
```

**Impact:**
- Man-in-the-middle attacks
- Credential interception (LLM API keys in transit)
- Conversation eavesdropping
- Session hijacking

**Recommendations:**
1. Add TLS support with `rustls` or `native-tls`
2. Make HTTPS mandatory in production
3. Implement certificate validation
4. Support WSS (WebSocket Secure)

---

### 🔴 HIGH: API Keys Stored in Plaintext

**Files Affected:**
- `gateway/src/services/providers.rs:65-77`
- `framework/zero-llm/src/config.rs:11`

**Issue:** LLM API keys stored in plaintext YAML files

**Evidence:**
```rust
// framework/zero-llm/src/config.rs:11
pub struct ProviderConfig {
    pub api_key: String,  // ⚠️ Stored in plaintext
    pub base_url: Option<String>,
    pub model: String,
    // ...
}
```

**Storage Location:**
```
~/Documents/agentzero/providers.yaml
~/Documents/agentzero/agents/*/config.yaml
```

**Impact:**
- Any system user can access LLM credentials
- Backups contain unencrypted API keys
- No audit trail of key access

**Recommendations:**
1. Implement encryption at rest (AES-256-GCM)
2. Integrate with OS keyring (Windows Credential Manager, macOS Keychain, libsecret)
3. Consider HashiCorp Vault for enterprise deployments
4. Add key rotation support

---

### 🔴 HIGH: No Rate Limiting

**Issue:** No protection against DoS attacks or brute force

**Impact:**
- Resource exhaustion via rapid API requests
- Brute force attacks on future authentication
- Potential service disruption

**Recommendations:**
1. Add `tower-governor` for rate limiting
2. Implement per-IP rate limits
3. Add per-user rate limits after auth
4. Consider burst capacity configuration

---

### 🟠 HIGH: Unmaintained Dependencies (2)

#### RUSTSEC-2024-0384: `instant` crate

**Package:** `instant` v0.1.13
**Issue:** No longer maintained
**Replacement:** `web-time`

**Dependency Chain:**
```
instant 0.1.13
  └── measure_time 0.8.3
       └── tantivy 0.22.1 (full-text search)
            └── search-index (AgentZero service)
```

#### RUSTSEC-2024-0436: `paste` crate

**Package:** `paste` v1.0.15
**Issue:** Repository archived
**Replacement:** `pastey` or `with_builtin_macros`

**Dependency Chain:**
```
paste 1.0.15
  ├── parquet 53.4.1
  │    └── session-archive (AgentZero service)
  ├── ratatui 0.29.0
  │    └── cli (AgentZero app)
  └── rmcp 0.2.1
       └── zero-mcp (AgentZero framework)
```

**Action Required:** Monitor for upstream updates or fork replacements

---

### 🟠 HIGH: Unsound Code in `lru` crate

**RUSTSEC-2026-0002:** `lru` v0.12.5

**Issue:** `IterMut` violates Stacked Borrows by invalidating internal `HashMap` pointer

**Patched Version:** >=0.16.3

**Recommendation:** Update to latest version

---

## 3. MEDIUM SEVERITY ISSUES

### 🟡 MEDIUM: No Security Headers

**File:** `gateway/src/http/mod.rs:154-157`

**Missing Headers:**
```
Content-Security-Policy: default-src 'self'
Strict-Transport-Security: max-age=31536000; includeSubDomains
X-Content-Type-Options: nosniff
X-Frame-Options: DENY
X-XSS-Protection: 1; mode=block
Referrer-Policy: strict-origin-when-cross-origin
```

**Recommendation:** Add using `tower-http` middleware

---

### 🟡 MEDIUM: Excessive `unwrap()` Usage

**Count:** 25+ instances

**Files Affected:**
- `framework/zero-agent/src/orchestrator/mod.rs`
- `framework/zero-agent/src/orchestrator/task_graph.rs`
- `framework/zero-agent/src/workflow/conditional_agent.rs`

**Example:**
```rust
// framework/zero-agent/src/orchestrator/mod.rs:395
let mut store = self.agent_store.write().unwrap();  // Could panic
```

**Impact:** Potential DoS via panic triggers

**Recommendation:** Replace with proper error handling (`?`, `.expect()` with context)

---

### 🟡 MEDIUM: Insufficient Path Validation

**Files Affected:**
- `gateway/src/services/agents.rs:125`
- `gateway/src/services/providers.rs:65-77`

**Issue:** No explicit path traversal validation before file operations

**Example:**
```rust
// gateway/src/services/agents.rs:125
let entries = fs::read_dir(&self.agents_dir)  // No validation
```

**Recommendation:** Add path sanitization and validation

---

### 🟡 MEDIUM: No Database Encryption

**File:** `gateway/src/database/connection.rs:32`

**Issue:** SQLite database stored unencrypted

**Impact:**
- Sensitive conversation content accessible
- No at-rest encryption for session data

**Recommendation:** Use `sqlx-sqlite` with encryption or external encryption layer

---

### 🟡 MEDIUM: No Audit Logging

**Missing:**
- Security event logging
- Authentication attempts (when implemented)
- Authorization failures
- Configuration changes
- Agent execution auditing

**Recommendation:** Add structured logging with `tracing` and file appender

---

### 🟡 MEDIUM: CORS Allows Any Origin

**File:** `gateway/src/http/mod.rs:35-38`

```rust
CorsLayer::new()
    .allow_origin(Any)  // ⚠️ Allows ANY origin
    .allow_methods(Any)
    .allow_headers(Any)
```

**Recommendation:** Restrict to specific origins in production

---

## 4. CODE QUALITY FINDINGS

### ✅ POSITIVE: Minimal Unsafe Code

**Found 2 locations** (both justified):

1. **`runtime/agent-tools/src/tools/execution/shell.rs:172`**
   ```rust
   if unsafe { libc::getuid() } == 0 {  // Check for root
   ```
   ✅ Required for Unix UID check, no safer alternative

2. **`runtime/agent-tools/src/tools/execution/shell.rs:528`**
   ```rust
   unsafe {
       let mut token: HANDLE = ptr::null_mut();
       // Windows admin check
   ```
   ✅ Required for Windows Token elevation check

---

### ✅ POSITIVE: SQL Injection Protection

**File:** `gateway/src/database/schema.rs`

All database queries use parameterized statements:
```rust
conn.execute(
    "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
    [SCHEMA_VERSION],  // ✅ Parameterized
)?;
```

**Status:** No SQL injection risk detected

---

### ✅ POSITIVE: Shell Command Guardrails

**File:** `runtime/agent-tools/src/tools/execution/shell.rs:20-119`

**Existing Protections:**
- 90+ blocked dangerous commands (rm -rf, mkfs, dd, etc.)
- Suspicious pattern warnings
- Disabled when running as root/admin
- Output size limits (1MB)
- Timeout enforcement (max 10 min)
- Path traversal checks on cwd

**Bypasses Still Possible:**
- Command chaining with `;`, `&&`, `||`
- Variable expansion tricks
- Unicode homograph attacks

**Risk Level:** Medium (good guardrails but bypasses exist)

---

## 5. FRONTEND SECURITY

### ✅ EXCELLENT: No XSS Vulnerabilities Found

**Scan Results:**
- No `innerHTML` usage
- No `dangerouslySetInnerHTML` usage
- No `eval()` usage
- No `Function()` constructor usage

**Status:** React's built-in XSS protection is working correctly

---

### ✅ EXCELLENT: No NPM Vulnerabilities

```json
{
  "metadata": {
    "vulnerabilities": {
      "info": 0,
      "low": 0,
      "moderate": 0,
      "high": 0,
      "critical": 0,
      "total": 0
    }
  }
}
```

**571 dependencies scanned** - 0 vulnerabilities found

---

## 6. COMMAND INJECTION ANALYSIS

### Command Execution Locations

**Found 8 locations:**

| Location | Line | Function | Risk |
|----------|------|----------|------|
| `gateway/src/connectors/dispatch.rs:225` | 225 | Connector dispatch | Medium |
| `runtime/agent-runtime/src/mcp/stdio.rs:65` | 65 | MCP stdio handler | Medium |
| `runtime/agent-tools/src/tools/execution/shell.rs:386` | 386 | Shell tool | Medium (has guardrails) |

**Sample:**
```rust
// runtime/agent-tools/src/tools/execution/shell.rs:386-392
let mut cmd = Command::new(&shell);  // shell is controlled
for arg in &shell_args {
    cmd.arg(arg);
}
cmd.arg(command);  // ⚠️ User input passed to shell
```

**Mitigation:** Shell tool has guardrails (90+ blocked commands)

---

## 7. ERROR HANDLING ANALYSIS

### Information Leakage Concerns

**Files with potentially verbose error messages:**
- `gateway/src/http/agents.rs` - Returns error messages directly
- `gateway/src/http/providers.rs` - Returns error messages directly

**Example:**
```rust
// gateway/src/http/providers.rs:41-42
match state.provider_service.get(&id) {
    Ok(provider) => Json(provider).into_response(),
    Err(e) => (StatusCode::NOT_FOUND, e).into_response(),  // ⚠️ May leak info
}
```

**Recommendation:** Sanitize error messages for production

---

## 8. WEBSOCKET SECURITY

### Connection Handler Analysis

**File:** `gateway/src/websocket/handler.rs`

**Findings:**
- ✅ Session ID generation is random
- ✅ Session cleanup implemented
- ⚠️ No authentication required
- ⚠️ No rate limiting on subscriptions
- ⚠️ No origin validation

**Subscription Limits:**
```rust
// Some limits exist in subscriptions module
Err(SubscribeError::TooManySubscriptions { limit }) => { ... }
Err(SubscribeError::ConversationFull { limit }) => { ... }
```

---

## 9. SUMMARY STATISTICS

| Metric | Value |
|--------|-------|
| **Rust Dependencies** | 480 |
| **Node.js Dependencies** | 571 |
| **Rust Files Scanned** | 100+ |
| **TypeScript Files** | 50+ |
| **Unsafe Blocks** | 2 (both justified) |
| **API Endpoints** | 40+ (all unauthenticated) |
| **Lines of Code** | ~50,000+ |

---

## 10. REMEDIATION PRIORITY MATRIX

| Priority | Issue | Effort | Impact |
|----------|-------|--------|--------|
| **P0** | Update bytes crate | Low | High |
| **P0** | Update lru crate | Low | High |
| **P0** | Add authentication | High | Critical |
| **P1** | Remove unmaintained deps | Medium | Medium |
| **P1** | Add TLS/HTTPS | Medium | Critical |
| **P1** | Add rate limiting | Medium | High |
| **P1** | Implement secrets encryption | Medium | High |
| **P2** | Add security headers | Low | Medium |
| **P2** | Add audit logging | Medium | Medium |
| **P2** | Database encryption | Medium | High |

---

## 11. COMPLIANCE ASSESSMENT

| Standard | Status | Notes |
|----------|--------|-------|
| **OWASP Top 10** | Partial | No auth, no encryption |
| **SOC2** | Not Compliant | No audit logging, no access controls |
| **ISO 27001** | Not Compliant | No ISMS, no controls |
| **GDPR** | Partial | Local-first is good, but no encryption |

---

## 12. SCAN TOOLS USED

| Tool | Version | Purpose |
|------|---------|---------|
| `cargo audit` | 0.22.0 | Rust vulnerability scanning |
| `cargo deny` | 0.19.0 | License + bans + advisories |
| `npm audit` | Built-in | Node.js vulnerabilities |
| `Gitleaks` | Action | Secrets scanning (in CI) |
| Custom grep patterns | N/A | Code pattern analysis |

---

## 13. RECOMMENDATIONS SUMMARY

### Immediate (P0)
1. Update `bytes` crate to >=1.11.1
2. Update `lru` crate to >=0.16.3
3. Implement authentication middleware

### Short-term (P1)
4. Add TLS/HTTPS support
5. Implement rate limiting
6. Encrypt API keys at rest
7. Replace unmaintained dependencies

### Medium-term (P2)
8. Add security headers
9. Implement audit logging
10. Encrypt database at rest

### Long-term
11. Implement RBAC
12. Add penetration testing
13. Compliance certification (SOC2, ISO 27001)

---

**Report Generated:** 2026-02-03
**Next Review:** After P0/P1 remediations
**Report Version:** 1.0
