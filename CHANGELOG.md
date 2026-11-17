# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
