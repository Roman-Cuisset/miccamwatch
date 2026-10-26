# MicCamWatch documentation

[Project overview](../README.md) · [Official website](https://miccamwatch.pages.dev/) ·
[Public releases](https://github.com/Roman-Cuisset/miccamwatch/releases)

**Public stable: [0.21.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.21.0).**
Use the version-specific guidance below; public package verification and earlier
source-specific qualification are not proof of physical hardware compatibility.

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

- [Validation](VALIDATION.md#public-release-0210) is the concise starting point for
  public 0.21.0, verified packages/provenance, recorded native qualification,
  the completed Windows standard-user gate and remaining hardware/manual limits.
- [0.21.0 release record](RELEASE_0.21.0.md) preserves package verification,
  publication recovery and separately authorized deployment boundaries.
- [Release history](RELEASE_HISTORY.md) preserves changes, immutable proof links,
  migration records, and dated publication/website observations moved out of the
  project overview. Historical “current/latest” statements are scoped to their
  release, not instructions to install an old version today.
- [Roadmap](ROADMAP.md), in French, records priorities, decisions, and handoffs.
  Public 0.21.0 is pinned to
  [`3d74686ace7b04728794765f9eb57c0c0a8bc84c`](https://github.com/Roman-Cuisset/miccamwatch/tree/3d74686ace7b04728794765f9eb57c0c0a8bc84c);
  the [immutable GUI/ARM handoff](https://github.com/Roman-Cuisset/miccamwatch/blob/cafd98daf7541f317b44aee98edb3b6efa3af561/docs/ROADMAP.md#L1698-L1718)
  preserves the earlier platform-by-platform qualification.
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
