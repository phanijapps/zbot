# RFC-0004: GitHub Release Installer and Cross-Platform Packaging

- **Status:** Draft
- **Author:** phanijapps
- **Approver:** phanijapps
- **Date opened:** 2026-06-01
- **Date closed:**
- **Related:** `memory-bank/future-state/path-to-release.md`; `memory-bank/PUBLISHING.md`; `docs/specs/github-release-installer/`

## The ask

**Recommendation (BLUF):** approve GitHub Releases as the source of prebuilt
zbot binaries and GitHub-hosted installer scripts as the public install path.
The user-facing Linux/macOS install command should be:

```bash
curl -fsSL https://raw.githubusercontent.com/phanijapps/zbot/main/scripts/install-release.sh | bash
```

Windows should use a PowerShell installer:

```powershell
irm https://raw.githubusercontent.com/phanijapps/zbot/main/scripts/install.ps1 | iex
```

**Why now:** zbot already has a source-build Linux installer, but that is not a
public packaging story. It requires Rust, Node, GCC, a local checkout, and time.
The release workflow also still packages legacy binary names in places. The
question is whether zbot's public install path should become a binary-release
installer backed by GitHub Releases, with the current source-build installer
kept as a developer path.

**Decisions requested:**

1. **Public installer source:** approve GitHub raw installer scripts under
   `scripts/` as the public bootstrap path. Default if no objection:
   `scripts/install-release.sh` for Linux/macOS and `scripts/install.ps1` for
   Windows.
2. **Binary source:** approve GitHub Releases assets as the only source for
   public installer downloads. Default if no objection: installers query the
   latest release by default and accept a pinned `--version`.
3. **Artifact contract:** approve `zbotd` and `zbot` as the packaged binary
   names and `zbot-<tag>-<platform>-<arch>` as archive naming. Default if no
   objection: stop shipping `zerod` / `zero` artifacts except as explicit
   legacy compatibility.
4. **Integrity contract:** approve checksum verification as mandatory before
   installing downloaded archives. Default if no objection: publish
   `checksums.sha256` with every release and make checksum mismatch a hard
   failure.
5. **Versioning scope:** do not reopen versioning. Default if no objection:
   inherit the existing CalVer tag scheme `vYYYY.M.D` with no zero padding.

## Problem & goals

The current install path is optimized for a developer or Pi owner who is
comfortable building from source. A public installer should not require a Rust
toolchain, Node, local source checkout, or source compilation. It should install
the released binaries that CI produced and verified.

At RFC drafting time, the release workflow and publishing docs were not
aligned:

- Release packaging still refers to `zerod` and `zero` in workflow steps.
- The product binary names are now `zbotd` and `zbot`.
- The current `scripts/install.sh` builds locally and targets Linux only.
- Release artifacts need to support Linux x86_64, Linux ARM64, macOS Intel,
  macOS Apple Silicon, and Windows x86_64.
- Windows needs PowerShell installation rather than a Bash pipeline.

**Goals:**

- Provide a one-command Linux/macOS install path that downloads prebuilt GitHub
  Release artifacts instead of building locally.
- Provide a Windows PowerShell install path with the same release-asset
  contract.
- Keep source-build installation available for developers and unsupported
  platforms.
- Publish deterministic release artifact names and archive layouts.
- Verify downloaded artifacts with SHA-256 checksums before installation.
- Keep CalVer versioning as an inherited constraint, not a topic reopened by
  this RFC.
- Remove packaging blockers that affect cross-platform binaries: legacy binary
  names, native TLS portability, unconditional jemalloc on Windows, and UI asset
  packaging.

**Non-goals:**

- Creating a custom package registry or package manager.
- Hosting installer scripts on a separate product domain.
- Replacing Homebrew, winget, AUR, Docker, or OS-native packages in this first
  packaging slice.
- Changing the existing `vYYYY.M.D` CalVer scheme.
- Adding automatic self-update behavior to the installed daemon or CLI.
- Solving macOS notarization and Windows code signing in the first installer
  slice; those remain follow-up release hardening work.

## Proposal

### 1. Installer split

Keep the current source-build flow, but separate it from the public binary
installer.

- `scripts/install-release.sh`: public Linux/macOS installer. Downloads
  prebuilt release archives from GitHub Releases.
- `scripts/install-from-source.sh`: source-build installer, derived from the
  current `scripts/install.sh`.
- `scripts/install.ps1`: public Windows installer. Downloads the matching
  Windows release zip from GitHub Releases.
- `scripts/uninstall.sh`: keep Linux uninstall behavior and update it for
  release-installed paths where needed.

The public README should show the release installer first. Source install
instructions should move to a developer section.

### 2. Installer behavior

The Linux/macOS installer should:

- Detect OS: `linux` or `macos`.
- Detect architecture: `x86_64` or `aarch64`.
- Resolve version:
  - default: latest non-prerelease GitHub Release;
  - `--version <tag>`: pinned release tag;
  - `--repo <owner/repo>`: optional test override.
- Select the matching archive:
  - `zbot-<tag>-linux-x86_64.tar.gz`
  - `zbot-<tag>-linux-aarch64.tar.gz`
  - `zbot-<tag>-macos-x86_64.tar.gz`
  - `zbot-<tag>-macos-aarch64.tar.gz`
- Download `checksums.sha256` from the same release.
- Verify the selected archive checksum before extraction.
- Install `zbotd` and `zbot` to `~/.local/bin` by default.
- Create the data directory under `~/Documents/zbot/` when possible.
- On Linux, optionally install a systemd user service unless `--no-service` is
  supplied.
- On macOS, skip service installation in the first slice unless a LaunchAgent
  template is explicitly added.

The Windows installer should:

- Detect `x86_64-pc-windows-msvc` support.
- Resolve latest or pinned release.
- Download `zbot-<tag>-windows-x86_64.zip`.
- Verify checksum using PowerShell's built-in hash support.
- Install `zbotd.exe` and `zbot.exe` to a user-local bin directory.
- Add that directory to the user PATH if needed, with clear messaging.

### 3. Release artifact contract

Release archives should be named:

```text
zbot-<tag>-linux-x86_64.tar.gz
zbot-<tag>-linux-aarch64.tar.gz
zbot-<tag>-macos-x86_64.tar.gz
zbot-<tag>-macos-aarch64.tar.gz
zbot-<tag>-windows-x86_64.zip
checksums.sha256
```

Each archive should contain a top-level directory:

```text
zbot-<tag>/
├── zbotd
├── zbot
├── README.md
├── LICENSE
└── VERSION
```

Windows archives contain `zbotd.exe` and `zbot.exe` instead.

The release workflow must package `zbotd` and `zbot`; it must not copy
`zerod` or `zero` unless a deliberate legacy compatibility artifact is added
and named as such.

### 4. Compatibility prerequisites

This RFC treats these as packaging prerequisites, not separate product
features:

- Use `reqwest` with explicit `rustls-tls` and `default-features = false` so
  release builds do not depend on platform OpenSSL/native TLS wiring.
- Gate `tikv-jemallocator` and `#[global_allocator]` behind `cfg(unix)` so the
  Windows MSVC build uses the system allocator.
- Either embed the production UI into `zbotd` or include the UI `dist/` in
  release archives and start the daemon with a correct static asset fallback.
  The preferred packaging shape is embedded UI because it keeps the runtime
  archive small and simple.
- Add Linux ARM64 release output if Raspberry Pi installation is a supported
  public path.

### 5. Versioning constraint

Versioning is out of scope. This RFC inherits the existing CalVer decision:
release tags use `vYYYY.M.D` with no zero padding. Artifact names include the
tag verbatim, for example:

```text
zbot-v2026.6.1-linux-x86_64.tar.gz
```

If versioning changes later, it should be handled by a separate RFC or ADR and
this installer contract should be updated to follow it.

## Options considered

The option space is MECE along the source from which a user obtains executable
bits.

| Option | Description | Trade-offs |
| --- | --- | --- |
| Do nothing | Keep documenting source checkout plus `scripts/install.sh`. | Lowest work, but public install remains slow and requires developer tooling. |
| Source-build curl installer | Host a script that clones the repo and builds locally. | Simple to ship, but still requires Rust/Node/GCC and fails the binary packaging goal. |
| GitHub raw script + GitHub Releases binaries | Host installer scripts in the repo and download matching release artifacts. | Recommended. Minimal infrastructure, auditable script source, and users install CI-built binaries. |
| Product-domain script + GitHub Releases binaries | Host `https://openclaw.ai/install.sh` or equivalent as a nicer entrypoint. | Good later for branding, but adds DNS/CDN ownership before the release contract is stable. |
| OS package managers first | Prioritize Homebrew, winget, AUR, apt, or Docker before raw scripts. | Useful follow-up, but slower to establish and still needs the same release artifacts underneath. |

Recommended option: GitHub raw script plus GitHub Releases binaries. It gives a
clean install command without adding hosting infrastructure or reopening
versioning.

## Risks & what would make this wrong

**Pre-mortem:**

- The installer downloads a wrong or tampered archive. Mitigation: verify
  `checksums.sha256` and fail closed on mismatch or missing checksum.
- GitHub API rate limits unauthenticated users. Mitigation: support explicit
  `--version` asset URLs and simple error messages; use only one release API
  call in the normal path.
- macOS users expect a service but the first installer only installs binaries.
  Mitigation: say that clearly and add LaunchAgent support in a follow-up.
- Linux service installation differs across distributions. Mitigation: make
  service installation optional and keep binary install useful by itself.
- Windows PATH mutation is brittle. Mitigation: use user-level PATH updates and
  print the installed path even when PATH update is skipped.
- Embedded UI makes the release build depend on the UI build output. Mitigation:
  release workflow builds UI first and fails if assets are missing.

**Key assumptions:**

- GitHub Releases are acceptable as the public binary distribution backend.
- A GitHub raw script URL is acceptable as the public install command for the
  first packaging slice.
- The product binary names are `zbotd` and `zbot`.
- CalVer `vYYYY.M.D` is already decided and should not be reopened here.
- Checksum verification is the minimum acceptable release integrity gate.

**Drawbacks:**

- `curl | bash` and `irm | iex` are convenient but require users to trust the
  script URL. Checksums protect the downloaded archive, not the installer
  script itself.
- GitHub availability becomes part of the install path.
- Raw-script install is not as native as Homebrew, winget, apt, or signed
  installers.
- Supporting services across Linux/macOS/Windows adds platform-specific code
  that needs focused tests and manual verification.

## Evidence & prior art

**Spike / de-risk result:** repository inspection at RFC drafting time
confirmed the workflow and installer did not yet match this contract.

- `.github/workflows/release.yml` copies `zerod` and `zero` while the product
  binaries are `zbotd` and `zbot`.
- `scripts/install.sh` is a source-build Linux installer, not a release-binary
  installer.
- `Cargo.toml` still enables default `reqwest` features instead of explicit
  `rustls-tls`.
- `apps/daemon` currently depends on and installs jemalloc unconditionally.

**Repo precedent:**

- `memory-bank/future-state/path-to-release.md` already names GitHub Releases,
  CalVer, `curl | sh`, checksums, cross-platform targets, rustls, and
  jemalloc gating as release-path requirements.
- `memory-bank/PUBLISHING.md` documents `zbotd` and `zbot` as the release
  binaries and describes cross-platform archive names.
- `scripts/install.sh` already owns Linux service setup behavior that can be
  reused after the installer split.

**External prior art:**

- GitHub's REST release API provides latest-release and release-asset endpoints
  suitable for installer asset discovery:
  <https://docs.github.com/en/rest/releases>
- GitHub documents release integrity verification as a supply-chain practice:
  <https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/verifying-the-integrity-of-a-release>
- `reqwest` documents that `default-tls` is enabled by default and that callers
  must use `default-features = false` to ensure a specific TLS backend:
  <https://docs.rs/reqwest/latest/reqwest/tls/index.html>
- curl release documentation treats signed release archives and verification as
  normal release hygiene:
  <https://curl.se/docs/verify.html>

## Experiment / validation

This RFC should be validated by CI artifacts and clean-machine installation
checks rather than a long production experiment.

**Hypothesis:** users can install zbot on supported platforms without a local
Rust or Node build toolchain when release artifacts are present.

**What we measure:**

- CI produces every expected archive and `checksums.sha256`.
- The Linux/macOS installer selects the right archive for OS/architecture.
- The Windows installer selects the right zip.
- Checksum mismatch causes a hard installer failure.
- Installed `zbotd --version` and `zbot --version` run from the install
  directory.

**Success criteria:**

- `scripts/install-release.sh --version <tag> --install-dir <tmpdir>
  --no-service` succeeds on Linux x86_64.
- A dry-run or test mode proves macOS x86_64 and macOS ARM64 archive selection.
- `scripts/install.ps1 -Version <tag> -InstallDir <tmpdir>` succeeds on
  Windows x86_64 CI or a manual Windows runner.
- Release workflow artifact names match the contract exactly.
- No release archive contains `zerod` or `zero` as the primary binary names.

## Open questions

1. **Should macOS install a LaunchAgent in the first slice?**
   Recommended default: no; install binaries first and add LaunchAgent support
   after the artifact contract is stable. Owner: phanijapps. Decide-by:
   implementation planning.
2. **Should Linux install the systemd user service by default?**
   Recommended default: yes when systemd user mode is available, with
   `--no-service` to install binaries only. Owner: phanijapps. Decide-by:
   implementation planning.
3. **Should the public Linux/macOS command use `bash` or `sh`?**
   Recommended default: `bash`, because the existing installer style uses Bash
   arrays and strict-mode helpers. Owner: phanijapps. Decide-by:
   implementation planning.

## Follow-on artifacts

- Spec: `docs/specs/github-release-installer/`
- ADR if accepted: record GitHub Releases as the public binary distribution
  source and raw GitHub installer scripts as the first public bootstrap path.
- Later specs or RFCs:
  - macOS LaunchAgent and notarization;
  - Windows code signing and winget;
  - Homebrew/AUR packages;
  - signed checksums and provenance.
