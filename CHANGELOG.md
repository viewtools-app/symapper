# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.0] - 2026-11-18

### Changed
- With `--focus`, every file the 0.1.0 ranking prints at the same budget stays in the map,
  as a name line when its signatures do not fit, so focus ordering no longer drops files
  the 0.1.0 ranking showed.
- With `--focus`, root files (README, LICENSE, manifests and CI workflows) follow the
  definition blocks. A root that is also a source file appears once at its ranked position.
- Focus scoring weighs matches by kind: definition or file name above directory above body
  text, and by the term's rarity among file names. Words no file defines, such as "fix",
  count little.

### Added
- Held-out task set `bench/tasks-held2.tsv`.
- Benchmark harness top-N metrics and `--summary <out-dir>` for saved runs.
- Held-out task set `bench/tasks-held.tsv`.

## [0.1.0] - 2026-11-17

### Added
- Focus words also match file contents; matched files are listed first.
- npm package `symapper` with prebuilt binaries for macOS and Linux.
- Release archives for each platform with `SHA256SUMS` checksums.
- Local code maps for TypeScript, TSX, JavaScript/JSX, Python, Go, Rust and Swift.
- Ranked definition lines with enclosing scope headers and a ranked file list.
- Repeatable focus options for paths and identifiers.
- Configurable token budget with a default of 1500 estimated tokens.
- Tag cache for unchanged files in the git common directory or `SYMAPPER_CACHE_DIR`;
  `--no-cache` disables cache reads and writes.
- Repeatable exclusion globs for repository-relative paths.
