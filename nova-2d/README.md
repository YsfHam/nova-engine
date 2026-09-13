# nova-2d

`nova-2d` is the dimension-specific renderer built on `nova-core`. It provides a shape-based rendering API and turns shape instances into core `DrawBatch` submissions without importing `wgpu`.

## Current Scope

### Shape System

- `Shape2D` trait — a shape is tied to a material type via the associated `Material` type. The shape provides `InstanceData` (a `Pod` struct) and a geometry reference. The material owns the full `MaterialTemplate` (shader, instance layout, bind group layout, blend state).
- `ShapeInstance<S>` — builder struct carrying position, angle, scale, color, UV rect, material handle, and z-index. Generic over `Shape2D`.
- `RectangleShape` → `SpriteMaterial` — textured/solid rectangles using `RectInstance` (transform mat3 + color + UV rect).
- `CircleShape` → `CircleMaterial` — filled circles using `CircleInstance` (position + radius + color). Fragment shader discards outside the unit circle with anti-aliased edges.
- `Sprite` is a type alias for `ShapeInstance<RectangleShape>`. `SpriteAtlas` returns sprites with computed UV rects.
- Users can add custom shapes by implementing `Shape2D` + a compatible `Material` + a WGSL shader.

### Camera

- `Camera2D` with position, zoom, rotation, and viewport rect.
- Left-handed orthographic projection with top-left origin (`with_size`) or centered origin.
- Rotation pivots around the viewport center.
- Zoom scales the visible area (zoom > 1 = objects appear larger).

### Batching

- `Batcher2D` groups instances by z-index (BTreeMap) → material (HashMap). Instance data stored as type-erased raw bytes with per-material stride.
- `Render2D` exposes the scene flow: `begin_scene`, `draw::<S>(&ShapeInstance<S>)`, `end_scene`.
- All shapes share the base quad geometry (4 vertices + 6 indices), uploaded once.

### Materials

- `SpriteMaterial` — texture + tint. Uses `sprite_shader.wgsl`.
- `CircleMaterial` — no bind group data (color is per-instance). Uses `circle_shader.wgsl`.
- `ColorMaterial` — uniform color. Uses `rectangle_shader.wgsl`.
- `Nova2DPlugin` registers all material types and inserts default assets (white texture, default sprite/circle materials).

## Usage Shape

```rust
let mut renderer = Render2D::begin_scene(target.commander(environment));

// Rectangle (sprite)
renderer.draw(&Sprite::new(sprite_material)
    .with_position(vec2(100.0, 200.0))
    .with_scale(vec2(64.0, 64.0)));

// Circle
renderer.draw(&ShapeInstance::<CircleShape>::new(circle_material)
    .with_position(vec2(400.0, 300.0))
    .with_scale(vec2(80.0, 80.0))
    .with_color(Color::GREEN));

renderer.end_scene(RenderPassDescriptor::new(), &ctx.assets_manager);
```

---

## Roadmap

### Phase 1 — Performance Foundations

> Goal: eliminate the documented CPU bottlenecks and enable large-scale 2D scenes.

| Step | Description | nova-core dependency |
|------|-------------|---------------------|
| **1.1 GPU-side transform construction** | Replace `Mat3` in `RectInstance` with `(position, angle, scale)` — build the transform matrix in the vertex shader. Eliminates per-quad `sin`/`cos` + matrix construction (~40-50ns/quad). Reduces instance data 36B → 24B. | None |
| **1.2 Frustum culling** | Camera-visible AABB check before `batcher.add()`. Skip off-screen instances entirely. Combine with camera zoom/rotation bounds. | None |
| **1.3 Static instance reuse** | Detect instances that haven't changed since last frame and skip re-upload. Track dirty flags on `ShapeInstance` or batch-level dirty tracking. | StagingBufferPool may need a persistent-vs-dynamic distinction |
| **1.4 Instance buffer reuse** | Reuse the instance staging allocation across frames instead of re-allocating. Double-buffer or ring-buffer the instance upload. | `StagingBufferPool` already ring-buffers; verify instance path uses it |

### Phase 2 — Rendering Richness

> Goal: support the visual features needed for real 2D games and tools.

| Step | Description | nova-core dependency |
|------|-------------|---------------------|
| **2.1 More shapes** | `LineShape` (line segments with thickness), `RingShape` (outlined circle), `RoundedRectShape`, `PolygonShape`. Each implements `Shape2D` + its own material + WGSL shader. | None |
| **2.2 Text rendering** | Glyph atlas cache (CPU-side rasterization or external library). `TextShape` or a `TextRenderer` that emits `RectangleShape` instances with glyph UVs. Layout engine for wrapped/aligned text. | Texture atlas support (multi-region texture) |
| **2.3 Render layers** | Named render layers beyond z-index (e.g. background, world, UI). Layer-level blend mode, opacity, and visibility toggle. | None |
| **2.4 Off-screen composition** | Render-to-texture for post-processing (blur, bloom, color grading). `Render2D` targets an off-screen `TextureRenderTarget`, then a full-screen quad applies the effect. | `TextureRenderTarget` already exists in nova-core |
| **2.5 Custom blend modes** | Expose blend mode per-material or per-layer. Additive, multiply, screen for particle/UI effects. | `BlendMode` already in nova-core; may need more modes |
| **2.9 Sprite atlas improvements** | Support non-uniform grid atlases (JSON/metadata-driven cell packing). Texture array atlases for large sprite sets. | Texture arrays (wgpu supports, nova-core may need exposure) |

### Phase 3 — Scene & Content Structure

> Goal: provide the organizational layer for building real applications.

| Step | Description | nova-core dependency |
|------|-------------|---------------------|
| **3.1 Transform hierarchy** | Parent-child transform propagation. Local-space `ShapeInstance` positions resolved to world space via a scene tree. Dirty propagation for efficient updates. | None (pure nova-2d or new `nova-scene` crate) |
| **3.2 Scene graph API** | `Scene2D` struct that owns entities (transform + shape + material), manages visibility, and feeds `Render2D`. Decouples content from rendering. | None |
| **3.3 Spatial partitioning** | Quadtree or hash grid for efficient culling and spatial queries (pick, overlap). Feeds into frustum culling. | None |
| **3.4 Animation system** | Keyframe or skeletal animation for sprites. UV animation via atlas frame sequences. Tweening for transform properties. | Time system already in nova-core (`Clock`) |

### Phase 4 — Interaction & Debugging

> Goal: enable interactive applications and development workflows.

| Step | Description | nova-core dependency |
|------|-------------|---------------------|
| **4.1 Picking / raycast** | Screen-to-world coordinate conversion. Point-in-shape test for circles, rectangles, polygons. Returns the topmost instance under the cursor. | Input system (see nova-core roadmap) |
| **4.2 Debug overlays** | Wireframe mode, bounding box visualization, batch count / draw call stats overlay. Built on the existing shape system. | None |
| **4.3 Gizmos** | Translation/rotation/scale handles for editor use. On-canvas manipulation of scene entities. | Input system |

### Phase 5 — Quality & Polish

> Goal: production-ready rendering quality.

| Step | Description | nova-core dependency |
|------|-------------|---------------------|
| **5.1 MSAA / supersampling** | Multi-sample anti-aliasing for shape edges. Configurable sample count. | wgpu MSAA support (nova-core pipeline config) |
| **5.2 DPI awareness** | Correct rendering on high-DPI displays. Logical vs physical pixel scaling. Camera viewport adjusts to surface scale. | Window DPI/scale info |
| **5.3 Color space management** | sRGB-aware rendering pipeline. Linear-space blending for correct alpha compositing. | Surface format negotiation (nova-core) |
| **5.4 Texture filtering control** | Per-material sampler config (nearest vs linear, mipmaps, anisotropic). Already partially supported via `SamplerConfig`. | Sampler config already in nova-core |