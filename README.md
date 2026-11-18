<h1 align="center">symapper</h1>

<p align="center">
A ranked map of the code that matters for your task, within a token budget.
</p>

<p align="center">
<a href="https://www.npmjs.com/package/symapper"><img alt="npm" src="https://img.shields.io/npm/v/symapper"></a>
<a href="https://www.npmjs.com/package/symapper"><img alt="npm downloads" src="https://img.shields.io/npm/dm/symapper"></a>
<a href="https://github.com/viewtools-app/symapper/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/viewtools-app/symapper/actions/workflows/ci.yml/badge.svg"></a>
<a href="LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/License-Apache--2.0-blue.svg"></a>
</p>

<p align="center">
<a href="https://symapper.viewtools.app/">Website</a> |
<a href="https://symapper.viewtools.app/docs.html">Docs</a> |
<a href="#installation">Install</a> |
<a href="#contributing">Contributing</a>
</p>

<p align="center">
<img alt="symapper mapping a repository for a task, then a grep to verify" src="site/media/demo.gif">
</p>

symapper is a small local CLI that prints a ranked map of the code that matters for a task:
signatures of the most relevant definitions and the files that use them, fitted into a token
budget. It is made for coding agents and for humans who want to orient in an unfamiliar repo
before reading files one by one. No LLM, no daemon, and no network requests while mapping
(`npx` downloads the package on first use).

## Features

- Personalized PageRank over a file graph, pulled toward your `--focus` paths and identifiers.
- Fixed token budget (`--tokens`, default 1500), so the output always fits a prompt or a brief.
- Six languages: TypeScript, JavaScript, Python, Go, Rust, Swift (tree-sitter).
- Git-aware and read-only: maps tracked files and never runs repository code or hooks.
- Tag cache for fast repeated runs; `--no-cache` turns it off.
- Single static binary on Linux, also installable through `npx`.

## Quick start

```sh
npx symapper .                                  # map the current repo
npx symapper . --focus cache                    # pull the ranking toward a word or path
npx symapper . --focus cache --tokens 120       # smaller budget
```

Sample output for `symapper . --focus cache --tokens 160` on this repository (first lines). With
`--focus`, `path:` blocks with the best-ranked signatures come first, followed by key repository
files (README, LICENSE, manifests, CI workflows):

```text
src/render.rs:
impl CachedSource {
    fn new(text: String) -> Self {
...
    impl Fixture {
        fn new(paths: &[&str]) -> Self {
src/cache.rs:
pub fn cache_path(common_dir: &Path, env_value: Option<&OsStr>) -> Result<PathBuf, String> {
tests/pipeline.rs:
fn cache_file(repo: &Fixture) -> PathBuf {
src/lib.rs:
pub fn map(opts: &Options) -> Result<MapOutput, Error> {
tests/map.rs:
fn python_go_swift() -> Fixture {
tests/common/mod.rs:
pub fn run_bin_with_ranking(
```

## Installation

**npm** (Node 18 or newer):

```sh
npx symapper <repo>      # run without installing
npm i -g symapper        # or install the command globally
```

Prebuilt binaries: macOS arm64 and x64, Linux x64 and arm64 (static musl). Windows is not
supported; use the Linux package in a Linux environment. Mapping needs git.

**GitHub Releases:** download the archive for your platform and `SHA256SUMS` from
[Releases](https://github.com/viewtools-app/symapper/releases), then verify and extract:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing    # macOS (Linux: sha256sum -c ...)
```

**From source** (Rust 1.90 or newer, git and a C compiler; not on crates.io):

```sh
cargo install --git https://github.com/viewtools-app/symapper --tag v0.2.0 --locked
cargo install --path . --locked    # from a clone
```

## Usage

```text
symapper <repo> [--focus <path|ident>] [--tokens N] [--exclude <glob>] [--no-cache]
```

| Flag | Meaning |
|---|---|
| `--focus <path\|ident>` | Repeatable path or identifier that personalizes ranking |
| `--tokens N` | Positive token budget; default 1500 |
| `--exclude <glob>` | Repeatable glob matching repository-relative paths |
| `--no-cache` | Disable cache reads and writes |
| `-V, --version` | Print the version |
| `-h, --help` | Print help |

```sh
symapper ./example-repo --focus src/lib.rs --focus calculate_total --tokens 1000
symapper ./example-repo --exclude 'tests/**' --exclude 'vendor/**' --no-cache
symapper ./example-repo --tokens 4000      # orient in a new repository
```

Use 500-1500 tokens for a task brief, or 3000-4000 to orient in a new repository. Exit codes:
0 success, 1 runtime or I/O failure, 2 invalid arguments or not a git work tree. Focus matching,
ranking and output layout are described in the [docs](https://symapper.viewtools.app/docs.html).

## Using with coding agents

Run symapper once at the start of a task for a ranked overview instead of many search-and-read
steps: pick focus words from the task, run
`npx symapper . --focus <word> --focus <word> --tokens 1500`, read the top files, then still
search the task's identifiers with `rg` or `grep` before editing. The map is a starting point, not
a replacement for search: static parsing misses dynamic calls, re-exports and macros.

To automate it, paste this into an agent instruction file such as `AGENTS.md`:

```text
Before exploring the repository, run:
npx symapper . --tokens 1500 --focus <word from the task> --focus <another word>
Use words and identifiers from the task as --focus. The map is a starting point,
not the answer: read what it points to, then still search the code (rg/grep) for
the task's words. The map can miss files.
```

## How it works

1. Tracked source files are listed from git (binary and oversized files are skipped).
2. Tree-sitter finds definitions and references.
3. They form a file graph: a file points to the files that define the names it uses.
4. Personalized PageRank ranks the graph, pulled toward your focus: file paths, identifiers, and
   words found in file contents.
5. Files that match the focus come first, each with its best-ranked definition.
6. Signatures are fitted into the token budget and printed as the map.

The method follows [Aider's repository map](https://aider.chat/2023/10/22/repomap.html),
rewritten in Rust with original queries.

## Supported languages

| Language | Extensions |
|---|---|
| TypeScript | `.ts`, `.mts`, `.cts`, `.tsx` |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` |
| Python | `.py`, `.pyi` |
| Go | `.go` |
| Rust | `.rs` |
| Swift | `.swift` |

Extensions are case-sensitive.

## Cache and safety

- The target repository is read-only except for the cache file and its temporary file.
- No repository code, hooks or git configuration commands run: git runs only `rev-parse` and
  `ls-files`, with the fsmonitor hook disabled.
- Tags are cached at `<git common dir>/symapper-cache.bin`. Set `SYMAPPER_CACHE_DIR` to an
  absolute path of an existing directory to use it instead; `--no-cache` disables the cache.
- Only tracked files are considered. Oversized (over 1 MiB) and binary files are skipped.

## Limits

Static tags miss dynamic calls, re-exports and macros, and can join unrelated same-name symbols.
Token counts are estimates (Unicode characters / 4), not model-specific counts. Only tracked files
in supported languages are mapped. Linked worktrees share the cache and can reuse stale tags when
path, size and mtime coincide; use `--no-cache` for a fresh run.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development checks and adding a language.

## Security

See [SECURITY.md](SECURITY.md) for private vulnerability reporting.

## Acknowledgements

- [Aider](https://aider.chat/2023/10/22/repomap.html): the repository map idea (a ranked graph of
  definitions and references, fitted to a token budget). symapper is a separate Rust
  implementation; no Aider code or query text is included.
- [tree-sitter](https://tree-sitter.github.io/) and its grammars: parsing.
- PageRank: L. Page, S. Brin, R. Motwani, T. Winograd, "The PageRank Citation Ranking: Bringing
  Order to the Web", Stanford InfoLab, 1999. Personalized ranking follows T. Haveliwala,
  "Topic-Sensitive PageRank", WWW 2002.

## License

[Apache-2.0](LICENSE). Dependency licenses are listed in [NOTICE](NOTICE) and
[THIRD-PARTY-LICENSES](THIRD-PARTY-LICENSES).
