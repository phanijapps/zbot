# Hermes Deltas — Where z-Bot Lags

Unbiased assessment of capabilities Hermes ships today that z-Bot does not. Organized by impact, with concrete deltas and what it would take to close each gap.

> Historical snapshot. For the current code-backed impact assessment, see
> [`impact-analysis-2026-05-30.md`](./impact-analysis-2026-05-30.md).

---

## P0 — Capabilities Hermes Has That z-Bot Doesn't

### 1. Self-Improving Skill Loop

**What Hermes does:** After complex tasks, the agent autonomously creates skills (markdown files with procedural knowledge). A background curator reviews skill usage telemetry, archives stale ones, and refines high-value ones using LLM summarization. Skills get patched in-place as the agent uses them and discovers edge cases.

**z-Bot delta:** Skills can be loaded, listed, and created via API, but the agent has no tool to autonomously create skills mid-execution. There is no curator. There is no skill usage telemetry. There is no skill refinement cycle.

**What to build:**
- `create_skill` tool (runtime tool, not just API endpoint) so the agent can save procedural knowledge during execution
- Skill usage counter in the skill store (track load count, last used, success/failure signal)
- `SkillCurator` sleep-cycle worker that reviews usage telemetry, archives skills below threshold, and asks an LLM to refine high-usage skills
- Skill patching: when a skill produces a suboptimal result and the agent corrects course, write the correction back into the skill

**Effort:** ~1 week for the tool + telemetry, ~3 days for the curator worker.

---

### 2. 22 Messaging Platform Adapters

**What Hermes does:** Telegram, Discord, Slack, WhatsApp, Signal, Matrix, Email, SMS, WeChat, DingTalk, Feishu, QQ Bot, WeCom, Mattermost, Home Assistant, MS Teams, LINE, SimpleX Chat, Webhook, API Server — all production adapters sharing a single `BasePlatformAdapter` ABC.

**z-Bot delta:** Connectors system exists with HTTP and CLI transports. WebSocket, gRPC, and IPC transports return `UnsupportedTransport`. No platform adapters are built. The bridge worker protocol exists (`/bridge/ws`) but has no reference adapters.

**What to build:**
- WebSocket transport in `gateway/gateway-connectors/src/dispatch.rs` (currently stubbed)
- 3 reference connector adapters (Telegram, Discord, Slack) that demonstrate inbound message → agent invoke → outbound response
- A connector adapter SDK or template so the community can build more

**Effort:** 2-3 days for WebSocket transport, ~1 week per reference adapter.

---

### 3. Browser Automation

**What Hermes does:** Full CDP-based browser control with persistent connections. Snapshot, click, type, scroll, JavaScript console, dialog handling. 180x faster than their previous architecture. Ships as a core tool.

**z-Bot delta:** No browser automation tool exists. No CDP integration. No Playwright/Puppeteer binding.

**What to build:**
- Browser tool wrapping a headless Chrome instance via CDP
- Actions: navigate, snapshot (accessibility tree), click, type, scroll, execute JS
- Persistent browser sessions per ward
- Screenshot capture for multimodal analysis

**Effort:** ~2 weeks for a minimal viable set.

---

### 4. Multimodal I/O (TTS, STT, Image Gen, Video Gen)

**What Hermes does:** Text-to-speech, speech-to-text (including local faster-whisper), image generation (multiple backend providers), video generation. All as pluggable provider subsystems.

**z-Bot delta:** Has `multimodal_analyze` tool for image analysis. No TTS. No STT. No image generation. No video generation.

**What to build:**
- STT tool wrapping a local model (whisper.cpp or faster-whisper via bridge worker)
- TTS tool wrapping a local model (Piper, or API-backed)
- Image gen tool with provider abstraction (OpenAI DALL-E, Stable Diffusion via bridge)
- Lower priority: video gen

**Effort:** ~1 week for STT, ~3 days for TTS, ~1 week for image gen with one provider.

---

## P1 — Gaps That Reduce Real-World Usability

### 5. Session Search / Cross-Session Recall UX

**What Hermes does:** FTS5 search across all past conversations with LLM-summarized recall. User can ask "what did we discuss about the API refactor last month?" and the agent searches and summarizes relevant past sessions.

**z-Bot delta:** The distillation pipeline extracts facts/entities/relationships per session. The `memory` tool has a `recall` action. But there is no `search_sessions` tool that lets the agent search across raw conversation transcripts. The memory search covers distilled knowledge, not the original conversations.

**What to build:**
- `search_sessions` tool backed by FTS5 on conversations.db
- Wire into recall middleware so the agent proactively searches past sessions when the user references past events
- Surface in UI as a search bar in the sessions/conversations view

**Effort:** ~2 days.

---

### 6. Credential Pooling and Provider Failover

**What Hermes does:** Multiple API keys per provider, automatic rotation on 429, exhaustion tracking, credential pool health dashboard.

**z-Bot delta:** One API key per provider config. `RetryingLlmClient` handles 429 with backoff and `RateLimitedLlmClient` manages per-provider semaphores, but if a single key is exhausted mid-session, the session fails.

**What to build:**
- Allow `api_keys: string[]` (or key pool) per provider config
- Rotate to next key on 429
- Surface pool health in the dashboard
- Fallback to cheaper model if all keys for preferred model are exhausted

**Effort:** ~3 days.

---

### 7. Computer Use / GUI Control

**What Hermes does:** A `computer_use` tool that controls the desktop GUI — mouse, keyboard, screenshots. For agents that need to interact with non-terminal applications.

**z-Bot delta:** Not implemented. Shell tool only.

**What to build:**
- Screenshot capture tool
- Mouse/keyboard event injection (platform-specific: xdotool on Linux, AppleScript on macOS)
- Coordinate-based click/type actions with OCR or accessibility-tree targeting

**Effort:** ~1 week for Linux-only MVP, ~2 weeks for cross-platform.

---

### 8. IDE Integration (ACP / Language Server)

**What Hermes does:** Agent Client Protocol server for VS Code, Zed, and JetBrains. The agent runs inside the IDE with access to the editor context.

**z-Bot delta:** Not implemented. The `acp_adapter` concept doesn't exist.

**What to build:**
- ACP server that bridges IDE context to the z-Bot daemon API
- Expose file diagnostics, open files, selection context as tool inputs
- Response channel that can apply edits to the IDE

**Effort:** ~2 weeks for VS Code extension + ACP bridge.

---

## P2 — Polish and Distribution

### 9. One-Click Installation

**What Hermes does:** One-liner `curl | sh` for Linux/macOS/WSL2/Windows/Termux. PyPI package. Docker image. Nix flake.

**z-Bot delta:** `make install` builds from source. Requires Rust toolchain. No pre-built binaries. No Docker image.

**What to build:**
- GitHub Actions CI that publishes release binaries for linux-x86_64, linux-arm64, macos-arm64
- `curl | sh` installer script
- Docker image (daemon + static UI)
- Homebrew tap (optional but nice)

**Effort:** ~1 week for CI + installer + Docker.

---

### 10. User-Facing Documentation

**What Hermes does:** Docusaurus website with guides, API reference, and plugin development docs.

**z-Bot delta:** `AGENTS.md` files in every crate (excellent for developers). No user-facing docs. No getting-started guide. No API reference beyond the OpenAPI spec.

**What to build:**
- Getting started guide (install, configure provider, first conversation)
- Agent configuration guide
- Connector development guide
- Memory system explainer: `memory-bank/components/memory-layer/explainer.md`
- API reference (generate from OpenAPI spec)

**Effort:** ~1-2 weeks for core docs.

---

### 11. Internationalization

**What Hermes does:** 13+ languages for static UI strings.

**z-Bot delta:** English only.

**What to build:**
- i18n framework in the React UI (react-intl or i18next)
- Extract all user-facing strings
- Community translations

**Effort:** ~3 days for framework + English extraction, ongoing for translations.

---

### 12. Mixture of Agents (MoA)

**What Hermes does:** Multi-model collaboration — parallel reference models generate responses, an aggregator synthesizes the best answer from all of them.

**z-Bot delta:** Not implemented. The orchestrator exists as framework code but is not wired into the gateway.

**What to build:**
- Wire `OrchestratorAgent` into the execution runner
- Add an agent config option for MoA mode (parallel LLM calls to multiple providers, aggregate response)
- Complexity-based routing: simple queries hit one fast model, complex queries fan out to multiple

**Effort:** ~1 week to wire the orchestrator + ~3 days for MoA mode.

---

## Summary: Priority Matrix

| # | Delta | Effort | User Impact |
|---|-------|--------|-------------|
| 1 | Self-improving skill loop | ~2 weeks | High — the Hermes flagship feature |
| 2 | Messaging platform adapters | ~4-6 weeks (3 adapters) | High — unlocks the "agent everywhere" use case |
| 3 | Browser automation | ~2 weeks | Medium — web interaction is common |
| 4 | Multimodal I/O | ~2-3 weeks (STT + TTS + image gen) | Medium — voice I/O is the growth path |
| 5 | Session search | ~2 days | Medium — users expect to find past conversations |
| 6 | Credential pooling | ~3 days | Medium — reliability for power users |
| 7 | Computer use / GUI | ~1-2 weeks | Low — niche but impressive |
| 8 | IDE integration | ~2 weeks | Low — developer audience only |
| 9 | One-click install | ~1 week | High — adoption blocker |
| 10 | User documentation | ~1-2 weeks | High — adoption blocker |
| 11 | Internationalization | ~3 days + ongoing | Low — post-adoption |
| 12 | MoA / orchestrator wiring | ~2 weeks | Medium — quality differentiator |

### The 30-Day Path to Parity

If the goal is feature parity with Hermes on the dimensions that matter most:

**Week 1-2:** Self-improving skill loop + session search + credential pooling + one-click install
**Week 3-4:** Wire orchestrator + 1 messaging adapter (Telegram) + browser automation MVP + getting-started docs

That gives z-Bot the learning loop, the integration story, and the onboarding story. Everything else is expansion.
