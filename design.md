# Desktop TUI design

## Product direction

Desktop TUI is a reusable transparent terminal canvas for Plasma 6, not a
single desktop theme. A user can place one or more independent instances on any
monitor and compose each instance from a data-driven template.

The visual baseline is intentionally restrained: no applet chrome, no forced
wallpaper, no input prompt, and no controls painted over the canvas. Content
comes from the selected template and the active KDE color scheme.

## Instance model

Plasma KConfig owns settings per applet instance:

- canvas source: built-in renderer or an external command;
- template ID or custom template path;
- optional model override;
- frame rate, animation, and FPS-counter visibility;
- command, exact arguments, working directory, environment, and lifecycle;
- optional terminal text selection; and
- terminal font family, size, and line spacing.

Each instance starts its own renderer and writes to a shared-frame file named
from its containment and applet IDs. This prevents instances on different
monitors from overwriting each other.

The terminal grid is derived from the applet's actual size when its renderer
starts, avoiding a fixed dependency on one monitor resolution. Configuration
changes reload only the affected instance.

## Interaction

The terminal stays unfocused and has no typing surface or renderer controls.
Mouse input is disabled by default, preserving Plasma's normal pointer and
desktop context menu. Command canvases can opt into left-button terminal mouse
events for clickable TUIs. The cursor remains a pointer, while middle- and
right-click remain unclaimed.

Configuration uses native Plasma/Kirigami form controls. Settings are applied
through the normal applet configuration window and restart only that instance.

## Templates

Templates are strict JSON manifests. They control:

- whether a model and/or system information is rendered;
- model asset, camera, scale, crop, lighting, animation, and texture filtering;
- system-information width, position, alignment, and visible sections; and
- outer terminal margin.

Built-in manifests are regular files in `renderer/templates/`. A custom
file can live anywhere and can point at an absolute GLB path. Unknown fields are
rejected so misspelled settings fail visibly instead of being silently ignored.
Numeric values are bounded before reaching the renderer.

## Renderer

The Rust process loads only the resources required by the selected template.
System-only templates skip mesh and texture loading entirely.

For model templates, the consuming prepared-mesh path flattens source polygons
into compact `u32` topology and releases the allocation-heavy face graph.
Animation geometry, skinning, keyframes, materials, and required textures remain
resident because they are needed for subsequent frames. Projected vertices,
depth cells, deferred shaders, animation matrices, and skinned geometry reuse
bounded scratch buffers after warm-up.

QML does not reconstruct or recolor cells. Ratatui produces glyphs and ANSI
truecolor at the terminal's exact row and column count. The patched bundled
QMLTermWidget preserves alpha in the color scheme and image render target so
Plasma composites the canvas over the desktop.

## Command canvases

Command canvases use the same terminal presentation but bypass the shared-frame
renderer. The installed Rust binary acts as a small process supervisor and
launches one explicit program directly in the PTY. Arguments retain their exact
boundaries; no shell parses strings or expands variables, globs, pipes, or
redirections.

The supervisor owns one-shot, interval, timeout, and restart behavior. Rapid
failures use bounded exponential backoff. Commands are local KConfig settings,
not template fields, so importing a visual JSON manifest cannot execute code.
The full contract and threat model are in
[`docs/command-canvases.md`](docs/command-canvases.md).

## Color

The renderer follows the active KDE color scheme and checks it once per second.
Semantic foreground, selection, inactive, hover, and complementary roles map to
the information hierarchy and untextured model fallback. Templates using
embedded textures retain their authored colors.

The terminal color scheme controls transparency only. It does not override
Ratatui foreground colors.

## Installation boundaries

The installer owns only the renderer, templates, example asset, bundled terminal
module, and Plasma applet package. It never modifies wallpaper, panel, activity,
or containment configuration.
