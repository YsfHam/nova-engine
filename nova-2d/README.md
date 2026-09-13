# nova-2d

`nova-2d` is the dimension-specific renderer built on `nova-core`. It provides a sprite API and turns sprites into core `DrawBatch` submissions without importing `wgpu`.

## Current Scope

- `Sprite` stores a material handle, position, scale, rotation, tint, z-index, and normalized UV rectangle.
- `SpriteAtlas` maps fixed-size atlas cells to sprites with normalized UVs.
- `Camera2D` creates a left-handed orthographic projection from a position and viewport size.
- `Batcher2D` groups instances by z-index and material, then emits shared-geometry draw batches.
- `Render2D` exposes the scene flow: `begin_scene`, `draw`, and `end_scene`.
- `ColorMaterial` supplies a uniform color path.
- `SpriteMaterial` supplies texture-backed rendering.
- `Nova2DPlugin` registers materials, inserts default assets, and uploads the shared base quad geometry.
- Default assets include a white texture, default color material, and default sprite material.

The base quad is centered at the origin. Transforms are represented in instance data and reconstructed by the shader, allowing many sprites to share one persistent geometry allocation.

## Usage Shape

An application obtains a material handle, creates sprites, builds a scene bind group containing camera uniforms, and submits the scene through a `RenderTarget` commander:

```rust
let mut renderer = Render2D::begin_scene(target.commander(environment));
renderer.draw(Sprite::new(material).with_position(position));
renderer.end_scene(RenderPassDescriptor::new(), &ctx.assets_manager);
```

## Planned Scope

- Add camera helpers for zoom, rotation, viewport changes, and coordinate conversion.
- Add text rendering backed by a glyph atlas and the existing sprite batching path.
- Add visibility/frustum culling and static-instance reuse for large scenes.
- Move shared geometry ownership from the current plugin-level global toward a renderer-scoped owner.
- Improve atlas validation and support more flexible layouts than a uniform grid.
- Add 2D render layers, off-screen composition, and post-processing helpers.
- Keep custom material implementations compatible with the core registration and bind-group contracts.