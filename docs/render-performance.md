# Renderer performance

Desktop TUI ships a standalone benchmark for model loading, prepared-mesh
construction, animation, projection, rasterization, terminal diffing, memory,
and steady-state allocations.

## Reproduce the reference run

```bash
cargo run --release --package desktop-tui \
  --example render_benchmark -- \
  renderer/assets/fox.glb 180 52 200
```

Reference machine:

- Intel Core i5-14600KF;
- Rust stable;
- included Khronos Fox GLB;
- 180×52 Ratatui grid;
- bilinear texture filtering;
- backface culling enabled; and
- 200 measured animated frames after warm-up.

Observed result:

| Measurement | Result |
| --- | ---: |
| Model load | 6.64 ms |
| Prepared topology | 0.33 ms |
| Live renderer RSS at 15 FPS | 7.3 MiB |
| Full Ratatui frame | 0.28 ms |
| Direct renderer | 0.25 ms |
| Direct-render throughput | about 3,948 FPS |
| Direct-render allocations | 0/frame |

Numbers are workload-specific. A large character model with several high
resolution textures will use substantially more memory than the 159 KiB Fox.
The Plasma Shell process and Qt scene graph are not included in standalone
renderer RSS.

## Useful benchmark controls

| Environment variable | Effect |
| --- | --- |
| `DESKTOP_TUI_BENCH_STATIC=1` | Disable animation |
| `DESKTOP_TUI_BENCH_NEAREST=1` | Use nearest-neighbor textures |
| `DESKTOP_TUI_BENCH_NO_CULL=1` | Disable backface culling |
| `DESKTOP_TUI_BENCH_TEXTURE_LIMIT=256` | Resize textures before preparation |
| `DESKTOP_TUI_BENCH_LOOP_BLEND=0.2` | Blend the animation loop seam |
| `DESKTOP_TUI_BENCH_PROFILE_FRAMES=20` | Increase profiled frame count |
| `DESKTOP_TUI_BENCH_TRACE_FRAMES=60` | Print per-frame phase timings |
| `DESKTOP_TUI_BENCH_DUMP_PPM=/tmp/frame.ppm` | Export the final buffer |
| `DESKTOP_TUI_BENCH_REQUIRE_RENDER_BUDGET=1` | Fail above 8 ms/direct frame |
| `DESKTOP_TUI_BENCH_REQUIRE_BUDGET=1` | Fail above 8 ms/full frame |

For a deterministic quality gate:

```bash
DESKTOP_TUI_BENCH_STATIC=1 \
DESKTOP_TUI_BENCH_REQUIRE_BUDGET=1 \
DESKTOP_TUI_BENCH_REQUIRE_RENDER_BUDGET=1 \
cargo run --release --package desktop-tui \
  --example render_benchmark -- \
  renderer/assets/fox.glb 180 52 200
```

## Why the renderer stays small

- Source face graphs are flattened into compact prepared topology.
- Animation matrices, skinned geometry, projected vertices, depth cells, and
  deferred shader data reuse warm buffers.
- Opaque geometry records the winning fragment and shades each visible
  terminal cell once.
- Texture opacity and emissive data are prepared once per texture.
- ASCII glyph ramps use direct byte indexing.
- Static model instances redraw at 1 FPS.
- System-only templates never load the model stack.
