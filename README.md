# Nova Engine

Nova is a Rust multimedia engine focused first on a correct, reusable GPU renderer. It uses `wgpu` for graphics and `winit` for windowing. The workspace currently provides a shared rendering core, a 2D sprite layer, an umbrella crate, and a runnable integration/stress harness.

## Workspace

| Crate | Scope |
| --- | --- |
| [`nova-core`](nova-core/README.md) | Application lifecycle, windows, assets, GPU resources, render targets, materials, pipelines, buffers, and math re-exports. |
| [`nova-2d`](nova-2d/README.md) | Sprite data, atlas helpers, 2D batching, camera projection, default materials, and the 2D plugin. |
| [`nova`](nova/README.md) | Public facade that re-exports the core and 2D APIs and defines `DefaultPlugins`. |
| [`nova-test`](nova-test/README.md) | Executable example and stress harness used to exercise the public API and renderer. |

The crates are workspace members at the repository root. `nova-2d` depends on `nova-core`; `nova` combines both; `nova-test` depends only on `nova`.

## Current Architecture

Applications implement `ApplicationProxy` with initialization, fixed-timestep update, and render callbacks. `ApplicationContext` exposes the window, `RenderContextRef`, asset manager, and default assets without exposing raw GPU ownership to application code.

The render path is:

1. `RenderContextRef` owns shared, interior-mutable `RenderContext` state.
2. `Frame` acquires the surface texture and creates a view.
3. A `RenderTarget` records commands, scene uniforms, and draw batches.
4. Dimension-specific renderers such as `Render2D` collect and sort commands.
5. `RenderTarget` resolves materials, caches GPU resources, submits commands, and `Frame` presents.

Assets are inserted directly into generational typed storage. `Texture` is a CPU asset that retains its pixel data. GPU textures, samplers, and compiled shaders are created lazily and cached by the renderer. Material types implement `Material` and `AsBindGroup`; the renderer registers their static template and builds instance bind groups during submission. `DrawBatch` uses `GenericHandle` so batchers can remain type-erased.

The current 2D path uses a shared base quad geometry and GPU-side instance data. `Sprite` is the only drawable, and `Batcher2D` groups instances by z-index and material handle.

## Build And Run

```text
cargo check --workspace
cargo run --release -p nova-test
```

## Roadmap

The next work follows the current architecture:

- Add asset serialization and file-backed loading without reintroducing GPU-owned assets or loader-driven runtime dependencies.
- Add hot reload and explicit invalidation for CPU assets and renderer caches.
- Improve cache lifetime and eviction policies for textures, samplers, shaders, and pipelines.
- Add camera/scenes, culling, and static-instance reuse so large scenes submit less CPU work.
- Add 3D support as another thin renderer over `nova-core`: meshes, perspective cameras, depth targets, lights, and 3D batching.
- Add render-graph or multi-pass composition around the existing `RenderTarget` and off-screen target APIs.
- Add richer diagnostics, focused tests, and benchmarks for generational handles, bind-group validation, cache keys, and batch submission.
- Add audio and other multimedia systems only after the graphics boundaries are stable.

The former `docs` directory has been retired. These README files are the maintained technical documentation for the workspace.