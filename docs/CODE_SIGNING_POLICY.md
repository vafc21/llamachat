# Code Signing Policy

This document describes how release artifacts for **LlamaChat**
(<https://github.com/vafc21/llamachat>) are built and code-signed. It follows the
requirements of the [SignPath Foundation](https://signpath.org/).

## Signing provider

Free code signing is provided by [SignPath.io](https://signpath.io/), certificate
by the [SignPath Foundation](https://signpath.org/). LlamaChat is grateful for
their support of open-source software.

## Team roles

LlamaChat is currently maintained by a single maintainer who performs all roles.
As the project grows these roles may be assigned to different people.

- **Authors / Committers** — write and commit source code. Current: the project
  maintainer (GitHub [@vafc21](https://github.com/vafc21)).
- **Reviewers** — review all changes from external contributors before merge.
  External contributions are accepted only via pull request and are reviewed
  before merging. Current: the project maintainer.
- **Approvers** — authorize the signing of a release. Current: the project
  maintainer.

All team members use multi-factor authentication (MFA) for both their GitHub
account and their SignPath account.

## Build & signing process

- All release binaries are built from source **only** by the project's GitHub
  Actions CI (`.github/workflows/build.yml`) on GitHub-hosted runners. No signed
  artifact is ever built on a developer machine.
- Signing is triggered only for tagged releases (`v*`) and is performed by the
  SignPath signing request in the CI `release` job. The signing credentials
  (`SIGNPATH_API_TOKEN`) live only as encrypted GitHub Actions secrets and are
  never exposed to contributors.
- Only artifacts produced by that CI workflow, from the project's own source
  code, are submitted for signing.

## Signed artifacts & metadata

Signed Windows artifacts are limited to LlamaChat's own installers and
executable:

- `LlamaChat_*_x64-setup.exe` (NSIS installer)
- `LlamaChat_*_x64_en-US.msi` (MSI installer)
- the `LlamaChat.exe` application binary they contain

Signed binaries carry the product name **LlamaChat** and the project's version
and publisher metadata. No third-party or unrelated binaries are signed.

## Privacy

LlamaChat is local-first and collects **no** telemetry or personal data by
default; nothing leaves the user's device unless they explicitly opt in. See the
[Privacy Policy](../PRIVACY.md) for details. The signing process itself collects
no end-user data.
