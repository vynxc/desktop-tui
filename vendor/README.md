# Vendored dependencies

Desktop TUI keeps two patched dependencies in-tree so installation is
reproducible and does not require system-wide custom libraries.

- `ratatui-3dmesh/` is a source snapshot of
  [vynxc/ratatui-3dmesh](https://github.com/vynxc/ratatui-3dmesh), including
  the compact prepared-mesh path and renderer performance work used by the
  desktop process. It is MIT licensed.
- `qmltermwidget/` is based on
  [Swordfish90/qmltermwidget](https://github.com/Swordfish90/qmltermwidget) at
  upstream commit `8913504`, with Qt 6, transparent painting, shared-frame,
  and desktop mouse-pass-through changes. It is GPL-2.0-or-later with some
  compatibly licensed inherited files.

See [THIRD_PARTY.md](../THIRD_PARTY.md) and each directory's license files.
