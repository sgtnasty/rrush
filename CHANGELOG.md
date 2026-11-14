# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-11-14

### Security

- Updated dependencies with `cargo update` to resolve `cargo audit` findings:
  - `quick-xml` 0.39.4 → 0.41.0 (RUSTSEC-2026-0194, RUSTSEC-2026-0195)
  - `event-listener` 5.4.1 → 5.4.2 (RUSTSEC-2026-0221)
  - `rustix` 1.1.4 → 1.1.6 (RUSTSEC-2026-0486)
  - `async-executor` 1.14.0 → 1.14.1 (yanked release)
  - `bevy` 0.19.0 → 0.19.1
- Added `.cargo/audit.toml` to ignore two advisories that have no fix on
  Bevy 0.19, with the reasons recorded in that file:
  - `ruzstd` 0.8.3 (RUSTSEC-2026-0344): only decodes Bevy's embedded
    tonemapping LUTs in this app. Goes away with Bevy 0.20.
  - `ttf-parser` 0.25.1 (RUSTSEC-2026-0192): unmaintained, informational.

## [0.2.0]

### Changed

- Refactored `main.rs` into focused modules.
- Added `RRUSH_SPEED` override for smooth frame capture.
- Updated README with module layout, dipole GIF and Mermaid diagrams.
- CI: install Wayland and libudev dependencies.

## [0.1.0]

### Added

- Initial hydrogen atom visualiser in Bevy.
- `Q` shortcut to quit the simulation.
- GitHub Actions workflow for the Rust project.
