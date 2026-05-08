# Path to Release — Cross-Platform Packaging & Distribution

**Date:** 2026-04-27
**Status:** Partial implementation — Phase 2 (ARM64 + rustls) and Phase 3 (`curl|sh` installer) landed; Phase 1 (embed UI) and Phase 4 (CalVer release.yml) still pending. CalVer scheme has shifted from `vYYYY.MM.DD` (zero-padded) to `vYYYY.M.D` (no zero-padding) per PR #102/#106 — see `2026-05-03-versioning-and-rename-plan.md`. Binary names are now `zbotd`/`zbot` (PR #103/#104/#107).
**Depends on:** Embedded UI (Phase 1), rustls migration (Phase 2 — done)

---

## 1. Goal

Package zbot as a downloadable, verifiable binary for **Linux (x86_64 + ARM64/Pi 5), macOS (Intel + Apple Silicon), and Windows (x86_64)** with:

- `curl | sh` one-command install
- CalVer versioning (`vYYYY.M.D`, no zero-padding)
- Cryptographic signing and provenance
- Security scanning (CVE + malware + secrets)
- Automated license attribution and SBOM
- Bleeding-edge Docker via GHCR

---

## 2. Non-Goals

- **Docker Hub publishing.** All Docker images go to GHCR only.
- **ARM32 builds.** Raspberry Pi Zero/2 not targeted — Pi 5 (aarch64) is the minimum ARM target.
- **Multiple same-day releases.** One CalVer release per day max. Bleeding-edge fixes go to Docker (`main` branch).
- **Bundling Python/Node.js/LibreOffice.** These are expected to be pre-installed on target systems. Installers warn if missing.

---

## 3. Distribution Channels

| Channel | Source | Version | Trigger |
|---------|--------|---------|---------|
| **GHCR Docker** | `main` branch | `latest` + `:sha-<short>` | Every push to `main` |
| **GitHub Releases** | Git tags (`vYYYY.M.D`) | CalVer date stamp | `scripts/release.sh` |
| **Native install** | GitHub Releases (latest) | Resolved from API | `curl | sh` / PowerShell |

---

## 4. Phases

### Phase 1: Embed Frontend in Binary [PENDING]

Single binary deployment — eliminate the need for a separate `dist/` directory. Not yet implemented as of 2026-05-08 — `apps/daemon/Cargo.toml` does not pull in `rust-embed`; only `gateway/gateway-templates/` uses `RustEmbed` for prompt templates.

**Changes:**
- Add `rust-embed` to `apps/daemon/Cargo.toml` with `embedded-ui` feature
- Create embedded asset module in daemon that includes `apps/ui/dist/` at compile time via `rust-embed`
- Modify `gateway/src/http/mod.rs` — when `--static-dir` is not provided, serve from embedded assets
- Update `gateway/src/config.rs` — detect embedded assets as default fallback
- Wrap jemalloc in `#[cfg(unix)]` — use system allocator on Windows (jemalloc doesn't support MSVC)
- Keep `--static-dir` CLI flag as a dev-time override

**Files:** `apps/daemon/Cargo.toml`, `apps/daemon/src/main.rs`, `gateway/src/http/mod.rs`, `gateway/src/config.rs`

---

### Phase 2: Add ARM64 Linux + rustls Migration [LANDED — PR #90]

Build for 5 targets. Eliminate OpenSSL cross-compilation dependency. `rustls` is in `Cargo.lock`; ARM64/Pi packaging shipped in PR #90.

**Changes:**
- Switch reqwest to `rustls` backend:
  ```toml
  reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls"] }
  ```
- Add `aarch64-unknown-linux-gnu` target to `release.yml` build matrix
- Install cross-compiler in CI: `gcc-aarch64-linux-gnu`, set `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER`
- Verify `fastembed`/ONNX Runtime has `aarch64-linux` prebuilt binaries
- jemalloc compiles from C source — works with cross-compiler natively

**Build targets:**

| Platform | Rust Target | Archive |
|----------|-------------|---------|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Linux ARM64 (Pi 5) | `aarch64-unknown-linux-gnu` | `.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `.tar.gz` |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `.zip` |

**Files:** `Cargo.toml` (workspace), `.github/workflows/release.yml`

---

### Phase 3: `curl | sh` Installer [LANDED]

One-command install from GitHub Releases. `scripts/install.sh` and `scripts/uninstall.sh` exist; service template substitutes `@@VERSION@@` from `Cargo.toml` per PR A2 (`6a92f91f`). PowerShell installer for Windows is still pending.

**`install.sh`** (Linux / macOS / Pi):
1. Detect OS (`linux`/`macos`) and architecture (`x86_64`/`aarch64`)
2. Fetch latest release from `https://api.github.com/repos/phanijapps/zbot/releases/latest`
3. Download and extract the matching tarball
4. Install `zbotd` + `zbot` to `~/.local/bin/`
5. Add `~/.local/bin` to PATH via `.bashrc`/`.zshrc` (idempotent)
6. Create data directory `~/Documents/zbot/`
7. Check for Python 3 and Node.js — print warnings if missing
8. Print success message with next steps

**`install.ps1`** (Windows):
Same logic, PowerShell equivalent. Installs to `%USERPROFILE%\bin\`.

**Usage:**
```bash
# Linux / macOS / Raspberry Pi
curl -fsSL https://raw.githubusercontent.com/phanijapps/zbot/main/install.sh | sh

# Windows PowerShell
irm https://raw.githubusercontent.com/phanijapps/zbot/main/install.ps1 | iex
```

**Files created:** `install.sh`, `install.ps1`

---

### Phase 4: Release Management (CalVer `vYYYY.M.D`) [PENDING — release.sh helper not yet implemented; manual cuts in use]

Version from date, one release per day, Docker handles bleeding edge. CalVer scheme is `YYYY.M.D` with **no zero-padding** (semver forbids leading zeros in numeric identifiers — see `2026-05-03-versioning-and-rename-plan.md`).

**`scripts/release.sh`:**
1. Generate version: `v$(date +%Y.%-m.%-d)` — `%-m`/`%-d` strip leading zeros (GNU date)
2. Fail if git tag already exists — no same-day releases
3. Update `version` in `[workspace.package]` of root `Cargo.toml` (`cargo set-version --workspace`)
4. Update `version` in `apps/ui/package.json` (`npm version --no-git-tag-version`)
5. Generate `CHANGELOG.md` from `git log LAST_TAG..HEAD --oneline`
6. Commit: `release: YYYY.M.D`
7. Tag: `git tag -a vYYYY.M.D -m "Release YYYY.M.D"`
8. Push tag → triggers `release.yml`

**Flags:**
- `--dry-run` — show what would happen without making changes
- `--version v2026.5.4` — manual override for same-day exceptional releases (bump the day)

**Files created:** `scripts/release.sh`, `CHANGELOG.md`

---

### Phase 5: Security Scanning + Binary Signing

Block releases with vulnerabilities. Assure users binaries are safe.

#### 5a. Continuous Scanning (re-enable `security.yaml`)

Currently disabled. Re-enable with expanded triggers (push to `main` + weekly schedule):

| Scan | Tool | Trigger |
|------|------|---------|
| Rust vulnerabilities | `cargo audit` | Push + weekly |
| Rust dependency policy | `cargo deny check` | Push + weekly |
| Rust linting | `cargo clippy -D warnings` | Push |
| Node vulnerabilities | `npm audit --audit-level=high` | Push + weekly |
| Secrets scanning | Gitleaks | Push + weekly |
| SAST (deep security) | CodeQL (Rust + JS/TS) | Push + weekly |
| Container CVE scan | Trivy | On Docker image build |

#### 5b. Docker Workflow (`.github/workflows/docker.yml`)

- Triggered on push to `main`
- Build multi-arch image (x86_64 + ARM64)
- Trivy scan — fail on critical CVEs
- Push to `ghcr.io/phanijapps/zbot:latest` and `:sha-<short-sha>`
- No Docker Hub

#### 5c. Release Gate (in `release.yml`)

Run **before** building any platform artifacts:
- `cargo audit` — block on known vulnerability
- `cargo deny check` — block on license/advisory/ban violations

#### 5d. Binary Scanning + Signing (post-build, pre-release)

| Step | Tool | Purpose | Output |
|------|------|---------|--------|
| CVE scan tarballs | **Grype** | Scan built archives for known CVEs | Fail release if critical/high found |
| Sign binaries | **Cosign** (keyless Sigstore) | Sign each `.tar.gz`/`.zip` via GitHub OIDC | `.sig` + `.cert` files attached to release |
| Provenance | **Cosign attest** | SLSA provenance — proves binary built by GitHub Actions from specific commit | Attestation attached to release |
| Malware scan | **VirusTotal API** | Upload each artifact — scanned by 70+ AV engines | Scan report URL in release notes |

**User-facing release notes include:**
```
All binaries are:
- CVE-scanned with Grype (0 critical/high vulnerabilities)
- Malware-scanned via VirusTotal (0 detections / 70+ engines)
- Signed with Sigstore/Cosign (verify provenance)
- Attested with SLSA provenance (built by GitHub Actions from source)

Verify a binary:
  cosign verify-blob --certificate zbotd.sig \
    --signature zbotd.sig \
    --certificate-identity https://github.com/phanijapps/zbot/.github/workflows/release.yml \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com \
    zbotd
```

**Files modified:** `.github/workflows/security.yaml`, `.github/workflows/test.yml`
**Files created:** `.github/workflows/docker.yml`

---

### Phase 6: License Compliance + SBOM

Automated license attribution, shipped with every release.

**`about.toml`** — config for `cargo-about`:
- Reads the existing `deny.toml` allow-list (MIT, Apache-2.0, BSD, ISC, etc.)
- Generates formatted HTML with every Rust crate's license text

**CI step in `release.yml`:**
- `cargo about generate about.hbs -o THIRD-PARTY-LICENSES.html` — Rust dependency licenses
- `npx license-checker --json > npm-licenses.json` — Node dependency licenses
- Include both in release tarball alongside `zbotd`, `zbot`, `README.md`, `VERSION`, `LICENSE`

**SBOM generation:**
- Rust: `cargo cyclonedx` → `bom.cdx.json`
- Docker: `syft ghcr.io/phanijapps/zbot:latest -o cyclonedx-json`
- Both attached to GitHub Release

**Files created:** `about.toml`, `about.hbs`, `NOTICE`
**Files modified:** `.github/workflows/release.yml`

---

## 5. GitHub Secrets Required

| Secret | Purpose | Cost |
|--------|---------|------|
| `GITHUB_TOKEN` | Already auto-provided — Cosign keyless signing, GHCR push | Free |
| `VIRUSTOTAL_API_KEY` | Free API key from virustotal.com — binary malware scanning | Free (public API, 500 req/day) |

All other tools (Trivy, Grype, Cosign, CodeQL, cargo-audit, cargo-deny, Syft, cargo-cyclonedx, cargo-about) require no secrets — free/open-source tools that run in CI.

---

## 6. Complete File Change List

| File | Action | Phase |
|------|--------|-------|
| `apps/daemon/Cargo.toml` | Edit — add `rust-embed`, `embedded-ui` feature | 1 |
| `apps/daemon/src/main.rs` | Edit — embedded asset serving, conditional jemalloc | 1 |
| `gateway/src/http/mod.rs` | Edit — serve from embedded assets | 1 |
| `gateway/src/config.rs` | Edit — detect embedded assets as fallback | 1 |
| `Cargo.toml` (workspace) | Edit — switch reqwest to `rustls` | 2 |
| `.github/workflows/release.yml` | Major rewrite — ARM64, security gate, signing, SBOM, licenses, CalVer | 2, 4, 5, 6 |
| `install.sh` | Create — `curl | sh` installer | 3 |
| `install.ps1` | Create — PowerShell installer | 3 |
| `scripts/release.sh` | Create — CalVer release management script | 4 |
| `CHANGELOG.md` | Create — auto-generated changelog | 4 |
| `.github/workflows/security.yaml` | Edit — re-enable triggers, add CodeQL, `cargo deny` | 5 |
| `.github/workflows/test.yml` | Edit — re-enable triggers | 5 |
| `.github/workflows/docker.yml` | Create — GHCR build + Trivy scan | 5 |
| `about.toml` | Create — `cargo-about` config | 6 |
| `about.hbs` | Create — license report HTML template | 6 |
| `NOTICE` | Create — attribution notices | 6 |

---

## 7. Execution Order

```
Wave 1 (parallel — no interdependencies):
  Phase 1: Embed frontend
  Phase 2: ARM64 + rustls
  Phase 5a/5b: Re-enable security + Docker workflow
  Phase 6: License compliance + SBOM tooling

Wave 2 (depends on Wave 1):
  Phase 3: Installer scripts (needs release artifacts to exist)
  Phase 4: Release management script

Wave 3 (depends on Wave 2):
  Phase 5c/5d: Release gate + binary scanning/signing (needs release workflow)
```

---

## 8. CI/CD Pipeline Summary

```
Push to main:
  test.yml            unit + integration + e2e + coverage
  security.yaml       audit + deny + clippy + gitleaks + npm audit + CodeQL
  docker.yml          build image → Trivy scan → push GHCR (:latest, :sha)

Push tag vYYYY.M.D:
  release.yml
    ├── Security gate (cargo audit + cargo deny)
    ├── Build frontend (once, share as artifact)
    ├── Build 5 targets (linux-x64, linux-arm64, mac-x64, mac-arm64, win-x64)
    ├── Generate SBOM (cargo cyclonedx)
    ├── Generate license report (cargo-about + license-checker)
    ├── Grype scan (CVE check on tarballs)
    ├── Cosign sign + attest (keyless Sigstore)
    ├── VirusTotal upload (malware scan)
    └── Create GitHub Release (artifacts + checksums + scan results)

Weekly schedule:
  security.yaml       full scan even if no code changes (catches new CVEs in existing deps)
```

---

## 9. Release Artifact Structure

Each release tarball/zip contains:

```
zbotd                          # Daemon binary
zbot                           # CLI binary (not on Windows if TUI issues)
README.md                      # Project readme
LICENSE                        # MIT license
VERSION                        # CalVer tag string
THIRD-PARTY-LICENSES.html      # Full license text for all Rust + Node deps
npm-licenses.json              # Node dependency license report
NOTICE                         # Attribution notices
bom.cdx.json                   # CycloneDX SBOM
```

GitHub Release additionally includes:
- `checksums.sha256` — SHA256 hashes of all archives
- `*.sig` + `*.cert` — Cosign signatures per archive
- VirusTotal scan report links in release body
- SLSA provenance attestation
