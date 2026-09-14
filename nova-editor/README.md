# nova-editor

`nova-editor` is the visual tooling layer for the Nova engine. It provides a docking-based editor interface built on egui, with a viewport that renders the 2D scene into an off-screen texture and displays it inside the GUI.

## Current Scope

- **Docking layout** via `egui-dock` — tabbed panels (Viewport, Inspector) with splitters and rearranging.
- **Off-screen scene rendering** — the 2D scene renders into a `TextureRenderTarget`, then the texture is registered with egui and displayed in the Viewport panel via `ui.image()`.
- **Animated demo scene** — orbiting rectangles, pulsing circles, bouncing shapes — all driven by a `Clock`.
- **egui integration** at the `nova-core` level — event forwarding, render pass ordering, and texture registration are handled by the engine, not the editor.

## Usage

```bash
cargo run --release -p nova-editor
```

The editor window opens with two docked panels:
- **Viewport** — displays the rendered 2D scene (animated shapes on a dark background).
- **Inspector** — placeholder for future property editing.

## Architecture

```
on_init:
  1. Create TextureRenderTarget (800×600, surface format)
  2. Register texture with egui renderer → EguiTextureHandle
  3. Load default sprite + circle materials

on_render:
  1. Create RenderTarget from TextureRenderTarget
  2. Render 2D scene (sprites + circles) into the texture
  3. RenderTarget drops → encoder submitted

on_gui:
  1. DockArea displays the viewport panel
  2. Viewport tab shows the scene texture via ui.image(texture_id)
  3. Inspector tab shows placeholder
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `nova` (with `egui` feature) | Engine facade — core + 2D + egui |
| `egui-dock` | Docking layout system |

## Planned Scope

### Short Term

- **Scene viewport controls** — pan, zoom, and orbit the camera with mouse input (depends on nova-core input system).
- **Inspector panel** — display and edit shape properties (position, scale, color, z-index) via egui widgets.
- **Entity list / hierarchy panel** — list all shapes in the scene, select and highlight them.
- **Add/remove entities** — buttons to add shapes (rectangle, circle) to the scene and remove selected ones.

### Medium Term

- **Scene serialization** — save/load scenes to files (depends on nova-core asset pipeline).
- **Material editor** — pick textures, adjust tint, change blend modes visually.
- **Gizmos** — on-canvas translation/rotation/scale handles for selected entities.
- **Performance overlay** — FPS, draw call count, instance count, batch count.

### Long Term

- **Plugin system** — editor extensions loaded at runtime (custom panels, inspectors).
- **Asset browser** — file tree for textures, materials, scenes.
- **Multi-viewport** — multiple scene views (2D + future 3D).
- **Undo/redo** — command pattern for all editor operations.