# MicCamWatch documentation

[Project overview](../README.md) · [Official website](https://miccamwatch.pages.dev/) ·
[Public releases](https://github.com/Roman-Cuisset/miccamwatch/releases)

**Public stable: 0.20.0. Development candidate: 0.21.0, not published.**
Use the version-specific guidance below; a candidate CI result is not a released
package or proof of physical hardware compatibility.

## Start here

| Goal | Document |
| --- | --- |
| Install the right package and verify it | [Installation](INSTALLATION.md) |
| Update, uninstall, or migrate an old installation safely | [Installation lifecycle](INSTALLATION.md#updates) |
| Run commands, choose a profile, or troubleshoot status | [Usage and safety](USAGE.md) |
| Understand what monitoring/control can and cannot prove | [Observation and control limits](USAGE.md#observation-and-control-limits) |
| Understand local history, network use, and personal data | [Logs and privacy](USAGE.md#logs-and-privacy) and [official Privacy Policy](https://miccamwatch.pages.dev/privacy) |

## Technical references

- [Architecture](ARCHITECTURE.md): collectors, evidence, ownership, IPC,
  implementation constraints, and version-scoped verification detail.
- [Signing and safe handling](SIGNING.md): Authenticode configuration, provenance,
  historical Defender incidents, and why a checksum is not a publisher signature.
- [Website maintenance](WEBSITE.md): static build, localization, canonical URLs,
  Cloudflare configuration, and explicitly authorized deployment checks.
- [Repository privacy notes](../PRIVACY.md): technical application privacy context;
  the public multilingual policy remains on the official website.

## Release status and evidence

- [Validation](VALIDATION.md) is the concise starting point for public 0.20.0,
  unpublished 0.21.0, native CI, and the remaining hardware/manual limits.
- [Release history](RELEASE_HISTORY.md) preserves changes, immutable proof links,
  migration records, and dated publication/website observations moved out of the
  project overview. Historical “current/latest” statements are scoped to their
  release, not instructions to install an old version today.
- [Roadmap](ROADMAP.md), in French, records priorities, decisions, and handoffs.
  The candidate's detailed GUI/ARM history stays on its
  [protected development branch](https://github.com/Roman-Cuisset/miccamwatch/tree/work/linux-arm64-021).
- [Earlier Linux/macOS handoff](NEXT_AI_LINUX_MACOS.md) is a historical engineering
  journal. Its superseded restrictions and release statuses are not the current
  command reference or the next development plan.

## Before sharing diagnostics or changing an installation

Redact personal paths and process/application identities from reports. Unknown
coverage is not an all-clear; software mute is not universal access denial.
Restore owned changes and finish pending restoration before external replacement
or removal. Do not delete ownership journals, adopt a foreign owner, or bypass
SmartScreen/Gatekeeper to make a command succeed.

No documentation command authorizes production deployment, a new application
release, a privilege grant, or physical-device testing on someone else's machine.
