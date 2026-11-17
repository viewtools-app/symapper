# Contributing

Contributions are welcome through issues and pull requests at
https://github.com/viewtools-app/symapper.
For vulnerabilities, follow [SECURITY.md](SECURITY.md) instead of opening an issue.

## Prerequisites and checks

Install Rust 1.90 or newer (the MSRV in Cargo.toml) and a C compiler for the
tree-sitter grammars, plus Node 18+ and npm for packaging checks. From a checkout, run:

```sh
cargo build --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
node --test npm/symapper/test/
bash scripts/npm-smoke.sh
```

Add fixture-based tests for behavior changes and bug fixes. Tests must create
temporary repositories and must not depend on repositories on your machine.

## Design rules

Read [ARCHITECTURE.md](ARCHITECTURE.md), especially its Invariants and traps section.

- Keep target repositories read-only except for the tag cache and its temporary file.
- Never execute target repository code, hooks or git configuration commands.
  Git reads configuration for discovery; helpers must remain disabled.
- Keep operation local: no network calls, LLM calls or background service.
- Preserve deterministic output for the same repository state and arguments.
  Break rank ties by path and identifier.
- Read source paths only when their canonical targets are tracked, inside the
  work tree and outside the git common directory.
- Keep grammar crates pinned to versions compatible with the pinned tree-sitter
  crate; update them together when necessary.

## Adding a language

1. Add a compatible, exactly pinned grammar crate in Cargo.toml.
2. Write an original queries/<lang>.scm file for definitions and references.
   Record attribution and license obligations in NOTICE for any adapted text.
3. Register the language, extensions, grammar, embedded query and scope kinds in
   src/lang.rs, including Lang::ALL.
4. Add tests/tags_<lang>.rs using temporary fixtures. Test definitions, references,
   scope headers and syntax-error recovery, and ensure all grammars load.
5. Update the supported-language documentation and run the checks above.

## Releasing

Bump `version` in `Cargo.toml`, refresh `Cargo.lock`, and update the matching CHANGELOG section.
Run the checks above, including `bash scripts/npm-smoke.sh`. Tag the matching version as
`vX.Y.Z` and push the tag when ready to publish. The `release` workflow checks the tag against
the Cargo version, builds all four targets, creates the GitHub Release with archives and
`SHA256SUMS`, then publishes to npm using trusted publishing. Platform packages publish before
the launcher; versions already published to npm are skipped only when their integrity matches
the staged package. A mismatch aborts before any package is published.

Use only plain `vX.Y.Z` tags, each higher than the last published version. Prereleases and
lower versions need an explicit npm dist-tag and are not supported by this release workflow.
Publish the `v0.1.0` tag before the site or README advertising `npx symapper` goes live on
the default branch: npm `latest` is a `0.0.0` placeholder until that release.

Before the first release, maintainers must ensure all five packages exist on npmjs.com and
configure their trusted publisher once: repository `viewtools-app/symapper`, workflow
`release.yml`. The packages are `symapper`, `@symapper-cli/darwin-arm64`, `@symapper-cli/darwin-x64`,
`@symapper-cli/linux-x64` and `@symapper-cli/linux-arm64`. Publishing uses GitHub Actions OIDC with
provenance and needs no stored npm token.

## Commits and pull requests

Keep each change focused. Use a concise, imperative commit subject describing
the result. Explain the problem, why the change helps and any behavior changes
in the pull request. Link related issues and list checks run with their results.
Update documentation when behavior changes, and call out limitations or checks
you could not run. Avoid unrelated formatting or dependency changes.

## License

symapper is licensed under the Apache License 2.0. Unless you state otherwise, any contribution you
submit for inclusion is licensed under Apache-2.0 as described in section 5 of the License, without
additional terms or conditions.
