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

- Serialization and file-backed asset reconstruction using CPU asset data and explicit dependency resolution.
- Hot reload, cache invalidation, and bounded cache lifetime policies.
- Stronger bind-group diagnostics and focused unit tests for layouts, handles, and pipeline keys.
- Depth texture management, render graphs, multi-pass composition, and additional off-screen workflows.
- 3D foundations: mesh assets, depth-aware targets, perspective camera support, lighting data, and 3D batch submission.
- Optional multithreaded or asynchronous loading once the synchronous ownership model is fully exercised.