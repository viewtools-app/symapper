# Architecture

<!-- Internals and invariants for contributors. -->

Last reviewed: 2026-11-17 (focus content matching)
Project type: rust-cli (local-tool)

## Purpose and scope
A local CLI that prints a ranked, token-budgeted map of a git repo: signatures of the most relevant
definitions plus the files that use them, so a coding agent does not spend a task reading files.
Method from Aider's repo map, rewritten in Rust.

What symapper is NOT: an MCP server, a daemon, an embeddings index, an LSP client, a knowledge graph,
or anything that edits code or calls an LLM or the network.

## Stack and entry points
Rust (edition 2024, MSRV 1.90), one package with a library and a thin binary. Crates (exact pins
in `Cargo.toml`): `tree-sitter` plus one grammar
crate per language (TypeScript/TSX, JavaScript/JSX, Python, Go, Rust, Swift), `rayon` for parallel
parsing, `clap` for arguments, `globset` for excludes; `tempfile` for tests. Entry point:
`src/main.rs` calls `symapper::cli::main()`; build `cargo build`, install
`cargo install --path . --locked`.

## Code map
Source, queries, integration tests and project files:
```
site/                   static landing page, reference, styles and favicon
DESIGN.md               website purpose, visual tokens and accessibility rules
NOTICE                  method and dependency credits
CHANGELOG.md            release history
CONTRIBUTING.md         contributor setup and checks
SECURITY.md             private vulnerability reporting
.github/                CI, Pages and release workflows, contribution templates
npm/symapper/           dependency-free Node launcher, package metadata and built-in tests
scripts/bench-recall.sh  recall benchmark: symapper vs grep baseline over a task list
scripts/npm-stage.sh    generate versioned launcher and platform packages from binaries
scripts/npm-smoke.sh    offline host packaging, install and fixture-map smoke check
scripts/release-archive.sh  create platform archive and staged binary layout
scripts/npm-publish.sh  publish platform packages before launcher, skip existing versions
src/main.rs             binary entry point
src/lib.rs              options, errors, collect/cache pipeline, map and source reader
src/cli.rs              clap arguments, cache environment option, validation and exit codes
src/git.rs              hardened git commands, root/common dir and tracked paths
src/files.rs            tracked files, safety/content filters, languages and root files
src/lang.rs             Lang::ALL, extensions, grammars, embedded queries and scope kinds
src/tags.rs             per-thread parser, definition/reference tags and source reading
src/cache.rs            versioned tag cache, schema hash, checksum and atomic writes
src/graph.rs            weighted referencer/definer groups per identifier
src/rank.rs             personalized PageRank and definition ranks
src/focus.rs            focus paths, identifiers and personalization vector
src/render.rs           root paths, definition blocks, budget and name-only ranked files
queries/typescript.scm  TypeScript definitions and references
queries/tsx.scm         TSX definitions and references, including JSX components
queries/javascript.scm  JavaScript/JSX definitions and references
queries/python.scm      Python definitions and references
queries/go.scm          Go definitions and references
queries/rust.scm        Rust definitions and references
queries/swift.scm       Swift definitions and references
tests/common/mod.rs     isolated temporary git fixture and binary helpers
tests/cli.rs            help/version, argument errors and repo validation
tests/files.rs          tracked paths, filters, root files and hostile input safety
tests/focus.rs          path forms, identifiers, personalization and symlink focus
tests/graph_rank.rs     group graph equals pairwise reference; linear shared-name storage
tests/export_public.rs  export allowlist, denylist, destination and git safety
tests/pipeline.rs       cache options, reuse/rebuild, exclusions, read failures and write safety
tests/golden/           no-focus and published v010 output goldens
tests/map.rs            multilingual map, budget, focus, determinism and name-only output
tests/tags_ts.rs        exact TypeScript/TSX tags, scopes and syntax-error recovery
tests/tags_js.rs        exact JavaScript/JSX tags and syntax-error recovery
tests/tags_python.rs    exact Python tags, decorators, assignments and parser switching
tests/tags_go.rs        exact Go tags, aliases, nested types and syntax-error recovery
tests/tags_rust.rs      exact Rust tags, scopes, file I/O and all-grammars-load test
tests/tags_swift.rs     exact Swift tags, scopes, annotations and syntax-error recovery
```
Only `git.rs` spawns processes. `files.rs`/`tags.rs` read target source files; `lib.rs`
also reads mapped sources through `tags::read_text` for focus content matching, only when
focus has content terms, and passes per-file match sets to `focus`. Failed content reads are
not retried for render. `cache.rs` reads and writes the cache. `lib.rs` passes a source reader
to render using the filtered source list. `graph`, `rank`, `focus` and `render` work on
in-memory data, except that focus canonicalizes the repo root/current directory and render
calls the supplied reader.

## Invariants and traps
1. Read-only on the target repo except the cache file. Enforced by
   `tests/pipeline.rs::map_changes_only_cache_in_git_dir`.
2. Never executes repo code, hooks or git configuration commands. Every git call uses
   `-c core.fsmonitor=false -c core.untrackedCache=false --no-optional-locks`, null stdin,
   and removes `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`,
   `GIT_COMMON_DIR`, `GIT_CONFIG_PARAMETERS`, `GIT_EXTERNAL_DIFF`. Only calls:
   `rev-parse --show-toplevel --git-common-dir` and `ls-files -z`; no diff or textconv.
   Git still reads repository configuration; the hardening prevents executing its helpers.
   Enforced by `git_removes_repo_override_environment` in `src/git.rs` and
   `hostile_fsmonitor_never_runs` / `git_dir_environment_is_ignored_by_binary` in `tests/files.rs`.
3. A path is read only if its canonical target is a tracked path inside the work tree and not
   under the git common dir. Excludes apply to the entry's own path. Non-UTF-8 paths are skipped;
   source files over 1,048,576 bytes or containing NUL in the first 8 KiB are skipped.
   Parsing, focus content and rendering reads are capped at 1,048,577 bytes and reject sources
   over the limit even when a source grows after listing.
   Unreadable or vanished files are skipped with a stderr note. Safe symlinks retain their own
   path. Enforced by `unsafe_symlink_targets_are_skipped`, `inside_symlink_keeps_own_path`,
   `outside_symlink_is_skipped`, `untracked_file_is_not_listed`, `non_utf8_path_is_skipped`,
   `oversized_file_is_skipped`, `nul_in_first_eight_kib_is_skipped` in `tests/files.rs`, and
   `unreadable_tracked_file_is_skipped_without_aborting_map` and
   `unreadable_file_with_content_focus_emits_exactly_one_note` in `tests/pipeline.rs`.
4. Map stdout holds repo text and relative paths only; no absolute paths or environment data.
   Diagnostic stderr may include paths from I/O errors.
5. No network, no LLM. Deterministic output for the same repo state and arguments; rank ties
   break by path then identifier. Name-only files sort by descending focus score, then
   descending file rank, then path.
6. Grammar crates are pinned to versions compatible with the pinned `tree-sitter` crate; bump
   together. Trap: a mismatched grammar ABI fails at runtime, not at compile time.
7. The 0.1.0 floor and `SYMAPPER_RANKING=v010` share `words`, `term_matches`,
   `COMMON_SHARE`, `resolve`, `graph.rs` identifier weights, `rank.rs` constants,
   `MATCHED_DEFS_PER_FILE` and the render format with current ranking. Changing any of
   these moves both away from published 0.1.0. The v010 goldens catch drift only on
   their fixtures; changes to shared code require broader published-output checks.

## Data and state
One cache file per target repo: `<git common dir>/symapper-cache.bin` by default when
`SYMAPPER_CACHE_DIR` is unset or empty. If set to an absolute path of an existing directory,
the cache is `<dir>/symapper-<hash>.bin`, with 16 lowercase hex digits of FNV-1a-64 over the
canonical git common directory path bytes. Repositories can share this directory without
sharing cache files; symapper never creates it. A relative or non-directory value emits one
`symapper: SYMAPPER_CACHE_DIR ignored: <reason>` note and disables cache reads and writes for
that run, without falling back to the git directory. The CLI reads the environment once
and passes it through `Options::cache_dir`; library callers supply this option explicitly
(`None` uses the default location). `cache_path` accepts the value explicitly.
`--no-cache` disables both reads and writes.
The test-only `SYMAPPER_RANKING=v010` switch selects the frozen 0.1.0 ranking pipeline;
unset selects the default, and any other value is a usage error. The CLI passes it through
`Options::ranking_v010`; it is intentionally absent from help.
The cache is only read from a regular file up to 256 MiB, with a capped read and an opened-handle
identity check on Unix. The metadata check and open are not atomic; concurrent replacement with
a FIFO can still block the open. Atomic save replaces final-path symlinks without following them.
Entries hold tags keyed by relative path, nanosecond
mtime and size. Format: magic `RMAPCACHE`, little-endian format version (1), schema hash, entry
count and entries, then an FNV-1a-64 checksum over all preceding bytes. Strings are length-prefixed
UTF-8; entries include tag names, kinds, rows and enclosing scope rows.

The schema hash covers the crate version, `tags::TAGS_VERSION`, and every embedded query and
scope-kind list through `Lang::ALL` / `Lang::query_source()`. Invalid format, version, schema or
checksum discards the cache, prints one stderr note and rebuilds. Missing cache is normal.
Write only when parsing was attempted or entries disappeared; excluded entries are retained
without parsing, and gone entries are dropped. Atomic write: create `<cache>.tmp-<pid>` without
clobbering an existing temp file, write, `sync_all`, rename. Failed writes warn on stderr and do
not fail the map. Concurrent writers: last rename wins. Losing the cache costs one cold run.
Linked worktrees share it: equal relative path, size and nanosecond mtime with different contents
can reuse stale tags (accepted cache risk). Nothing else is stored.

## Key flows
1. Map: resolve repo -> list/filter tracked files -> load cache -> parse misses in parallel ->
   save changed cache -> resolve focus -> build file graph -> read mapped content when focus
   has content terms -> compute personalization -> personalized PageRank -> split rank into
   (file, identifier) definitions -> promote matched definitions -> render.
   The hidden v010 switch instead uses `personalization_v010` (the 0.1.0 scoring and its
   independent `CONTENT_WEIGHT_V010 = 0.75`), the same rank functions and cap 1, and roots first.
   With focus, the default pipeline first runs that v010 pipeline at the same token budget,
   retaining its printed graph file set as the recall floor. Without focus, only the default
   pipeline runs; the v010 switch runs only v010 and never applies a floor. One source cache in
   `map`, keyed by graph file index, is passed to every `render_with_files` call, memoizing
   successful and failed render reads across the legacy, plain and floor passes.
   Successful entries also keep line start byte offsets, computed once for row lookup.
2. Focus first matches a mapped file by repo-relative path (including `./`), current-directory
   relative path or absolute path inside the repo, retaining a symlink's own path. A matched
   file also contributes its stem as an identifier. Other values are identifiers, including a
   directory path. Terms and names split on underscores, hyphens, dots, slashes, whitespace
   and case boundaries (SheetLayout -> sheet, layout; HTTPServer -> http, server), lowercase,
   and retain digits in their word. Matching requires a contiguous word run. Identifier edge
   multipliers use x10 for a case-insensitive whole-name match, otherwise x3 for a word-run
   match, never both; other multipliers are unchanged. Term words are prepared before matching
   tags. Definition names are indexed by path because graph paths are sorted and deduplicated.
   Explicit files score the number of terms plus one, above every other file.
   Focus terms also match file contents (identifiers, comments and strings). The content
   tokenizer splits on every non-alphanumeric character and uses the same case-boundary and
   lowercase rules. For content matching, terms use the same tokenizer as file text; a match
   is a contiguous word run across punctuation and lines. Route matching still uses `words`.
   Stems of explicit focus files never match content. A term matching content in more than
   half the mapped files (`COMMON_SHARE = 0.5`) gets no content weight; route matches still
   count. Per term, use the best kind: strong names get IDF, directories `PATH_WEIGHT=0.75` x IDF;
   IDF = ln((N+1)/(route_df+1))/ln(N+1), or 1 for N <= 1. Body-only gets `CONTENT_WEIGHT=0.5`
   x min(content IDF, route IDF) when route_df > 0, else x `CONTENT_FLAT=0.25`.
   Sum per-term scores; personalization is score / total score, uniform if all zero.
   A directory term such as push selects all files under push/. Explicit focus files are
   omitted from output; identifier matches are still printed. Before budget fitting, a
   matched-first definition prefix takes at most one definition per matched file
   (`MATCHED_DEFS_PER_FILE = 1`), ordered by descending focus score with existing rank order
   breaking ties. The remaining definitions keep their rank order. Without focus, no content
   is read and output is unchanged.
3. Render sorted root names, then `path:` blocks with scope headers, `...` gaps and source lines
   cut at 100 characters. With focus, non-source roots follow blocks; mapped roots appear only
   at their block or name-only position. Bisect ranked definitions, reserving printed roots, to fit
   the budget (Unicode characters / 4, aiming for 85-100% when enough content). Append remaining
   file names by descending focus score, then descending file rank, then path, skipping focus
   files and already printed paths, stopping at the first name that exceeds the budget. Trim
   root names from the end if needed. An empty `Input.floor` preserves this output byte for
   byte. With a floor, step 1 renders that plain output using the shared source cache and returns
   it unchanged if it already contains every required file (excluding explicit focus files).
   Otherwise step 2 reserves one bare path line for each floor file absent from the selected
   block prefix and printed roots. Trim sorted roots from the end until roots plus the full
   reserve fit, then bisect block prefixes against candidate characters plus reserve characters,
   dividing by four only once and retaining the 85% early break. This cuts signature blocks
   before losing floor files. In the name-only list, floor paths always print at their normal
   score/rank/path position, without a colon. Other paths must fit together with all floor lines
   still waiting; skip a non-fitting path while a floor line waits, otherwise stop as usual.
   The floor's bare paths fit because v010 printed them within the same budget; there is no
   fallback that drops floor files. `render_with_files` also returns the printed graph file
   indices, including blocks, name-only paths and mapped roots. `render` wraps it with an empty
   cache and returns only the text.
4. No successfully parsed supported files: empty stdout, `symapper: no supported source files`
   on stderr, exit 0. Successful maps also exit 0; runtime/I/O errors exit 1; bad arguments,
   invalid globs or a repo that is not a git work tree exit 2.

## Infrastructure and deploy
Profile: local-tool.
Supported prebuilt platforms: macOS arm64/x64 and Linux x64/arm64 (static musl). No Windows
prebuilt packages. Artifacts are generated, never committed. Distribution: `npx symapper <repo>`
or `npm i -g symapper` with Node 18+, GitHub Release archives with `SHA256SUMS`, or source:
`cargo install --git https://github.com/viewtools-app/symapper --locked` or
`cargo install --path . --locked` from a clone; version check `symapper --version`.
CI: GitHub Actions (`.github/workflows/ci.yml`). The website source is `site/`, deployed by
`.github/workflows/pages.yml`.
Packaging: `release-archive.sh` creates `symapper-<version>-<triple>.tar.gz` and the stage layout
`dist/<triple>/symapper`. `npm-stage.sh <version> <dist> <out>` requires the Cargo version,
skips missing target binaries, and generates the launcher plus platform packages
`@symapper-cli/darwin-arm64`, `@symapper-cli/darwin-x64`, `@symapper-cli/linux-x64` and
`@symapper-cli/linux-arm64`, written to `<out>/@symapper-cli/<os>-<cpu>` with `publishConfig.access` public.
Each platform package contains `bin/symapper`, LICENSE and NOTICE, with matching `os`/`cpu`
metadata. All optional platform dependencies use the launcher version; there are no install scripts.
The launcher resolves the platform package with `require.resolve`, runs its binary with inherited
stdio, forwards arguments and exit codes, and re-raises child signals. Unsupported platforms get
an error naming the platform and a source-install command. Missing packages get an error naming
the package, suggesting reinstalling without `--omit=optional` or installing from source.
Spawn failures report the binary path and error.

The release workflow runs on `v*` tags, checks the Cargo version, builds four native targets,
creates archives and `SHA256SUMS`, and creates the GitHub Release using CHANGELOG notes.
It stages npm packages and calls `npm-publish.sh`, publishing all four platform packages before
`symapper` and skipping existing versions only after every existing package passes a staged
versus registry integrity check. A mismatch aborts before any package is published. npm trusted
publishing uses GitHub Actions OIDC and provenance; maintainers configure the repository/workflow publisher on all five packages once.

Data / backup: cache only, not needed (rebuilt). Runtime exposure / auth / secrets: none.
Observability: exit code and stderr. Environments: one. Runtime external services: none, no cost.
Release distribution uses GitHub and npm.
Failure impact: agents lose the map and fall back to reading files; nothing else breaks.
Binary deployment: local installation. A published npm version cannot be republished. Roll back
by publishing a fixed higher version and using `npm deprecate` on the bad one; for local installs,
reinstall the previous version.

## Testing and verification
`cargo test` uses temporary fixture repos, never real machine repos. The Code map lists each
integration test file and its coverage. `tests/files.rs` covers tracked-only reads and safety;
`tests/pipeline.rs` covers cache hits, metadata invalidation, corruption, retained excluded entries,
nonfatal writes and unchanged target files; `tests/map.rs` covers the end-to-end multilingual map.
The six `tests/tags_*.rs` files cover seven grammars (TSX with TypeScript, JSX with JavaScript).
`tests/tags_rust.rs::all_grammars_load` checks parser/query compatibility for `Lang::ALL`.
Unit tests in `cache.rs` cover format corruption, length bounds, schema inputs and atomic writes;
`graph.rs` covers weights; `rank.rs` covers convergence, dangling mass and definition splitting;
`render.rs` covers formatting, Unicode, budgets, focus suppression and name-only ordering.
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `git diff --check`.
`node --test npm/symapper/test/` covers platform selection, resolution, diagnostics, child status
and signals without npm dependencies. `bash scripts/npm-smoke.sh` builds the host binary, stages
and packs the launcher and host package, installs local tarballs offline, checks the version and a
fixture map, and compares an error with the native binary. Set `SMOKE_CARGO_OFFLINE=1` to make
its Cargo build offline too. The CI npm job runs both checks on macOS and Linux.
`tests/map.rs::no_focus_output_matches_golden` guards unchanged no-focus output against
`tests/golden/`. `tests/map.rs::v010_output_matches_published_goldens` guards the v010
pipeline against published 0.1.0 fixture output. Its
`default_map_restores_a_legacy_path_missing_without_floor` test uses a temporary repo and
fixed budget where the empty-floor pipeline drops a legacy path, checking that default `map`
restores that path and keeps every legacy path within budget.
`scripts/bench-recall.sh` is the manual relevance benchmark against a grep
baseline; task lists stay private. Timing and relevance on real repositories remain manual checks.

## Where to make a change
| Task | Start in |
|---|---|
| Add a prebuilt platform | `npm/symapper/bin/symapper.js` (`packageFor`); `npm/symapper/package.json` (optional dependencies); `scripts/npm-stage.sh`; `scripts/npm-publish.sh`; `scripts/release-archive.sh`; `scripts/npm-smoke.sh`; `.github/workflows/release.yml` (matrix); `npm/symapper/test/launcher.test.js`, `stage.test.js` and `publish.test.js`; platform lists in `ARCHITECTURE.md` and `CONTRIBUTING.md` |
| A new language | Grammar crate pin; new query `.scm`; `Lang` variant, `ALL`, `from_path`, `language()`, `query_source()` and `scope_kinds()` in `lang.rs`; `query()` storage follows `ALL`; bump `tags::TAGS_VERSION`; add `tests/tags_<lang>.rs` and cover it in `all_grammars_load` |
| Tag extraction, extension map, scopes or grammar pins | `tags.rs` / `lang.rs`; bump `tags::TAGS_VERSION` |
| Ranking weights | `graph.rs::ident_mul`, `rank.rs` constants, `focus.rs::CONTENT_WEIGHT`, `focus.rs::COMMON_SHARE`, `focus.rs::PATH_WEIGHT`, `focus.rs::CONTENT_FLAT`, `rank.rs::MATCHED_DEFS_PER_FILE`; bound test: `focus::tests::match_strength_weights_obey_kind_bounds`; retune only with `scripts/bench-recall.sh` on a tuning set. Shared knobs (`words`, `term_matches`, `COMMON_SHARE`, `resolve`, identifier weights, rank constants, `MATCHED_DEFS_PER_FILE` and the render format) also move the 0.1.0 floor and `SYMAPPER_RANKING=v010` away from published 0.1.0; v010 goldens cover only their fixtures |
| Output format | `render.rs` |

## Decisions and known debt
Open risks: static tags miss dynamic calls, re-exports and macros, and
join same-name symbols; ranking weights need tuning against real tasks.
