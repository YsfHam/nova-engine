# nova-core

`nova-core` is the engine foundation. It owns application startup, window integration, asset storage, GPU access, rendering infrastructure, and shared math types.

## Current Scope

### Application

- `ApplicationBuilder` configures window attributes, graphics configuration, frame rate, control flow, and plugins.
- `ApplicationProxy` is the application entry point with `on_init`, `on_update`, and `on_render` callbacks.
- `ApplicationContext` provides `WindowApi`, `RenderContextRef`, `AssetsManager`, and `DefaultAssets`.
- The engine currently targets synchronous, single-threaded application and rendering flow.

### Assets

- `Asset` is a `'static` marker trait. Assets are inserted directly through `AssetsManager::insert_asset`.
- `AssetStorage<T>` uses typed generational `Handle<T>` values and safe stale-handle rejection.
- `GenericHandle` provides type-erased identity for draw batches and material registration.
- `get_asset`, `get_asset_mut`, `remove_asset`, and `get_asset_any` provide typed and erased lookup paths.
- `Texture` is a CPU-side asset containing source data, size, format, and sampler configuration.

### Graphics

- `GraphicsContext` owns the `wgpu` device, queue, surface, and surface configuration internally.
- `RenderContextRef` shares the central `RenderContext` through `Rc<RefCell<_>>`.
- `Frame` owns the acquired surface texture and presents it after rendering.
- `RenderTarget` records commands for on-screen or off-screen views and submits on drop.
- `RenderCache` lazily caches GPU textures, sampler objects, and compiled shaders.
- `PipelineCache` caches pipelines using stable material-template and layout information.
- `GeometryPool` stores reusable geometry; `StagingBufferPool` handles dynamic uploads.
- `UniformBuffer` is used for scene and material uniform uploads.

### Materials And Batches

`Material::template()` defines static shader, vertex, blend, depth, topology, and bind-group layout information. `AsBindGroup` produces GPU-free runtime values. The renderer validates those values, resolves texture handles, creates bind groups per submission, and reuses the compiled pipeline.

`DrawBatch` is the dimension-agnostic submission contract. Core does not know about sprites or meshes; higher-level crates provide the command collection and sorting policy.

## Boundary Rules

- Raw `wgpu` ownership stays inside `nova-core`.
- Higher-level renderer crates use `RenderTarget`, `RenderPass`, assets, and engine-native descriptors.
- Asset data remains separate from renderer-owned GPU state.
- Core remains dimension-agnostic and must not depend on 2D or 3D command types.

## Planned Scope

### Phase 1 — Input & Window

> Goal: enable interactive applications. Required before any real application can be built.

| Step | Description | nova-2d dependency |
|------|-------------|-------------------|
| **1.1 Input system** | Keyboard, mouse, and touch event handling. Platform-agnostic input state (key codes, button states, scroll deltas). `ApplicationContext` exposes an `InputApi`. Pluggable backends (winit events → engine input state). | None (nova-2d picking depends on this) |
| **1.2 Window DPI / resize events** | Surface reconfiguration on resize. Logical vs physical pixel size. DPI scale factor propagation to renderers and camera. | Camera2D viewport update on resize |
| **1.3 Window lifecycle events** | Focus gain/loss, minimized/restored, close requested. ApplicationProxy callbacks for each. | None |

### Phase 2 — Asset Pipeline

> Goal: move from manual asset insertion to a real loading pipeline.

| Step | Description | nova-2d dependency |
|------|-------------|-------------------|
| **2.1 Asset loading from files** | `AssetsManager::load_from_file(path)` — resolve asset type from file extension or metadata, route to the correct loader, insert and return a handle. Replaces the current `unimplemented!()` stub. | None |
| **2.2 Asset metadata & serialization** | Serialize/deserialize asset metadata (TypeId + metadata struct). Dependency resolution from relative paths. Scene file format foundation. | Scene system (nova-2d Phase 3) |
| **2.3 Hot reload** | Watch asset source files for changes. Invalidate GPU caches (textures, shaders, pipelines). Re-upload changed assets without restarting. | Material/shader reload (nova-2d materials) |
| **2.4 Async loading** | Background thread asset loading with a load queue and completion notification. Avoid blocking the main/render thread on I/O. `Handle<A>` starts as "pending", resolves when load completes. | None |

### Phase 3 — Rendering Infrastructure

> Goal: support the multi-pass, off-screen, and quality features the 2D renderer needs.

| Step | Description | nova-2d dependency |
|------|-------------|-------------------|
| **3.1 Render graph / pass graph** | Declarative multi-pass rendering: describe passes and their dependencies, let the engine schedule and resource-manage them. Enables post-processing chains, shadow passes, etc. | nova-2d Phase 2.4 (off-screen composition) |
| **3.2 Depth buffer support** | Depth texture creation and attachment. Depth-stencil config on render passes. Enables 3D rendering and depth-based 2D layering. | Optional for 2D; required for 3D |
| **3.3 Pipeline statistics & timing** | GPU timestamp queries for per-pass timing. Pipeline statistics (vertex count, fragment invocations). Feed into debug overlays. | nova-2d Phase 4.2 (debug overlays) |
| **3.4 Buffer sub-allocation** | Fine-grained sub-allocation within large GPU buffers for vertex/index/instance data. Reduces buffer count and allocation overhead. | Directly improves nova-2d batcher upload perf |

### Phase 4 — Threading & Architecture

> Goal: scale beyond single-threaded synchronous execution.

| Step | Description | nova-2d dependency |
|------|-------------|-------------------|
| **4.1 Render thread** | Move rendering to a dedicated thread. Main thread produces draw commands; render thread consumes and submits. Overlaps CPU frame work with GPU execution. | nova-2d batcher must be Send-ready |
| **4.2 Command buffering** | Record multiple frames of commands ahead of GPU execution. Triple-buffered command submission for pipeline-smooth throughput. | None |
| **4.3 Resource deletion queue** | Deferred destruction of GPU resources (old textures, resized buffers) that are still referenced by in-flight frames. Prevents use-after-free in a multi-frame pipeline. | None |

### Phase 5 — 3D Foundations

> Goal: lay the groundwork for 3D rendering as a future `nova-3d` crate.

| Step | Description | nova-2d dependency |
|------|-------------|-------------------|
| **5.1 Mesh assets** | 3D vertex formats (position, normal, UV, tangent). Mesh asset type and storage. Index buffer management for complex geometry. | None |
| **5.2 Perspective camera** | 3D camera with perspective projection, view matrix, frustum extraction. Coexists with `Camera2D`. | None |
| **5.3 Lighting data** | Directional, point, and spot light uniforms. Light uniform buffer management. Shadow map generation pipeline. | None |
| **5.4 3D batch submission** | `DrawBatch` with 3D mesh geometry + material + transform instances. 3D material templates with lighting shaders. | None |