# Nova Engine — Final Architecture

> **Status:** Authoritative — supersedes `nova-engine-architecture.md` and `rendering-architecture-plan.md`.
> **Last updated:** 2026-08-23

---

## 1. Vision & Principles

Nova Engine is a simple, efficient 2D/3D multimedia framework in Rust built on `wgpu`.

**Guiding principles:**

1. **One renderer, not two.** 2D and 3D are configurations and thin helpers over a single shared rendering core — never separate renderers.
2. **Graphics first, correctness first.** Render the foundation before audio/physics. Prefer correct, ergonomic architecture over micro-optimization; performance comes from good structure.
3. **Type safety over raw handles.** Generational, typed `Handle<T>` references. Stale handles resolve to `None`, never use-after-free.
4. **Nothing outside `nova-core::graphics` touches raw `wgpu`.** `GraphicsContext` stays `pub(crate)`. `RenderContext` is the only public face of the GPU.
5. **Synchronous and single-threaded** for V1. No async loading, no multithreaded rendering. `Arc<Mutex>` keeps the door open for later.
6. **Data-driven where it counts.** Material templates and shaders are assets loaded from files; per-instance materials are lightweight runtime objects that reference them.
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
| `Asset`, `AssetLoader`, `ErasedLoader`, `LoadContext` | Extensible loader system |
| `Shader`, `Texture`, `Sampler`, `Mesh` | GPU-backed assets used by both dimensions |
| `MaterialTemplate`, `Material` | The material model (recipe + instance) |
| `WindowApi` | Window abstraction — shared |
| `AppContext`, `ApplicationProxy` | Application entry point — shared |
| `DrawBatch` | The dimension-agnostic submission contract |
| `UniformArena` | Per-frame transient uniform uploads (camera, scene globals) |
| Default WGSL shaders | Embedded via `include_str!`, registered at init |

**`nova-2d` (2D-specific helpers):**

| Component | Why it's here |
|-----------|---------------|
| `Render2D` | Frame-scoped borrower — 2D command API |
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
| `Render3D` | Frame-scoped borrower — 3D command API |
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

Batchers (`Batcher2D`, `Batcher3D`) are **dimension-specific** and live in `nova-2d` / `nova-3d`. They collect commands, sort them, and produce `DrawBatch` structs that they hand to `Frame::submit_draw_batch()`.

```
Render2D::draw_quad(cmd)                      Render3D::draw_mesh(cmd)
  → Batcher2D: collect cmd                      → Batcher3D: collect cmd
  → (at frame end) Batcher2D: sort              → (at frame end) Batcher3D: sort
  → Batcher2D: flush                            → Batcher3D: flush
    → Frame::submit_draw_batch(DrawBatch)         → Frame::submit_draw_batch(DrawBatch)
      → pipeline cache lookup (RenderContext)      → pipeline cache lookup (RenderContext)
      → bind group creation (RenderContext)        → bind group creation (RenderContext)
      → uniform arena (frame-scoped)               → uniform arena (frame-scoped)
      → command encoder (frame-scoped)             → command encoder (frame-scoped)
    → (on Frame::drop) GPU submit + present        → (on Frame::drop) GPU submit + present
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
│  ├── Render2D / Render3D (borrow Frame, commands)         │
│  ├── Batcher2D / Batcher3D (dimension-specific sorting)   │
│  └── Command structures (QuadCmd, MeshCmd, etc.)          │
├──────────────────────────────────────────────────────────┤
│  Frame & RenderPass                  [nova-core]         │
│  ├── Frame (surface texture + encoder + uniform arena)    │
│  ├── RenderPass (scoped recording, borrows Frame mutably) │
│  ├── submit_draw_batch() — the contract for batchers      │
│  └── Drop: flush uploads + submit + present               │
├──────────────────────────────────────────────────────────┤
│  RenderContext                        [nova-core]         │
│  ├── Pipeline cache (keyed by MaterialTemplate)           │
│  ├── Bind group allocator                                 │
│  ├── begin_frame() → Frame                                │
│  ├── Surface management (resize, recover, present mode)   │
│  └── Holds: Arc<GraphicsContext> (via Mutex)              │
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
│  └── Assets: Shader, Texture, Sampler, Mesh,              │
│      MaterialTemplate, Material                           │
└──────────────────────────────────────────────────────────┘
```

### 3.1 Layer Responsibilities

| Layer | Crate | Owns | Knows About | Exposes To Above |
|-------|-------|------|-------------|------------------|
| `GraphicsContext` | nova-core | GPU device, queue, surface | wgpu only | Raw GPU access (to `RenderContext` only) |
| `RenderContext` | nova-core | Pipeline cache, bind group allocator, surface | `GraphicsContext` + asset handles | `begin_frame()` → `Frame`, `device()`/`queue()` accessors |
| `Frame` | nova-core | Surface texture, command encoder, uniform arena, frame index | `RenderContext` (borrowed) | `begin_pass()`, `submit_draw_batch()` (to batchers) |
| `RenderPass` | nova-core | Scoped wgpu render pass recording | `Frame` (borrowed mutably) | Draw methods (to high-level renderer) |
| High-Level Renderer | nova-2d / nova-3d | Command collection, batching, sorting | `Frame` + `RenderPass` API | `Render2D`/`Render3D` borrowers (to Application) |
| Application | user crate | Scene state, game logic | `AppContext` | Nothing — this is the top |

**Key rules:**
- No layer reaches below its own level. Application never sees `wgpu`. High-level renderer never touches `wgpu` directly — it goes through `Frame` / `RenderPass`.
- `nova-2d` and `nova-3d` never import `wgpu`. They go through `Frame`'s API.
- `nova-core` does not know about `QuadCmd`, `MeshCmd`, `BatchKey2D`, or `BatchKey3D`. It only knows about `DrawBatch`.
- `RenderContext` is long-lived (created once at startup). `Frame` is short-lived (created and dropped each frame). This separation prevents stale frame state from leaking between frames.

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
    inner: Mutex<GraphicsContext>,     // Arc-wrapped for shared ownership
    pipeline_cache: PipelineCache,     // keyed by PipelineKey (from MaterialTemplate)
    bind_group_allocator: BindGroupAllocator,
    // future: depth_texture_pool, default material templates
}
```

> **Design constraint — don't let it become a god object.** Each subsystem (`PipelineCache`, `BindGroupAllocator`) is its own struct. `RenderContext` *coordinates* them, it does not *be* them.

### 5.3 Responsibilities

- **Pipeline cache**: Compiles and caches `wgpu::RenderPipeline` objects, keyed by `PipelineKey` (derived from `MaterialTemplate` properties + target format). First encounter with a new template triggers compilation; subsequent materials using the same template reuse the cached pipeline.
- **Bind group allocator**: Creates and manages `wgpu::BindGroup` objects from material instance data. Allocates from pooled descriptor sets to avoid per-frame allocation churn.
- **Frame lifecycle**: `begin_frame()` acquires the current surface texture, creates a command encoder, and returns a `Frame` object that scopes all frame-specific data. The `Frame`'s `Drop` impl submits and presents.
- **Surface management**: Resize, reconfigure, recover from loss (replace `panic!("surface lost")` with reconfigure + skip frame). Configurable present mode.
- **Resource accessors**: `device()`, `queue()`, `surface_format()` — the escape hatch for advanced/crate-internal use (e.g., asset loaders, `nova-2d`/`nova-3d` vertex buffer creation).

### 5.4 Public API (V1)

| Method | Description |
|--------|-------------|
| `device() -> MutexGuard<GraphicsContext>` (or accessor) | Access to `wgpu::Device` (for resource creation) |
| `queue() -> ...` | Access to `wgpu::Queue` (for buffer/texture writes) |
| `surface_format() -> TextureFormat` | Current surface format (needed for pipeline creation) |
| `begin_frame() -> Frame` | Acquire surface texture, create view, return `Frame` |
| `resize(width, height)` | Reconfigure surface on window resize |

### 5.5 Interior Mutability

Because `RenderContext` is shared via `Arc`, all methods take `&self`. Mutable state (surface config, pipeline cache) uses `Mutex`. Uncontended single-threaded `Mutex` is effectively free. This keeps the option open for multithreading later without refactoring.

---

## 6. Frame & RenderPass

### 6.1 Frame — Per-Frame Unit

Created by `RenderContext::begin_frame()`, consumed by `submit()` (or `Drop`). **Short-lived — RAII frame boundary.**

```rust
pub struct Frame<'a> {
    renderer: &'a RenderContext,
    view: wgpu::TextureView,              // surface texture view (or off-screen target)
    encoder: wgpu::CommandEncoder,         // owned, built up during the frame
    uniform_arena: UniformArena,          // per-frame uniform uploads (camera, scene globals)
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    frame_index: u64,                      // increments each frame (for double-buffering)
}
```

**What lives in `Frame` (per-frame, transient):**
- The acquired surface texture view (target for rendering)
- The command encoder (all draw commands recorded into this)
- The uniform arena (staging buffers for uniform data uploaded this frame)
- Frame index (for double/triple-buffering schemes)
- Per-frame allocation pools (transient bind groups, temporary buffers)

**What stays in `RenderContext` (persistent, shared across frames):**
- `GraphicsContext` (the GPU connection)
- Pipeline cache (pipelines persist across frames)
- Bind group allocator (the allocator pool persists; individual bind groups are per-frame)

**Lifecycle:**

```rust
impl RenderContext {
    pub fn begin_frame(&self) -> Frame<'_> {
        let surface_texture = self.acquire_surface_texture(); // handles loss/Outdated
        let view = surface_texture.texture.create_view(&Default::default());
        let encoder = self.device().create_command_encoder(/* ... */);
        Frame {
            renderer: self,
            view,
            encoder,
            uniform_arena: UniformArena::new(),
            color_format: self.surface_format(),
            depth_format: self.depth_format(),
            frame_index: self.next_frame_index(),
        }
    }
}

impl<'a> Drop for Frame<'a> {
    fn drop(&mut self) {
        // 1. Flush uniform arena — submit remaining staging buffers
        self.uniform_arena.flush(&self.renderer.queue());
        // 2. Submit the command encoder
        let cmd = self.encoder.finish();
        self.renderer.queue().submit(std::iter::once(cmd));
        // 3. Present the surface texture
        self.surface_texture.present();
    }
}
```

The `Drop` impl ensures the frame is always properly submitted and presented, even if the user panics.

### 6.2 RenderPass — Recording Scope

```rust
pub struct RenderPass<'frame> {
    // wraps wgpu::RenderPass<'frame>
    // borrows Frame mutably so only one pass is active at a time (wgpu rule)
}
```

- `begin_pass(desc) -> RenderPass` — opens a render pass on the frame's view (or a provided off-screen target).
- `set_pipeline(&mut self, &Pipeline)`
- `set_bind_group(&mut self, index, &BindGroup)`
- `set_vertex_buffer`, `set_index_buffer`
- `draw(...)`, `draw_indexed(...)`
- `draw_material(&mut self, &Material, ...)` — convenience: binds pipeline + bind groups + draws.

2D and 3D renderers build higher-level helpers on top of this (e.g., `SpriteBatch::flush(&mut self, pass: &mut RenderPass)`).

### 6.3 Off-Screen Rendering

`begin_pass` accepts an arbitrary `TextureView`, not just the surface view. The surface view is the default. This enables post-processing and render-to-texture for 3D without changing the API.

### 6.4 UniformArena

Per-frame uniform uploads for scene-global data (camera matrices, time, transforms) that doesn't belong in `Material`.

```rust
impl Frame {
    fn upload_uniform(&mut self, bytes: &[u8]) -> BindGroupEntry;
}
```

- Both `Camera2D` and `Camera3D` produce matrices and call `upload_uniform(...)`.
- Arena is reset each frame.
- Shared infrastructure in `nova-core` — camera types are just producers of uniform bytes.

### 6.5 submit_draw_batch — The Contract

```rust
impl<'a> Frame<'a> {
    pub fn submit_draw_batch(&mut self, batch: DrawBatch) {
        // 1. Look up pipeline from RenderContext's pipeline cache
        let pipeline = self.renderer.pipeline_cache.get_or_compile(
            &batch.template_key, &self.renderer.device()
        );
        // 2. Create bind groups from material data
        let bind_groups = self.renderer.bind_group_allocator.create(&batch.material);
        // 3. Upload uniform data via frame's uniform arena
        self.uniform_arena.upload_uniforms(&batch.uniform_data);
        // 4. Record render pass commands into the frame's command encoder
        let mut pass = self.encoder.begin_render_pass(&batch.render_pass_descriptor);
        pass.set_pipeline(pipeline);
        for (i, bg) in bind_groups.iter().enumerate() {
            pass.set_bind_group(i as u32, bg, &[]);
        }
        pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
        pass.draw(0..batch.vertex_count, 0..batch.instance_count);
    }
}
```

This is the dimension-agnostic submission interface. `Batcher2D` and `Batcher3D` both call it — they just produce different `DrawBatch` instances from different command types.

### 6.6 DrawBatch

The contract struct between dimension-specific batchers and the dimension-agnostic `RenderContext`:

```rust
pub struct DrawBatch {
    pub template_key: PipelineKey,       // derived from MaterialTemplate
    pub material: Material,              // or Handle<Material> + material data
    pub bind_groups: Vec<wgpu::BindGroup>,
    pub vertex_buffer: wgpu::Buffer,     // or a reference to a pooled buffer
    pub vertex_count: u32,
    pub instance_count: u32,
    pub uniform_data: Vec<u8>,           // per-batch uniform uploads
    pub render_pass_descriptor: wgpu::RenderPassDescriptor<'static>,
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
- `Frame` / `RenderPass` — per-frame scope objects.

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
pub trait Asset: 'static + Send + Sync {}

pub trait AssetLoader: 'static {
    type Asset: Asset;
    fn load(&mut self, path: &Path, ctx: &LoadContext) -> Result<Self::Asset, AssetError>;
    fn extensions(&self) -> Vec<String>;
}

pub(crate) trait ErasedLoader: 'static {
    fn load_erased(&mut self, path: &Path, ctx: &LoadContext) -> Result<Box<dyn Any>, AssetError>;
}
```

**`ErasedLoader` pattern:** wraps a concrete `AssetLoader` in a type-erased form so the asset manager can store heterogeneous loaders in a single collection. `load_erased` returns `Box<dyn Any>`, which the caller downcasts to the concrete asset type.

### 7.4 LoadContext

```rust
pub struct LoadContext<'a> {
    gfx: Arc<GraphicsContext>,       // for GPU uploads (texture, buffer creation)
    manager: &'a AssetManager,        // for nested dependency loads
}
```

- Passed to loaders. Provides GPU access (for texture upload, buffer creation) and `AssetManager` access (for nested loads — e.g., `MaterialTemplateLoader` loads `Shader` assets during its own load).
- **Why `Arc<GraphicsContext>` instead of `&GraphicsContext`:** `load()` becomes self-contained — no external ctx parameter, no borrow splitting at call sites. Loaders can freely access GPU resources without transient borrow lifetimes.

### 7.5 Loading Flow

```
load::<Texture>("sprites/player.png")
  → AssetManager: find loader for "png" extension
  → ErasedLoader::load_erased("sprites/player.png", ctx)
    → TextureLoader::load(path, ctx)
      → (uses GraphicsContext from ctx to upload to GPU)
    → Result<Texture>
  → AssetStorage<Texture>: insert → Handle<Texture>
  → Return Handle<Texture>
```

- `load::<T>()` (typed) is preferred over untyped `load_file()`.
- GraphicsContext (Arc) is passed to loaders that need GPU access via the `LoadContext`.

### 7.6 Nested Dependencies

Assets can depend on other assets:
- `MaterialTemplate` depends on `Shader` assets.
- `Material` depends on a `MaterialTemplate` handle.
- `Mesh` may depend on `Material` (for default material assignment).

**Strategy (V1): Immediate nested load.** The loader calls `ctx.load::<Shader>(...)` synchronously during its own load. The dependency is fully loaded before the parent asset is returned. Simple, blocks on I/O. Move to two-phase (metadata → resolve deps) only if loading stalls become a problem.

### 7.7 Operations

| Operation | Signature | Description |
|-----------|-----------|-------------|
| **Add** | `add<T: Asset>(asset: T) -> Handle<T>` | Store a pre-constructed asset. No GPU access needed. |
| **Load** | `load<T: Asset>(path) -> Result<Handle<T>>` | Read file → find loader by extension → run loader → store → return handle. |
| **Load with hint** | `load_with_hint<T, L>(path) -> Result<Handle<T>>` | Load using a specific loader type (disambiguation). |
| **Remove** | `remove<T: Asset>(handle) -> Option<T>` | Free slot, bump generation. Stale handles return `None`. |
| **Add loader** | `register_loader<L: AssetLoader>(loader)` | Register a new loader for specific file extensions. |
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

**`Material` (the instance — lightweight):**
- `Handle<MaterialTemplate>` — reference to the recipe
- Unique uniform values (the actual data for each uniform defined in the template)
- Texture bindings (handles to `Texture` assets)

A `Material` is almost free to create — it's just a handle + a small bag of values. You can spawn thousands of them without worrying about pipeline state duplication.

```rust
pub struct MaterialTemplate {
    vertex_shader: Handle<Shader>,
    fragment_shader: Handle<Shader>,
    vertex_layout: VertexBufferLayout,
    blend_state: BlendState,
    depth_stencil: Option<DepthStencilState>,
    topology: PrimitiveTopology,
    uniform_layout: Vec<UniformBinding>,  // name, type, slot, visibility
}

pub struct Material {
    template: Handle<MaterialTemplate>,
    uniforms: Vec<UniformValue>,      // indexed by uniform_layout slot
    textures: Vec<Handle<Texture>>,   // texture bindings
    // Derived, cached (per-instance):
    uniform_buffer: Option<wgpu::Buffer>,
    bind_groups: Vec<Option<wgpu::BindGroup>>,
    dirty: bool,
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

The `MaterialTemplate` produces (or contributes to) the `PipelineKey` directly:

```rust
impl MaterialTemplate {
    pub fn pipeline_key(&self) -> PipelineKey {
        PipelineKey {
            vertex_shader: self.vertex_shader,
            fragment_shader: self.fragment_shader,
            vertex_layout: self.vertex_layout.clone(),
            blend_state: self.blend_state,
            depth_stencil: self.depth_stencil,
            topology: self.topology,
        }
    }
}
```

When `RenderContext` encounters a new template for the first time, it calls `template.pipeline_key()`, checks the cache, and compiles if missing.

### 8.4 Uniform Layout

The template defines a list of uniform bindings:

```rust
pub struct UniformBinding {
    name: String,
    uniform_type: UniformType,   // Mat4, Vec3, Float, etc.
    binding_slot: u32,
    visibility: ShaderStage,     // Vertex, Fragment, or both
}

pub enum UniformType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Mat3,
    Mat4,
    FloatArray(usize),
    Vec4Array(usize),
}
```

When a `Material` sets a uniform:

```rust
impl Material {
    pub fn set_uniform(&mut self, name: &str, value: UniformValue) {
        // Look up slot in template's uniform_layout
        // Store in self.uniforms[slot]
        self.dirty = true;
    }

    pub fn set_texture(&mut self, binding: u32, texture: Handle<Texture>) {
        self.textures[binding] = texture;
        self.dirty = true;
    }
}
```

### 8.5 Caching Summary

| Cache | Key | Lives on | When compiled/rebuilt |
|-------|-----|----------|------------------------|
| Pipeline cache | `(template pipeline_key, target format)` | `RenderContext` (via `PipelineCache`) | First draw with a new template + target format |
| Bind group cache | — (per-material) | `Material` | When `dirty` (textures/uniforms changed) |
| Uniform buffer | — (per-material) | `Material` | When `dirty` (params changed) |

**Co-located vs centralized caching:** The pipeline cache is centralized in `RenderContext` (because pipelines are shared across materials by template). The bind group cache and uniform buffer are co-located on each `Material` (because they're per-instance). This avoids a separate cache layer to synchronize.

### 8.6 Material as Asset vs Runtime Object

`MaterialTemplate` is clearly an asset (loaded from file, shared, hot-reloadable). `Material` (the instance) supports **both**:
- **As an asset:** Materials defined in data files (`.mat.toml`), loaded by a `MaterialLoader`, stored in `AssetStorage`. Good for data-driven workflows.
- **As a runtime object:** Materials created in code (`Material::new(template)`), stored in user-owned collections. Good for procedural materials (e.g., each enemy gets a material with a unique tint).

The `Handle<MaterialTemplate>` inside `Material` bridges the two worlds.

### 8.7 Default Material

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
   - Submit draw call via `Frame::submit_draw_batch(DrawBatch)`.

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
      → Frame::submit_draw_batch(DrawBatch)                 [nova-core]
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

### 10.1 Frame-Scoped Borrowers

`Render2D` (in `nova-2d`) and `Render3D` (in `nova-3d`) are frame-scoped borrowers of `Frame` — not `RenderContext`. The flow is: `RenderContext` creates a `Frame` via `begin_frame()`, then dimension crates borrow the `Frame` to collect commands and submit batches.

```rust
// In nova-2d
pub struct Render2D<'a> {
    frame: &'a mut Frame<'a>,
    batcher: Batcher2D,
}

impl<'a> Render2D<'a> {
    pub fn new(frame: &'a mut Frame<'a>) -> Self {
        Render2D { frame, batcher: Batcher2D::new() }
    }
}
```

- `Render2D::new(&mut frame)` borrows the `Frame` for the duration of rendering.
- When `Render2D` is dropped, the batcher flushes — sorts commands and calls `frame.submit_draw_batch()` for each batch.
- The `Frame` itself is dropped separately (after all dimension-specific borrowers are done), which triggers the GPU submission and present via `Drop`.
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
- The hardcoded `render()` free function in `handler.rs` is replaced by `begin_frame` → `on_render` → `submit` (via `Frame::drop`).

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
    proxy.on_render(ctx, &mut frame);
    // frame dropped here → GPU submit + present
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
2. **`Frame` + `RenderPass`** — per-frame state, `begin_pass()`, `submit()` (or `Drop`), uniform arena.
3. **Asset system core** — `AssetStorage<T>`, `Handle<T>`, `Asset`/`AssetLoader`/`ErasedLoader` traits, `LoadContext` (already partially implemented — refactor to use `Arc<RenderContext>`).
4. **`Shader` + `Texture` assets** — basic loaders (WGSL text loader, PNG texture loader with GPU upload).
5. **`MaterialTemplate`** — struct, uniform layout, loader (with nested shader dependency), `pipeline_key()`.
6. **`Material`** — struct, `set_uniform`/`set_texture`, `Handle<MaterialTemplate>` reference, dirty-flag bind group/uniform buffer caching.
7. **`DrawBatch`** — the contract struct.
8. **`PipelineCache` + `BindGroupAllocator`** — in `RenderContext`.
9. **Default material** — white 1×1 texture + default template. The fallback for commands without explicit materials.
10. **Replace `render()` free function** — `begin_frame` → `on_render` → `Frame::drop`. Add `on_render` to `ApplicationProxy`.

### Phase 2: nova-2d

11. **`QuadCmd` + `BatchKey2D`** — 2D command struct and sort key.
12. **`Batcher2D`** — collect, sort by `(layer, template, texture, z)`, produce `DrawBatch`es.
13. **`Render2D`** — frame-scoped borrower of `Frame`, command collection, delegates to `Batcher2D` + `Frame::submit_draw_batch()`.
14. **`SpriteBatch`** — dynamic vertex buffer builder for quads.
15. **`Camera2D`** — orthographic projection → uniform bytes.
16. **2D default material/template** — embedded WGSL shaders for standard 2D sprite rendering.
17. **End-to-end 2D rendering** — first visible output on screen.

### Phase 3: nova-3d

18. **`MeshCmd` + `BatchKey3D`** — 3D command struct and sort key (with depth sorting for transparency).
19. **`Batcher3D`** — collect, sort, produce `DrawBatch`es. Instancing for repeated meshes.
20. **`Render3D`** — frame-scoped borrower of `Frame`, command collection.
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
| **Frame** | Per-frame object: surface texture view + command encoder + uniform arena. RAII — `Drop` submits + presents. |
| **RenderPass** | Scoped recording context that borrows a `Frame` mutably. |
| **MaterialTemplate** | Asset defining the rendering recipe (shaders, layouts, blend state, uniform definitions). Shared. Drives pipeline compilation. |
| **Material** | Instance of a template. Holds unique uniform values and texture bindings. Lightweight. |
| **UniformArena** | Per-frame transient buffer for camera/scene uniforms. |
| **PipelineCache** | `HashMap<(PipelineKey, format), RenderPipeline>` — avoids recompilation. Lives in `RenderContext`. |
| **PipelineKey** | Cache key for `wgpu::RenderPipeline`, derived from `MaterialTemplate` properties. |
| **DrawBatch** | Contract struct passed to `Frame::submit_draw_batch()`. Contains pipeline key + bind groups + vertex data + instance count. |
| **BatchKey2D** | Sort key for batching 2D commands: `{ template, texture }`. |
| **BatchKey3D** | Sort key for batching 3D commands: `{ template, mesh, material }`. |
| **Batcher2D / Batcher3D** | Collects, sorts, and flushes dimension-specific commands. Produces `DrawBatch`es. |
| **Render2D / Render3D** | Frame-scoped borrower of `Frame`. Provides the dimension-specific command API. |
| **AppContext** | The only interface exposed to application code. Contains `RenderContext` + `AssetManager` + `WindowApi`, no raw wgpu. |
| **ApplicationProxy** | User-implemented trait. The entry point for application logic (`on_update`, `on_render`). |