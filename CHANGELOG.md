# Changelog

All notable changes to Desktop TUI will be documented here.

## [Unreleased]

## [0.2.0] - 2026-07-29

- Added command canvases for rendering existing CLI and TUI programs in the
  widget's real PTY.
- Added exact argument passing, validated environment additions, optional
  working directories and timeouts, interval refreshes, and bounded restart
  backoff.
- Added per-instance renderer/command source controls and applet contract tests.

## [0.1.0] - 2026-07-29

- Initial public release.
- Transparent Ratatui rendering inside a Plasma 6 applet.
- Per-instance templates, GLB overrides, FPS, animation, font, and mouse
  settings.
- Five built-in model and system-information layouts.
- Multi-monitor-safe shared frames.
- Neutral Khronos Fox sample model with documented redistribution terms.
- User-local installer, uninstaller, environment doctor, and CI workflow.

[Unreleased]: https://github.com/vynxc/desktop-tui/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/vynxc/desktop-tui/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/vynxc/desktop-tui/releases/tag/v0.1.0
