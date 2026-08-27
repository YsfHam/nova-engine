# Nova Engine — Final Architecture

> **Status:** Authoritative — supersedes `nova-engine-architecture.md` and `rendering-architecture-plan.md`.
> **Last updated:** 2026-08-25

---

## 1. Vision & Principles

Nova Engine is a simple, efficient 2D/3D multimedia framework in Rust built on `wgpu`.

**Guiding principles:**

1. **One renderer, not two.** 2D and 3D are configurations and thin helpers over a single shared rendering core — never separate renderers.
2. **Graphics first, correctness first.** Render the foundation before audio/physics. Prefer correct, ergonomic architecture over micro-optimization; performance comes from good structure.
3. **Type safety over raw handles.** Generational, typed `Handle<T>` references. Stale handles resolve to `None`, never use-after-free.
4. **Nothing outside `nova-core::graphics` touches raw `wgpu`.** `GraphicsContext` stays `pub(crate)`. `RenderContext` is the only public face of the GPU.
5. **Synchronous and single-threaded** for V1. No async loading, no multithreaded rendering. `Arc<Mutex>` keeps the door open for later.
6. **Data-driven where it counts.** Material templates and shaders are assets loaded via metadata; per-instance materials are lightweight runtime objects that reference them. Assets own their metadata — it is their identity for serialization, dedup, and hot-reload.
7. **Layered with hard boundaries.** Each layer has a well-defined responsibility and never reaches below its own level.

---

## 2. Workspace Layout

```
nova-engine/
├── Cargo.toml                  # workspace root
├── crates/
│   ├── nova-core/              # shared infra: app, window, GPU, renderer, assets, math
│   ├── nova-2d/                # 2D helpers: SpriteBatch, Camera2D, TextRenderer (future)
│   ├── nova-3d/                # 3D helpers: MeshRenderer, Camera3D, LightSystem (future)
│   └── nova/                   # optional umbrella crate (facade with feature flags)
├── nova-test/                  # test harness / examples (early adopter)
└── docs/
    └── architecture.md         # this document
```

### 2.1 Crate Dependency Graph

```
┌──────────────┐
│   nova-2d    │────┐
└──────────────┘    │     ┌──────────────┐
┌──────────────┐    ├────▶│  nova-core   │
│   nova-3d    │────┘     └──────────────┘
└──────────────┘
```

- `nova-2d` and `nova-3d` depend on `nova-core` and **never on each other**.
- `nova-2d` and `nova-3d` **never import `wgpu`** — they go through `RenderContext` / `Frame` / `RenderPass`.
- User applications depend on `nova-2d` and/or `nova-3d` (and may use `nova-core` directly for asset types).
- An optional `nova` umbrella crate re-exports everything with `2d`/`3d` feature flags.

### 2.2 What Goes Where

**`nova-core` (shared infrastructure):**

| Component | Why it's here |
|-----------|---------------|
| `GraphicsContext` | GPU connection — both dimensions need it |
| `RenderContext` | Public hub: pipeline cache, bind group allocator, frame lifecycle, surface management |
| `Frame` | Per-frame state: surface texture, command encoder, uniform arena, frame index |
| `RenderPass` | Scoped recording context borrowing `Frame` mutably |
| `AssetManager`, `AssetStorage<T>`, `Handle<T>` | Handle-based resource storage shared by all |
| `Asset`, `AssetLoader`, `ErasedLoader`, `LoadContext` | Metadata-driven loader system (assets own their `Metadata`) |
| `Shader`, `Texture`, `Sampler`, `Mesh` | GPU-backed assets used by both dimensions |
| `MaterialTemplate`, `Material` | The material model (recipe + instance) |
| `WindowApi` | Window abstraction — shared |
| `AppContext`, `ApplicationProxy` | Application entry point — shared |
| `DrawBatch` | The dimension-agnostic submission contract |
| `UniformArena` | Per-render-target transient buffer for scene-global uniforms (camera, time). Group 0. |
| Default WGSL shaders | Embedded via `include_str!`, registered at init |

**`nova-2d` (2D-specific helpers):**

| Component | Why it's here |
|-----------|---------------|
| `Render2D` | Render-target-scoped borrower — 2D command API |
| `QuadCmd` | 2D render command |
| `BatchKey2D` | 2D sort key (template + texture) |
| `Batcher2D` | Collects, sorts, flushes 2D commands |
| `SpriteBatch` | Dynamic vertex buffer builder for quads |
| `Camera2D` | Orthographic projection → uniform bytes |
| `TextRenderer` | Glyph atlas + sprite batch (future) |
| 2D default material templates | Embedded WGSL for flat/textured 2D |

**`nova-3d` (3D-specific helpers):**

| Component | Why it's here |
|-----------|---------------|
| `Render3D` | Render-target-scoped borrower — 3D command API |
| `MeshCmd` | 3D render command |
| `BatchKey3D` | 3D sort key (template + mesh + material) |
| `Batcher3D` | Collects, sorts (depth for transparency), flushes 3D commands |
| `MeshRenderer` | Draws `Mesh` assets |
| `Camera3D` | Perspective projection → uniform bytes |
| `LightSystem` | Directional/point/spot light uniform bind group |
| `SceneUniforms` | Camera + lighting data uploaded once per frame |
| 3D default material templates | Embedded WGSL for unlit/PBR 3D |
| Culling (future) | Frustum culling |

### 2.3 The Key Boundary: `RenderContext` vs Batcher

`RenderContext` (in core) owns the GPU plumbing: pipeline cache, bind group allocator, frame lifecycle. It is **dimension-agnostic** — it knows nothing about quads, meshes, or sprites.

Batchers (`Batcher2D`, `Batcher3D`) are **dimension-specific** and live in `nova-2d` / `nova-3d`. They collect commands, sort them, and produce `DrawBatch` structs that they hand to `RenderTarget::submit_draw_batch()`.

```
Render2D::draw_quad(cmd)                      Render3D::draw_mesh(cmd)
  → Batcher2D: collect cmd                      → Batcher3D: collect cmd
  → (at frame end) Batcher2D: sort              → (at frame end) Batcher3D: sort
  → Batcher2D: flush                            → Batcher3D: flush
    → RenderTarget::submit_draw_batch(DrawBatch)     → RenderTarget::submit_draw_batch(DrawBatch)
      → pipeline cache lookup (RenderContext)      → pipeline cache lookup (RenderContext)
      → bind group creation (RenderContext)        → bind group creation (RenderContext)
      → uniform arena (render-target-scoped)        → uniform arena (render-target-scoped)
      → command encoder (render-target-scoped)      → command encoder (render-target-scoped)
    → (on RenderTarget::submit) GPU submit        → (on RenderTarget::submit) GPU submit
    → (after submit) Frame::present()              → (after submit) Frame::present()
```

This keeps core clean: it only knows about `DrawBatch`, never `QuadCmd` or `MeshCmd`.

---

## 3. Layered Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Application Layer                    [user crate]        │
│  ├── ApplicationProxy trait (user entry point)            │
│  ├── AppContext (RenderContext + WindowApi, no raw wgpu)  │
│  └── User code (scenes, game logic, etc.)                 │
├──────────────────────────────────────────────────────────┤
│  High-Level Renderer                 [nova-2d / nova-3d]  │
│  ├── Render2D / Render3D (borrow RenderTarget, commands) │
│  ├── Batcher2D / Batcher3D (dimension-specific sorting)   │
│  └── Command structures (QuadCmd, MeshCmd, etc.)          │
├──────────────────────────────────────────────────────────┤
│  RenderTarget & RenderPass          [nova-core]         │
│  ├── RenderTarget (view + encoder + uniform arena)        │
│  ├── RenderPass (scoped recording, borrows RenderTarget)  │
│  ├── submit_draw_batch() — the contract for batchers      │
│  ├── borrows &mut RenderContext (pipeline + bind group)  │
│  └── submit() — encoder.finish + queue.submit             │
├──────────────────────────────────────────────────────────┤
│  Frame                               [nova-core]         │
│  ├── Surface texture + view (for RenderTarget)            │
│  └── present() — surface present                         │
├──────────────────────────────────────────────────────────┤
│  RenderContext                        [nova-core]         │
│  ├── Pipeline cache (keyed by MaterialTemplate)           │
│  ├── Bind group allocator + MaterialUniformPool           │
│  ├── Scene bind group layout (group 0 singleton)         │
│  ├── begin_frame() → Frame                                │
│  ├── Surface management (resize, recover, present mode)   │
│  └── Holds: GraphicsContext                                │
├──────────────────────────────────────────────────────────┤
│  GraphicsContext                      [nova-core]         │
│  ├── wgpu::Surface / Device / Queue / Config              │
│  └── pub(crate) — never exposed outside nova-core        │
├──────────────────────────────────────────────────────────┤
│  Asset System                         [nova-core]         │
│  ├── AssetStorage<T> (generational arena per type)        │
│  ├── Handle<T> (Copy, typed, generational)                │
│  ├── AssetManager (TypeMap of storages + loader registry) │
│  ├── Asset / AssetLoader / ErasedLoader / LoadContext     │
│  ├── Metadata-driven load: load<A>(A::Metadata)          │
│  └── Assets: Shader, Texture, Sampler, Mesh,              │
│      MaterialTemplate, Material                           │
└──────────────────────────────────────────────────────────┘
```

### 3.1 Layer Responsibilities

| Layer | Crate | Owns | Knows About | Exposes To Above |
|-------|-------|------|-------------|------------------|
| `GraphicsContext` | nova-core | GPU device, queue, surface | wgpu only | Raw GPU access (to `RenderContext` only) |
| `RenderContext` | nova-core | Pipeline cache, bind group allocator, scene bind group layout, `MaterialUniformPool`, surface | `GraphicsContext` + asset handles | `begin_frame()` → `Frame`, `device()`/`queue()` accessors |
| `Frame` | nova-core | Surface texture + view | — | `view()` (to `RenderTarget`), `present()` |
| `RenderTarget` | nova-core | Command encoder, uniform arena | `RenderContext` (borrowed mutably) | `begin_render_pass()`, `submit_draw_batch()`, `upload_uniform()` (to batchers) |
| `RenderPass` | nova-core | Scoped wgpu render pass recording | `RenderTarget` (borrowed mutably) | Draw methods (to high-level renderer) |
| High-Level Renderer | nova-2d / nova-3d | Command collection, batching, sorting | `RenderTarget` + `RenderPass` API | `Render2D`/`Render3D` borrowers (to Application) |
| Application | user crate | Scene state, game logic | `AppContext` | Nothing — this is the top |

**Key rules:**
- No layer reaches below its own level. Application never sees `wgpu`. High-level renderer never touches `wgpu` directly — it goes through `Frame` / `RenderPass`.
- `nova-2d` and `nova-3d` never import `wgpu`. They go through `RenderTarget` / `RenderPass`.
- `nova-core` does not know about `QuadCmd`, `MeshCmd`, `BatchKey2D`, or `BatchKey3D`. It only knows about `DrawBatch`.
- `RenderContext` is long-lived (created once at startup). `RenderTarget` is short-lived (created and submitted each render target scope). `Frame` is short-lived (surface texture lifecycle). This separation prevents stale frame state from leaking between frames.

---

## 4. GraphicsContext

The lowest layer. Owns the GPU connection. `pub(crate)` — never exposed outside `nova-core`.

```rust
pub(crate) struct GraphicsContext {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) width: AtomicU32,
    pub(crate) height: AtomicU32,
}
```

- Created once at startup via `GraphicsContext::new(window).await`.
- Wrapped in `Arc` and shared with `RenderContext` (which wraps it in `Mutex` for interior mutability).
- **Not exposed to the application layer.** Only `RenderContext` and asset loaders (via `LoadContext`) hold a reference.

---

## 5. RenderContext

The public hub for all rendering. Sits between `GraphicsContext` and the high-level renderer. Long-lived — created once at startup, lives for the app's lifetime.

### 5.1 Purpose

`RenderContext` is the **single owner of all rendering infrastructure** above raw GPU access. It provides the structural enforcement that the high-level renderer avoids touching `wgpu` directly — not just a convention, but a constraint enforced by visibility.

### 5.2 Structure

```rust
pub struct RenderContext {
    gfx: GraphicsContext,                      // owns the GPU connection
    scene_bind_group_layout: wgpu::BindGroupLayout, // singleton group 0 layout
    pipeline_cache: PipelineCache,             // keyed by (Handle<MaterialTemplate>, TextureFormat)
    bind_group_allocator: BindGroupAllocator,  // per-material bind group cache + MaterialUniformPool
}
```

> **Design constraint — don't let it become a god object.** Each subsystem (`PipelineCache`, `BindGroupAllocator`) is its own struct. `RenderContext` *coordinates* them, it does not *be* them.

### 5.3 Responsibilities

- **Pipeline cache**: Compiles and caches `wgpu::RenderPipeline` objects, keyed by `(Handle<MaterialTemplate>, TextureFormat)`. First encounter with a new template triggers compilation; subsequent materials using the same template reuse the cached pipeline.
- **Bind group allocator**: Owns the `MaterialUniformPool` (shared buffer for all material uniforms) and a `HashMap<Handle<Material>, wgpu::BindGroup>` cache. Materials are immutable → bind groups built once, cached forever.
- **Scene bind group layout**: Singleton group 0 layout (binding 0 = camera Mat4, binding 1 = time F32). Every pipeline includes this as its first bind group layout. `UniformArena` builds one bind group per render target.
- **Frame lifecycle**: `begin_frame()` acquires the current surface texture, creates a view, and returns a lightweight `Frame`. The caller creates a `RenderTarget` from `&mut RenderContext` + the view, draws into it, calls `submit()`, then `Frame::present()`.
- **Surface management**: Resize, reconfigure, recover from loss. Configurable present mode.
- **Resource accessors**: `device()`, `queue()`, `surface_format()` — the escape hatch for advanced/crate-internal use (e.g., asset loaders, `nova-2d`/`nova-3d` vertex buffer creation).

### 5.4 Public API (V1)

| Method | Description |
|--------|-------------|
| `device() -> &wgpu::Device` | Access to `wgpu::Device` (for resource creation) |
| `queue() -> &wgpu::Queue` | Access to `wgpu::Queue` (for buffer/texture writes) |
| `surface_format() -> TextureFormat` | Current surface format (needed for pipeline creation) |
| `scene_bind_group_layout() -> &BindGroupLayout` | Group 0 layout (for the `UniformArena`) |
| `begin_frame() -> Option<Frame>` | Acquire surface texture, create view, return `Frame` |
| `resize(width, height)` | Reconfigure surface on window resize |

### 5.5 Interior Mutability

Because `RenderContext` is shared via `Arc`, all methods take `&self`. Mutable state (surface config, pipeline cache) uses `Mutex`. Uncontended single-threaded `Mutex` is effectively free. This keeps the option open for multithreading later without refactoring.

---

## 6. Frame, RenderTarget & RenderPass

### 6.1 Frame — Surface Lifecycle

Created by `RenderContext::begin_frame()`. **Lightweight** — owns only the surface texture and its view. The caller passes the view to a `RenderTarget` and calls `present()` after the render target is submitted.

```rust
pub struct Frame {
    output: wgpu::SurfaceTexture,
    view: wgpu::TextureView,
}

impl Frame {
    pub fn view(&self) -> &wgpu::TextureView;
    pub fn present(self, queue: &wgpu::Queue);
}
```

`Frame` has no lifetime tied to `RenderContext` and no GPU command state. It exists only for on-screen rendering; off-screen rendering creates a `RenderTarget` directly from an arbitrary `TextureView`.

### 6.2 RenderTarget — View-Agnostic Rendering

`RenderTarget` is the view-agnostic rendering scope. It borrows `&mut RenderContext` for direct mutable access to the pipeline cache and bind group allocator (no `RefCell`), owns the command encoder and the per-target `UniformArena`, and works for both on-screen and off-screen rendering.

```rust
pub struct RenderTarget<'a> {
    render_ctx: &'a mut RenderContext,
    view: &'a wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
    uniform_arena: UniformArena,
}
```

**Why `RenderTarget` borrows `&mut RenderContext`:** The pipeline cache and bind group allocator need mutable access during a frame (compile/cache pipelines, build/cached bind groups). `RenderTarget` holding `&mut RenderContext` gives direct field access — no `RefCell`, no guard types, references returned from the cache are tied to the `RenderTarget` borrow lifetime.

**What lives on `RenderTarget` (per-render-target, transient):**
- The target texture view (surface or off-screen)
- The command encoder (all draw commands recorded into this)
- The uniform arena (scene-global uniforms for this render target)

**What stays in `RenderContext` (persistent, shared across frames):**
- `GraphicsContext` (the GPU connection)
- Pipeline cache (pipelines persist across frames)
- Bind group allocator (bind group cache persists; individual bind groups are cached per-material, immutable)
- Scene bind group layout (singleton group 0 layout)

**Lifecycle:**

```rust
impl RenderContext {
    pub fn begin_frame(&mut self) -> EngineResult<Option<Frame>> {
        let output = self.get_surface_texture()?; // handles loss/Outdated
        Ok(Some(Frame::new(output)))
    }
}

// On-screen rendering (in handler.rs):
let frame = render_ctx.begin_frame()?;
let mut target = RenderTarget::new(&mut render_ctx, frame.view());
proxy.on_render(ctx, &mut target);
target.submit();                    // encoder.finish() + queue.submit()
let queue = render_ctx.queue().clone();
frame.present(&queue);             // surface present

// Off-screen rendering (future):
let mut target = RenderTarget::new(&mut render_ctx, offscreen_view);
// ... draw into target ...
target.submit();                    // encoder.finish() + queue.submit (no present)
```

### 6.3 RenderPass — Recording Scope

```rust
pub struct RenderPass<'frame> {
    inner: wgpu::RenderPass<'frame>,  // borrows RenderTarget mutably
}
```

- `RenderTarget::begin_render_pass(desc) -> RenderPass` — opens a render pass on the render target's view (or a provided off-screen target).
- `set_pipeline(&mut self, &Pipeline)`
- `set_bind_group(&mut self, index, &BindGroup)`
- `set_vertex_buffer`, `set_index_buffer`
- `draw(...)`, `draw_indexed(...)`
- `draw_material(&mut self, &Material, ...)` — convenience (Step 9): binds pipeline + bind groups + draws.

2D and 3D renderers build higher-level helpers on top of this (e.g., `SpriteBatch::flush(&mut self, pass: &mut RenderPass)`).

### 6.4 Off-Screen Rendering

`RenderTarget` is view-agnostic: `new(render_ctx, view)` accepts any `TextureView`. This enables post-processing and render-to-texture for 3D without changing the API — just create a `RenderTarget` with an off-screen view.

### 6.5 Uniform System — Two Sources, Two Bind Groups

Uniforms come from two distinct sources with two bind groups:

| Concern | Owner | Lifetime | Bind group | Serializable |
|---------|-------|----------|------------|--------------|
| Scene globals (camera, time, lights) | `UniformArena` on `RenderTarget` | one render target scope | **group 0** | no — runtime state |
| Per-material params (color, transform) | `Material` (immutable, typed values) | lives with the asset | **group 1** | yes — typed `UniformValue`s |

**Group 0 (environment):** Singleton layout on `RenderContext` (binding 0 = camera Mat4, binding 1 = time F32). `UniformArena` builds one bind group per render target via `build_bind_group()`. Cameras call `RenderTarget::upload_uniform(binding_slot, value)`.

**Group 1 (material):** Layout derived from `MaterialTemplateMetadata` (uniform_layout + texture_layout). `MaterialUniformPool` holds a shared persistent `wgpu::Buffer` for all material uniforms (batch-built). `BindGroupAllocator` caches per-material `wgpu::BindGroup` keyed by `Handle<Material>` — immutable materials mean bind groups are built once, never invalidated.

### 6.6 UniformArena

Per-render-target uniform uploads for scene-global data (camera matrices, time, transforms) that doesn't belong in `Material`.

```rust
impl UniformArena {
    pub fn upload(&mut self, binding_slot: u32, value: UniformValue);
    pub fn build_bind_group(&mut self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Option<wgpu::BindGroup>;
    pub fn reset(&mut self);
}
```

- Both `Camera2D` and `Camera3D` produce matrices and call `upload_uniform(...)`. `UniformValue` is typed (Mat4/Vec4/F32), not raw bytes — so the arena can rebuild the buffer when invalidated.
- Arena builds one bind group (group 0) per render target.

### 6.7 MaterialUniformPool

Shared, persistent `wgpu::Buffer` for all material uniforms. Batch-built from an iterator of materials.

```rust
impl MaterialUniformPool {
    pub fn build<'a, I>(&mut self, device: &wgpu::Device, materials: I)
        where I: IntoIterator<Item = MaterialUniformEntry<'a>>;
    pub fn buffer(&self) -> Option<&wgpu::Buffer>;
    pub fn allocation(&self, handle: Handle<Material>) -> Option<UniformAllocation>;
}
```

- The caller (the entity setting up the render pass) controls when to (re)build: first frame or after materials added/removed.
- Each material gets a fixed `(offset, size)` allocation that never moves (materials are immutable, append-only).

### 6.8 submit_draw_batch — The Contract

```rust
impl<'a> RenderTarget<'a> {
    pub fn submit_draw_batch(&mut self, batch: DrawBatch) {
        // 1. Look up pipeline from RenderContext's pipeline cache
        let pipeline = self.get_or_compile_pipeline(batch.template);
        // 2. Get or create bind group from the bind group allocator
        let bind_group = self.get_or_create_bind_group(batch.material, pipeline.bind_group_layout);
        // 3. Record render pass commands
        // ...
    }
}
```

This is the dimension-agnostic submission interface. `Batcher2D` and `Batcher3D` both call it — they just produce different `DrawBatch` instances from different command types.

### 6.9 DrawBatch

The contract struct between dimension-specific batchers and the dimension-agnostic `RenderTarget`:

```rust
pub struct DrawBatch {
    pub template_key: PipelineCacheKey,  // derived from MaterialTemplate
    pub material: Handle<Material>,       // material to bind
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
    pub instance_count: u32,
}
```

---

## 7. Asset System

### 7.1 What Is an Asset?

An **asset** is any long-lived engine resource that:
1. Is loaded from a file (or constructed once and registered), and
2. Is referenced by a typed `Handle<T>` through the `AssetManager`, and
3. May be shared across many consumers (materials, scenes, renderers), and
4. Benefits from generational lifecycle management (creation, lookup, removal, future hot-reload).

**Assets (stored in `AssetManager`):**

| Asset type | GPU resource | Loaded from | Notes |
|------------|-------------|-------------|-------|
| `Shader` | `wgpu::ShaderModule` | `.wgsl` files | WGSL only for V1; SPIR-V later |
| `Texture` | `wgpu::Texture` + view | `.png`, `.jpg` | Decoded via `image` crate; pixels discarded after upload |
| `Sampler` | `wgpu::Sampler` | Created in code | Not file-loaded; added via `add()` |
| `Mesh` | `wgpu::Buffer` (VB + IB) | `.obj`, `.gltf` (later) | CPU data discarded after upload (V1) |
| `MaterialTemplate` | (pipeline recipe — no GPU resource directly) | `.mat.toml` / `.mat.ron` | The rendering "kind"; drives pipeline compilation |
| `Material` | uniform buffer + bind groups | Created in code or `.mat.toml` | Instance of a template; can be an asset or runtime object |

**Explicitly NOT assets (runtime-only, not in `AssetManager`):**
- `Camera2D` / `Camera3D` — per-frame state, uploaded via `UniformArena`.
- `LightSystem` / `SceneUniforms` — per-frame global uniforms.
- `SpriteBatch` / `MeshRenderer` — transient per-frame command collectors.
- `QuadCmd` / `MeshCmd` — ephemeral render commands.
- `Frame` / `RenderTarget` / `RenderPass` — per-frame scope objects.

The distinction: **if it persists across frames and is shared by reference, it's an asset. If it's reconstructed every frame or scoped to a frame, it's not.**

### 7.2 Core Data Structures

**`AssetStorage<T>` — generational arena per asset type:**

```rust
pub struct AssetStorage<T: Asset> {
    slots: Vec<Option<T>>,
    generations: Vec<u32>,
    free_list: Vec<u32>,
}
```

- `add(item) -> Handle<T>` — inserts into a free slot or appends, returns handle.
- `get(handle) -> Option<&T>` — validates generation, returns reference.
- `get_mut(handle) -> Option<&mut T>` — mutable access.
- `remove(handle) -> Option<T>` — frees slot, bumps generation. Stale handles elsewhere return `None` on future lookups.

**`Handle<T>` — typed, generational reference:**

```rust
pub struct Handle<T: Asset> {
    index: u32,
    generation: u32,
    _marker: PhantomData<T>,
}
```

- `Copy` — cheap to pass around, store in materials, duplicate freely.
- Typed — `Handle<Texture>` and `Handle<Shader>` are distinct types; the compiler prevents misuse.
- Generational — detects stale/dangling references (returns `None` instead of UB).
- `Hash + Eq + PartialEq` — usable as `HashMap` keys (for batch sort keys).

**`AssetManager` — heterogeneous container:**

```rust
pub struct AssetManager {
    storages: HashMap<TypeId, Box<dyn Any>>,  // TypeMap — one AssetStorage<T> per type
    loaders: Vec<Box<dyn ErasedLoader>>,       // dispatched by file extension
    ctx: Arc<RenderContext>,                   // internal, passed to loaders
}
```

- Lookup by `TypeId` — `manager.storage::<Texture>()` downcasts and returns the typed storage.
- Single source of truth for all engine resources.

### 7.3 Traits

```rust
pub trait Asset: 'static {
    type Metadata: Any + Send + Sync + Clone + 'static;
    fn metadata(&self) -> &Self::Metadata;
}

pub trait AssetLoader: 'static {
    type Asset: Asset;
    fn load(&self, metadata: <Self::Asset as Asset>::Metadata, ctx: &LoadContext<'_>) -> Result<Self::Asset, AssetError>;
}

pub(crate) trait ErasedLoader: 'static {
    fn load_erased(&self, metadata: Box<dyn Any>, ctx: &LoadContext<'_>) -> Result<Box<dyn Any>, AssetError>;
}
```

**`ErasedLoader` pattern:** wraps a concrete `AssetLoader` in a type-erased form so the asset manager can store heterogeneous loaders in a single collection. `load_erased` takes `Box<dyn Any>` metadata (downcast to `A::Metadata` inside) and returns `Box<dyn Any>` (downcast to `A` by the caller).

**Loaders are stateless** (`load` takes `&self`, not `&mut self`). This keeps `AssetLoadersStorage::get()` immutable. If a loader ever needs internal caching, it should use interior mutability (`RefCell`/`Mutex`).

### 7.4 LoadContext

```rust
pub struct LoadContext<'a> {
    pub render_ctx: Rc<RefCell<RenderContext>>,   // for GPU uploads (texture, buffer creation)
    pub assets: &'a AssetsManager,                 // for retrieving already-loaded dependencies
}
```

- Passed to loaders. Provides GPU access (for texture upload, buffer creation) and read-only `AssetsManager` access (for retrieving already-loaded dependencies by handle — e.g. `ctx.assets.get_asset::<Shader>(shader_handle)`).
- **Loaders do not load new assets** from `LoadContext`. Dependency path→handle resolution is the caller's job (whoever constructs the metadata resolves dependencies first). This will be automated when serialization/`load_from_file` lands.
- **`LoadContext` is short-lived:** it borrows the manager for the duration of a single `load()` call and is dropped before the manager inserts the resulting asset. The borrow is structured in two phases: immutable (build ctx + get loader + run loader) → ctx dropped → mutable (insert asset).
- **`render_ctx` stays `Rc<RefCell<…>>`** so a loader can borrow it mutably if ever needed (no overlapping `borrow_mut()` is active during loading — it happens outside `begin_frame`).

### 7.5 Loading Flow

```
load::<Texture>(TextureMetadata::from_file("sprites/player.png", sampler_handle))
  → AssetsManager: find loader for Texture (by TypeId)
  → ErasedLoader::load_erased(Box::new(metadata), ctx)
    → TextureLoader::load(metadata, ctx)
      → (uses render_ctx from ctx to create GPU texture + upload pixels)
    → Result<Texture>
  → LoadContext dropped (immutable borrow of AssetsManager ends)
  → AssetStorage<Texture>: insert → Handle<Texture>
  → Return Handle<Texture>
```

- `load::<T>(metadata)` is the primary API. The metadata carries everything needed to create the asset, including `Handle`s to dependencies.
- `load_from_file(path)` will be implemented when serialization lands — it reads a metadata file from disk, resolves dependency paths → handles, then delegates to `load(metadata)`.

### 7.6 Nested Dependencies

Assets can depend on other assets:
- `MaterialTemplate` depends on `Shader` assets (via `Handle<Shader>` in `MaterialTemplateMetadata`).
- `Material` depends on a `MaterialTemplate` handle.
- `Mesh` may depend on `Material` (for default material assignment).

**Strategy (V1): Caller-resolved dependencies.** The caller resolves dependencies *before* constructing the metadata — e.g. loads `Shader`s first, then passes `Handle<Shader>`s into `MaterialTemplateMetadata`. The loader retrieves already-loaded deps via `ctx.assets.get_asset(handle)` if it needs to inspect them at load time. Re-entrant loading (a loader calling `load()` for new assets) is *not* supported — this keeps the borrow model simple. When serialization lands, `load_from_file` will automate the resolve-deps-then-load flow.

### 7.7 Operations

| Operation | Signature | Description |
|-----------|-----------|-------------|
| **Insert** | `insert_asset<T: Asset>(asset: T) -> Handle<T>` | Store a pre-constructed asset. No loader invoked. |
| **Load** | `load<T: Asset>(metadata: T::Metadata) -> Result<Handle<T>>` | Find loader by asset TypeId → run loader with metadata → store → return handle. |
| **Load from file** | `load_from_file<T: Asset>(path) -> Result<Handle<T>>` | *(Not yet implemented)* Read metadata file → resolve dependency paths → `load(metadata)`. |
| **Remove** | `remove_asset<T: Asset>(handle) -> Option<T>` | Free slot, bump generation. Stale handles return `None`. |
| **Register loader** | `register_loader<L: AssetLoader>(loader)` | Register a loader for an asset type (one loader per type; re-register overwrites). |
| **Access** | `get_asset(handle) -> Option<&T>` / `get_asset_mut(handle) -> Option<&mut T>` | Typed access with generational validation. |

---

## 8. Material System

### 8.1 The Split: MaterialTemplate + Material

The core of the material model. **`MaterialTemplate` is the recipe (an asset); `Material` is the instance (lightweight).**

**`MaterialTemplate` (the recipe — an asset):**
- Shaders (vertex + fragment, as `Handle<Shader>`)
- Vertex buffer layout
- Blend state
- Depth/stencil state
- Primitive topology
- Uniform layout definitions (name, type, binding slot, visibility for each uniform)

Loaded once, shared by reference (via `Handle<MaterialTemplate>`). Goes through the asset system, has a loader, is stored in `AssetStorage`. Drives pipeline compilation.

**`Material` (the instance — immutable):**
- `Handle<MaterialTemplate>` — reference to the recipe
- Unique uniform values (`HashMap<String, UniformValue>` — typed, serializable)
- Texture bindings (`HashMap<u32, Handle<Texture>>`)

Materials are **immutable**: once loaded from metadata, they cannot be changed. No `dirty` flag, no mutators. To change a material, load a new one. This removes the invalidation path — the GPU uniform buffer and bind group are derived caches built once, never rebuilt.

The GPU-derived state (uniform buffer offset, bind group) is **not stored on the `Material`** — it lives in the `BindGroupAllocator` / `MaterialUniformPool` on `RenderContext`. This keeps the `Material` pure data (serializable) and the GPU state in the renderer layer.

```rust
pub struct MaterialTemplate {
    metadata: MaterialTemplateMetadata,   // owned — the asset's identity
}

pub struct MaterialTemplateMetadata {
    pub vertex_shader: Handle<Shader>,
    pub fragment_shader: Handle<Shader>,
    pub vertex_buffer_layout: VertexBufferLayout,
    pub blend_state: BlendMode,              // engine-native (not wgpu::BlendState)
    pub depth_stencil: Option<DepthStencilConfig>,
    pub topology: Topology,                   // engine-native
    pub uniform_layout: Vec<UniformBinding>,
    pub texture_layout: Vec<TextureBinding>,
}

pub struct Material {
    metadata: MaterialMetadata,
}

pub struct MaterialMetadata {
    template: Handle<MaterialTemplate>,
    uniforms: HashMap<String, UniformValue>,  // typed, keyed by name
    textures: HashMap<u32, Handle<Texture>>,  // keyed by binding slot
}
```

### 8.2 Why This Is Better

**Pipeline compilation is deduplicated.** The `wgpu::RenderPipeline` is derived from the template, not the instance. Two materials sharing the same template share the same compiled pipeline. The pipeline cache (in `RenderContext`) keys off template properties:
- Fewer cache entries (one per template, not one per material).
- No redundant pipeline compilations.
- First material using a template pays the compilation cost. All others reuse it.

**Batching is cleaner.** The `BatchKey2D` sort key becomes:

```rust
pub struct BatchKey2D {
    template: u64,   // MaterialTemplate handle id (integer comparison)
    texture: u64,    // primary texture handle id
}
```

Template handle replaces the old `shader` field. Since templates are assets with generational keys, comparison is a single integer check — faster than comparing shader pairs. Blend state is in the template, so it's already captured by the template key and removed from `BatchKey`.

**Hot-reload is clean.** When a shader changes, reload the `MaterialTemplate` asset. All materials referencing it pick up the new pipeline on the next frame. The handle indirection handles it — no need to track which materials used which shaders.

**Materials are typed without runtime cost.** The template's uniform layout defines what uniforms the material expects. When a `Material` sets a uniform by name, the lookup goes through the template's layout. The bind group creation is fully driven by the template. This gives type safety at the material level without compile-time generics — the template *is* the type.

### 8.3 PipelineKey

The `PipelineCache` keys off `(Handle<MaterialTemplate>, TextureFormat)`:

```rust
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct PipelineCacheKey {
    material_template_handle: Handle<MaterialTemplate>,
    target_format: wgpu::TextureFormat,
}
```

When `RenderTarget` encounters a new template for the first time, it calls `get_or_compile_pipeline()`, checks the cache, and compiles if missing.

### 8.4 Uniform Layout

The template defines a list of uniform bindings (in `uniform.rs`):

```rust
pub struct UniformBinding {
    pub name: String,
    pub ty: UniformType,          // Mat4, Vec4, F32 (grows as needed)
    pub binding_slot: u32,
    pub visibility: ShaderStage,  // Vertex, Fragment, Both (engine-native)
}

pub enum UniformType {
    Mat4,
    Vec4,
    F32,
}

pub enum UniformValue {
    Mat4(Mat4),
    Vec4(Vec4),
    F32(f32),
}
```

Materials are constructed via builders on `MaterialMetadata`:

```rust
impl MaterialMetadata {
    pub fn with_uniform(self, name: impl Into<String>, value: UniformValue) -> Self;
    pub fn with_texture(self, binding: u32, texture: Handle<Texture>) -> Self;
}
```

### 8.5 Load-Time Validation

`MaterialLoader` validates the material against its template at load time:
- Every uniform declared in the template must be provided with a value of the correct type.
- No extra uniforms beyond what the template declares.
- Every texture binding declared in the template must be provided.
- No extra texture bindings beyond what the template declares.

Validation failures produce `AssetError::DependencyValidationFailure { asset_name, dependency_name, reason }`. This ensures that by the time a `Material` exists in the asset store, it is guaranteed to match its template — draw-time never handles mismatches.

### 8.6 Caching Summary

| Cache | Key | Lives on | When compiled/rebuilt |
|-------|-----|----------|------------------------|
| Pipeline cache | `(Handle<MaterialTemplate>, TextureFormat)` | `RenderContext` (via `PipelineCache`) | First draw with a new template + target format |
| Bind group cache | `Handle<Material>` | `RenderContext` (via `BindGroupAllocator`) | First draw with a new material (immutable → never rebuilt) |
| Material uniform buffer | — (shared) | `RenderContext` (via `MaterialUniformPool`) | Batch-built when material set changes |
| Scene uniform buffer | — (per render target) | `RenderTarget` (via `UniformArena`) | Every render target (scene globals change each frame) |

**Centralized caching:** All GPU-derived caches live in `RenderContext` (pipeline cache, bind group allocator, material uniform pool) — not co-located on `Material`. This keeps `Material` pure data (serializable) and GPU state in the renderer layer. Immutable materials mean the bind group cache is a perfect cache: build once, look up forever, never invalidate.

### 8.7 Material as Asset vs Runtime Object

`MaterialTemplate` is clearly an asset (loaded from file, shared, hot-reloadable). `Material` (the instance) supports **both**:
- **As an asset:** Materials defined in data files (`.mat.toml`), loaded by a `MaterialLoader`, stored in `AssetStorage`. Good for data-driven workflows.
- **As a runtime object:** Materials created in code (`Material::new(template)`), stored in user-owned collections. Good for procedural materials (e.g., each enemy gets a material with a unique tint).

The `Handle<MaterialTemplate>` inside `Material` bridges the two worlds.

### 8.8 Default Material

For the common case (textured sprite, no special state), a default `MaterialTemplate` is provided:
- Vertex shader: standard 2D position + UV transform
- Fragment shader: sample texture, multiply by color
- Blend: alpha blending
- Uniforms: transform (Mat4), color (Vec4)

The default material is a white 1×1 texture + this template. Used as the fallback when a command has no explicit material. A `QuadCmd` with no material and no texture renders as a solid colored quad.

---

## 9. Rendering Pipeline

### 9.1 Command Structures

Render commands are struct-based and extensible:

```rust
pub struct QuadCmd {
    pub transform: Mat4,                    // or position + rotation + scale
    pub texture: Option<Handle<Texture>>,   // overrides material texture if set
    pub material: Option<Handle<Material>>, // overrides default material
    pub color: [f32; 4],                    // tint/multiply
    pub layer: f32,                         // render order
    pub uv_rect: Option<UVRect>,            // texture sub-region
}
```

**Texture on command overrides material texture.** This allows using the same material for different sprites — the material defines the shader and blend state, the command provides the specific texture.

### 9.2 Batching vs Instancing

Both batching and instancing are supported, and the choice is per-scenario:

**Batching (merged vertex buffer):**
- Best for 2D quads/sprites where each quad has different textures or per-vertex data.
- The batcher merges consecutive commands with the same `BatchKey` into a single vertex buffer and issues one draw call.
- `SpriteBatch` accumulates quads into a dynamic vertex buffer, then flushes with one draw.

**Instancing:**
- Best for 3D (and 2D) when many identical meshes share the same material and template — only the transform (instance data) differs.
- `Render3D::draw_instanced(mesh, material, instances: &[InstanceData])` uploads instance data as a separate buffer and issues one `draw_indexed` with `instance_count`.
- Instance data = per-instance transform (and any per-instance attributes).

**When to use which:**

| Scenario | Technique | Why |
|----------|-----------|-----|
| 2D sprites with different textures | Batching (merged VB) | Texture switches are the cost; merge vertices and bind one texture atlas, or group by texture |
| 2D sprites sharing one texture atlas | Batching or instancing | Both work; batching is simpler for quads |
| 3D repeated meshes (trees, rocks) | Instancing | Same mesh + material, only transform differs — instance buffer is compact |
| 3D unique meshes | Neither (direct draw) | Each mesh is its own draw call; batching by material reduces pipeline switches |
| 3D transparent objects | Sorting + direct draw | Back-to-front depth sort required; can't batch across sort boundaries |

**Sort key drives the decision:**
- `BatchKey2D = { template, texture }` — consecutive commands with the same key are batched (merged VB) or instanced.
- `BatchKey3D = { template, mesh, material }` — same key + same mesh → instance; same key + different mesh → separate draws (grouped to minimize pipeline switches).

### 9.3 Batching Process

1. Collect all commands during the frame.
2. Sort by `(layer, BatchKey, z)` — minimizes state changes (pipeline switches, texture bindings).
3. Group consecutive commands with the same `BatchKey`:
   - **2D:** Merge into a single vertex buffer, one draw call (batching).
   - **3D same mesh:** Instance buffer, one `draw_indexed` (instancing).
   - **3D different mesh:** Separate draw calls, grouped to share pipeline.
4. For each batch:
   - Look up pipeline from cache (keyed by template).
   - Create bind group from material uniforms + textures.
   - Upload uniform data via uniform arena.
   - Submit draw call via `RenderTarget::submit_draw_batch(DrawBatch)`.

### 9.4 Frame Lifecycle

```
Frame start
  → RenderContext::begin_frame()                           [nova-core]
    → acquires surface texture, creates command encoder
    → returns Frame<'a> (owns all per-frame state)
  → Application: on_update(scene_data)                      [user crate]
  → Render2D::new(&mut frame) / Render3D::new(&mut frame)    [nova-2d / nova-3d]
    → Collect commands (draw_quad, draw_mesh, etc.)
  → Drop Render2D/Render3D
    → Batcher2D/Batcher3D: sort collected commands
    → Batcher2D/Batcher3D: flush — for each batch:
      → RenderTarget::submit_draw_batch(DrawBatch)           [nova-core]
        → pipeline cache lookup (from RenderContext)
        → bind group creation (from RenderContext)
        → upload uniforms via frame's uniform arena
        → record into frame's command encoder
  → Drop Frame                                               [nova-core]
    → flush uniform arena (submit staging buffers)
    → finish + submit command encoder to GPU queue
    → present surface texture
```

### 9.5 2D vs 3D

2D and 3D share the same core infrastructure (`GraphicsContext`, `RenderContext`, asset system, pipeline cache — all in `nova-core`). They specialize in their own crates through:

- **Different command types** (`QuadCmd` in `nova-2d` vs `MeshCmd` in `nova-3d`).
- **Different batchers** (`Batcher2D` sorts by layer; `Batcher3D` sorts by material then distance for transparency).
- **Different shaders/materials** (2D templates use orthographic projection; 3D uses perspective).
- **Different uniform sets** (3D materials have camera matrices, lighting uniforms; 2D has simpler transforms).
- **3D-only concepts** (`Camera3D`, `LightSystem`, `SceneUniforms`, Culling) live in `nova-3d`.

The high-level API (`Render2D` in `nova-2d`, `Render3D` in `nova-3d`) presents different interfaces but both delegate to the same `RenderContext` in `nova-core`.

A 2D scene and a 3D scene can render into the same `Frame` as two passes (3D world with depth, 2D UI without depth).

---

## 10. High-Level Renderer

### 10.1 Render-Target-Scoped Borrowers

`Render2D` (in `nova-2d`) and `Render3D` (in `nova-3d`) are render-target-scoped borrowers of `RenderTarget` — not `RenderContext`. The flow is: `RenderContext` creates a `Frame` via `begin_frame()`, the caller creates a `RenderTarget` from `&mut RenderContext` + `frame.view()`, then dimension crates borrow the `RenderTarget` to collect commands and submit batches.

```rust
// In nova-2d
pub struct Render2D<'a> {
    target: &'a mut RenderTarget<'a>,
    batcher: Batcher2D,
}

impl<'a> Render2D<'a> {
    pub fn new(target: &'a mut RenderTarget<'a>) -> Self {
        Render2D { target, batcher: Batcher2D::new() }
    }
}
```

- `Render2D::new(&mut target)` borrows the `RenderTarget` for the duration of rendering.
- When `Render2D` is dropped, the batcher flushes — sorts commands and calls `target.submit_draw_batch()` for each batch.
- The `RenderTarget` is submitted separately (after all dimension-specific borrowers are done), which triggers the GPU submission. Then `Frame::present()` presents the surface.
- This prevents aliasing — you can't have `Render2D` and `Render3D` borrowing the same `Frame` simultaneously.

No trait extensions, no orphan rule issues. Clean constructor pattern.

### 10.2 Command API

```rust
impl<'a> Render2D<'a> {
    pub fn draw_quad(&mut self, cmd: QuadCmd);
    pub fn draw_sprite(&mut self, texture: Handle<Texture>, transform: Mat4);
    pub fn draw_text(&mut self, text: &str, font: Handle<Font>, transform: Mat4);
}

impl<'a> Render3D<'a> {
    pub fn draw_mesh(&mut self, mesh: Handle<Mesh>, material: Handle<Material>, transform: Mat4);
    pub fn draw_instanced(&mut self, mesh: Handle<Mesh>, material: Handle<Material>, instances: &[InstanceData]);
}
```

- Commands are collected, not executed immediately. This allows sorting and batching before GPU submission.
- The API is ergonomic — the user doesn't think about pipelines, bind groups, or upload queues.

---

## 11. Application Layer

### 11.1 ApplicationProxy Trait

```rust
pub trait ApplicationProxy {
    fn on_update(&mut self, ctx: &ApplicationContext, dt: Duration);
    fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame);
}
```

- `on_update` called in the fixed-timestep loop (already implemented).
- `on_render` called once per `RedrawRequested`, after the fixed-timestep `on_update` loop.
- The hardcoded `render()` free function in `handler.rs` is replaced by `begin_frame` → `RenderTarget::new` → `on_render` → `submit` → `Frame::present`.

### 11.2 AppContext

```rust
pub struct ApplicationContext {
    pub render: Arc<RenderContext>,
    pub assets: &AssetManager,
    pub window: &dyn WindowApi,
}
```

- Exposes `RenderContext` + `AssetManager` + `WindowApi` only. No raw `wgpu` handles.
- Passed to `ApplicationProxy::on_update()` and `ApplicationProxy::on_render()`.
- This is the **only** interface the application code sees.

### 11.3 Render Loop (Target)

```rust
WindowEvent::RedrawRequested => {
    // Update phase (fixed timestep, already implemented)
    let mut dt = self.frame_clock.restart();
    while dt >= self.frame_time {
        proxy.on_update(ctx, self.frame_time);
        dt -= self.frame_time;
    }

    // Render phase (new)
    let mut frame = ctx.render.begin_frame();
    let mut target = RenderTarget::new(&mut ctx.render, frame.view());
    proxy.on_render(ctx, &mut target);
    target.submit();
    frame.present(ctx.render.queue());
    // frame + target dropped here
}
```

### 11.4 Surface Loss Recovery

Replace `panic!("surface lost")` with reconfigure + skip:

```rust
wgpu::CurrentSurfaceTexture::Lost => {
    ctx.render.reconfigure_surface();
    return; // skip this frame
}
```

---

## 12. Default Resources

`nova-core` ships built-in WGSL shaders via `include_str!`:

```
nova-core/src/graphics/defaults/
    shader_2d_flat.wgsl         ← colored quads (no texture)
    shader_2d_textured.wgsl     ← textured quads
    shader_3d_unlit.wgsl        ← flat-colored 3D (future)
    shader_3d_pbr.wgsl          ← PBR 3D (future)
```

Registered at `RenderContext` init, exposed as default handles. `nova-2d` and `nova-3d` rely on these defaults. Only specialized effects bring custom shaders, loaded through the same `AssetManager::load` path.

---

## 13. Re-exports

Each dimension crate re-exports the core types its users need, so applications can depend on just `nova-2d` (or `nova-3d`) without also depending on `nova-core` explicitly:

```rust
// In nova-2d/src/lib.rs
pub use nova_core::{
    AppContext, ApplicationProxy, AssetManager, Handle,
    Material, MaterialTemplate, Shader, Texture, Mesh,
    RenderContext, Frame, RenderPass, DrawBatch,
};
```

This way, `use nova_2d::*` gives you everything you need for a 2D application.

---

## 14. Implementation Roadmap

### Phase 1: nova-core foundation

1. **`RenderContext`** — wrap `GraphicsContext` behind `Mutex`, add `device()`/`queue()`/`surface_format()` accessors, surface loss recovery, configurable present mode.
2. **`Frame` + `RenderTarget` + `RenderPass`** — surface lifecycle, view-agnostic rendering scope, render pass recording, uniform arena.
3. **Asset system core** — `AssetStorage<T>`, `Handle<T>`, `Asset`/`AssetLoader`/`ErasedLoader` traits, `LoadContext` (already partially implemented — refactor to use `Arc<RenderContext>`).
4. **`Shader` + `Texture` assets** — basic loaders (WGSL text loader, PNG texture loader with GPU upload).
5. **`MaterialTemplate`** — struct, uniform layout, loader (with nested shader dependency), `PipelineCacheKey`.
6. **`Material`** — struct, `set_uniform`/`set_texture`, `Handle<MaterialTemplate>` reference, dirty-flag bind group/uniform buffer caching.
7. **`DrawBatch`** — the contract struct.
8. **`PipelineCache` + `BindGroupAllocator`** — in `RenderContext`.
9. **Default material** — white 1×1 texture + default template. The fallback for commands without explicit materials.
10. **Replace `render()` free function** — `begin_frame` → `RenderTarget::new` → `on_render` → `submit` → `Frame::present`. Add `on_render` to `ApplicationProxy`.

### Phase 2: nova-2d

11. **`QuadCmd` + `BatchKey2D`** — 2D command struct and sort key.
12. **`Batcher2D`** — collect, sort by `(layer, template, texture, z)`, produce `DrawBatch`es.
13. **`Render2D`** — render-target-scoped borrower of `RenderTarget`, command collection, delegates to `Batcher2D` + `RenderTarget::submit_draw_batch()`.
14. **`SpriteBatch`** — dynamic vertex buffer builder for quads.
15. **`Camera2D`** — orthographic projection → uniform bytes.
16. **2D default material/template** — embedded WGSL shaders for standard 2D sprite rendering.
17. **End-to-end 2D rendering** — first visible output on screen.

### Phase 3: nova-3d

18. **`MeshCmd` + `BatchKey3D`** — 3D command struct and sort key (with depth sorting for transparency).
19. **`Batcher3D`** — collect, sort, produce `DrawBatch`es. Instancing for repeated meshes.
20. **`Render3D`** — render-target-scoped borrower of `RenderTarget`, command collection.
21. **`Camera3D`** — perspective projection → uniform bytes.
22. **`LightSystem`** — directional, point, spot light structs.
23. **`SceneUniforms`** — camera + lighting data, uploaded once per frame.
24. **3D default material/template** — embedded WGSL shaders for standard 3D rendering.
25. **End-to-end 3D rendering** — first 3D output on screen.

### Phase 4: Application layer + polish

26. **`AppContext` + `ApplicationProxy`** — the final API surface. Re-exports from dimension crates.
27. **Hot-reload** — asset reload propagation through template handles.
28. **Optional: `nova` umbrella crate** — facade with feature flags for 2d/3d.
29. **Async loading** (future) — `LoadingHandle`, deferred resolution.
30. **ECS integration** (future) — optional backend.
31. **Culling** (future) — frustum culling in `nova-3d`.

---

## 15. Open Questions (deferred, not blocking)

| Question | Default for now | Revisit when |
|----------|-----------------|--------------|
| Multi-window / multi-surface | Single surface | Need arises |
| Async resource loading | Synchronous | Loading stalls become a problem |
| Uniform buffer arena vs one-per-material | One buffer per material | Material count > ~1000 |
| Asset deduplication | None (duplicate loads create duplicate handles) | Memory waste is measured |
| Hot-reload | Not implemented | Requested (architecture supports it via handles) |
| SPIR-V shaders | WGSL only | Need arises |
| Multithreaded rendering | Single-threaded | Measured need (`Arc + Mutex` keeps the option open) |
| Bind group layout: explicit vs reflected | Explicit declaration | Reflection can be added later as a convenience helper |
| Vertex layout ownership | 2D/3D crates ship `Vertex2D`/`Vertex3D` structs with a `VertexBufferLayout` const | — |

---

## 16. Dependency Notes

### Current dependencies

```toml
[dependencies]
winit = "0.30.13"
wgpu = "30.0.0"
pollster = "1.0.1"    # block_on for async GPU init
```

### Planned additions

| Crate | When | Purpose |
|------|------|---------|
| `image` | Phase 1 (TextureLoader) | Decode PNG/JPG for texture loading |
| `glam` | Phase 1 (math) | Vectors, matrices, quaternions for cameras and transforms |
| `bytemuck` | Phase 1 | Safe `#[repr(C)]` casting for vertex/uniform buffer uploads |

### Feature flags (planned)

```toml
[features]
default = ["graphics"]
graphics = ["dep:wgpu"]
audio = []           # future
2d = []
3d = []
```

---

## 17. Glossary

| Term | Meaning |
|------|---------|
| **Handle** | A typed, `Copy` generational index referencing an asset in an `AssetStorage`. |
| **AssetStorage** | A typed arena storing assets of one type, with generational slot management. |
| **AssetManager** | Container of all `AssetStorage<T>` instances + loader registry. |
| **Asset** | A long-lived engine resource loaded from file (or constructed once), referenced by `Handle<T>`, shared across consumers. |
| **Loader** | A trait-implementing struct that turns file bytes into a GPU asset. |
| **GraphicsContext** | Internal, raw wgpu resources (Surface, Device, Queue, Config). `pub(crate)`. |
| **RenderContext** | Public hub wrapping `GraphicsContext` via `Arc<Mutex<...>>`. Owns pipeline cache + bind group allocator. |
| **Frame** | Surface lifecycle: owns the surface texture + view. Lightweight. `present()` presents to screen. |
| **RenderTarget** | View-agnostic rendering scope: command encoder + uniform arena. Borrows `&mut RenderContext`. `submit()` finishes encoder + submits to queue. |
| **RenderPass** | Scoped recording context that borrows a `RenderTarget` mutably. |
| **MaterialTemplate** | Asset defining the rendering recipe (shaders, layouts, blend state, uniform definitions). Shared. Drives pipeline compilation. |
| **Material** | Instance of a template. Holds unique uniform values and texture bindings. Lightweight. |
| **UniformArena** | Per-render-target transient buffer for scene-global uniforms (camera, time). Group 0. |
| **MaterialUniformPool** | Shared persistent buffer for all material uniforms. Batch-built. Group 1. |
| **PipelineCache** | `HashMap<(PipelineCacheKey, format), RenderPipeline>` — avoids recompilation. Lives in `RenderContext`. |
| **PipelineCacheKey** | Cache key for `wgpu::RenderPipeline`: `(Handle<MaterialTemplate>, TextureFormat)`. |
| **BindGroupAllocator** | Per-material `wgpu::BindGroup` cache keyed by `Handle<Material>`. Immutable materials = perfect cache. Lives in `RenderContext`. |
| **DrawBatch** | Contract struct passed to `RenderTarget::submit_draw_batch()`. Contains pipeline key + material handle + vertex data. |
| **BatchKey2D** | Sort key for batching 2D commands: `{ template, texture }`. |
| **BatchKey3D** | Sort key for batching 3D commands: `{ template, mesh, material }`. |
| **Batcher2D / Batcher3D** | Collects, sorts, and flushes dimension-specific commands. Produces `DrawBatch`es. |
| **Render2D / Render3D** | Render-target-scoped borrower of `RenderTarget`. Provides the dimension-specific command API. |
| **AppContext** | The only interface exposed to application code. Contains `RenderContext` + `AssetManager` + `WindowApi`, no raw wgpu. |
| **ApplicationProxy** | User-implemented trait. The entry point for application logic (`on_update`, `on_render`). |