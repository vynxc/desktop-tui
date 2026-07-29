# Template authoring

Desktop TUI templates are JSON files loaded when a widget instance starts.
Copy an included template from `renderer/templates/`, edit it, then select
**Custom template file** in the widget settings.

## Complete example

```json
{
  "name": "Model with right-side details",
  "description": "A custom layout for a secondary monitor.",
  "outer_margin": 2,
  "model": {
    "enabled": true,
    "asset": "/path/to/character.glb",
    "scale": 2.4,
    "pan": [-0.15, -0.3, 0.0],
    "rotation_degrees": [0.0, 180.0, 0.0],
    "flip_horizontal": false,
    "texture_filter": "nearest",
    "texture_lighting": true,
    "color_brightness": 0.9,
    "backface_culling": true,
    "light_direction": [0.55, 0.35, 1.0],
    "diffuse_light": 0.55,
    "ambient_light": 0.45,
    "animation_index": 0,
    "animation_speed": 1.0,
    "animation_loop_blend_seconds": 0.25
  },
  "system": {
    "enabled": true,
    "width_percent": 32,
    "horizontal_position": "right",
    "horizontal_alignment": "left",
    "vertical_alignment": "center",
    "sections": ["SYSTEM", "HARDWARE", "NETWORK"]
  }
}
```

Relative model paths are resolved inside the installed asset directory.
Absolute paths can point anywhere readable by the user. The **Model override**
widget setting takes precedence over `model.asset`.

Desktop TUI supports binary `.glb` and JSON `.gltf` assets. Embedded or linked
PNG, JPEG, and WebP textures are decoded, then limited to 512 pixels on their
longest side by default. Set `DESKTOP_TUI_MAX_TEXTURE_DIMENSION` to change that
limit.

## Fields

### Root

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | string | Human-readable template name |
| `description` | string | Short authoring note |
| `outer_margin` | integer | Empty terminal rows above and below content |
| `model` | object | Model renderer configuration |
| `system` | object | System-information configuration |

### Model

| Field | Type | Default |
| --- | --- | --- |
| `enabled` | boolean | `false` |
| `asset` | string | empty |
| `scale` | number | `1.0` |
| `pan` | three numbers | `[0, 0, 0]` |
| `rotation_degrees` | three numbers | `[0, 0, 0]` |
| `flip_horizontal` | boolean | `false` |
| `texture_filter` | `nearest` or `bilinear` | `nearest` |
| `texture_lighting` | boolean | `true` |
| `color_brightness` | number | `1.0` |
| `backface_culling` | boolean | `true` |
| `light_direction` | three numbers | `[0.55, 0.35, 1]` |
| `diffuse_light` | number from 0 to 1 | `0.65` |
| `ambient_light` | number from 0 to 1 | `0.35` |
| `animation_index` | non-negative integer | `0` |
| `animation_speed` | number | `1.0` |
| `animation_loop_blend_seconds` | number | `0.35` |

### System information

| Field | Type | Default |
| --- | --- | --- |
| `enabled` | boolean | `true` |
| `width_percent` | integer from 10 to 100 | `70` |
| `horizontal_position` | `left`, `center`, or `right` | `center` |
| `horizontal_alignment` | `left`, `center`, or `right` | `center` |
| `vertical_alignment` | `top`, `center`, or `bottom` | `center` |
| `sections` | string array | all sections |

Available sections are `SYSTEM`, `DESKTOP`, `HARDWARE`, `STORAGE`, and
`NETWORK`. An empty array displays every section that fits.

Unknown fields are rejected. Invalid numeric values are clamped to safe renderer
ranges.

## Runtime overrides

The Plasma applet supplies these automatically, but they are useful when
running the renderer directly:

| Variable | Meaning |
| --- | --- |
| `DESKTOP_TUI_TEMPLATE` | Built-in template ID |
| `DESKTOP_TUI_TEMPLATE_FILE` | Absolute custom manifest path |
| `DESKTOP_TUI_MODEL_PATH` | Model path overriding `model.asset` |
| `DESKTOP_TUI_FPS` | Requested frame rate from 1 to 60 |
| `DESKTOP_TUI_ANIMATE_MODEL` | `0` disables animation and drops to 1 FPS |
| `DESKTOP_TUI_SHOW_FPS` | `1` draws the renderer FPS counter |
| `DESKTOP_TUI_USERNAME` | Optional identity-label username override |
| `DESKTOP_TUI_HOSTNAME` | Optional identity-label hostname override |
