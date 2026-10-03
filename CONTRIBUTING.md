# Contributing to Professional DocX

Professional DocX is publicly readable, but the repository remains maintainer-controlled.

## Governance

External contributions are proposals only. Opening a pull request does not grant write access, approval authority, or permission to merge changes. The repository owner decides whether a change is accepted and merged.

All repository content is subject to owner review. Protected-branch rules and required checks will be used for `master` and `develop` once the repository is public.

## Never submit confidential material

Do not commit, upload, paste, or attach:

- real client or audit documents;
- real client filenames or engagement exports;
- production databases, search indexes, logs, backups, or crash dumps;
- credentials, tokens, certificates, signing keys, or `.env` files;
- internal server names, private IP addresses, VPN configuration, NAS/share paths, or production endpoints;
- screenshots containing client, staff, infrastructure, or engagement information.

Use synthetic fixtures and fictional names only.

## Pull requests

Before submitting a pull request:

1. Keep the change focused.
2. Add or update tests where behavior changes.
3. Run the relevant frontend and Rust checks.
4. Explain any new dependency, network access, filesystem permission, Tauri capability, or security-boundary change.
5. Do not weaken CSP, path validation, authorization boundaries, evidence integrity, or audit logging without an explicit design rationale.

Pull requests from forks may require maintainer approval before GitHub Actions are allowed to run.

## Security issues

Do not open a public issue for a suspected vulnerability. Follow [SECURITY.md](SECURITY.md).
