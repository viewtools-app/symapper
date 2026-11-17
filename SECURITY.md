# Security policy

## Supported versions

Security fixes target the latest released version. Upgrade to the latest release
before reporting a problem when possible.

## Reporting a vulnerability

Report vulnerabilities privately through
[GitHub security advisories](https://github.com/viewtools-app/symapper/security/advisories).
Select "Report a vulnerability" on the repository. Do not open a public issue
with exploit details.

Include the affected version, platform, reproduction steps and expected impact.
Use a minimal sample repository with no secrets or private data.

## Scope

symapper reads untrusted repositories. Relevant security issues include:

- Path traversal or reading files outside the target work tree.
- Symlinks bypassing tracked-file or git-directory boundaries.
- Execution of repository code, git configuration helpers or hooks.
- Cache tampering that causes unsafe reads, writes or execution.

The target repository must remain read-only except for its tag cache and the
temporary file used for atomic cache writes. See ARCHITECTURE.md for invariants.
