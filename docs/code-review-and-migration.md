# Nova Engine — Code Review & Migration Walkthrough

> **Companion to:** `docs/architecture.md` (the target architecture).
> **Purpose:** Track progress against the target architecture, identify remaining work, and lay out a step-by-step migration path.
> **Last updated:** 2026-08-28

---

## Part A — Progress Tracker

### Completed Steps

#### ✅ Step 0 — Immediate fixes (no new types needed)

**Goal:** Fix surface-loss panic and `AssetStorage` bounds panics.

**What was done:**
- **C4 — Surface loss recovery:** `handler.rs` no longer panics on `Surface::Lost`. `RenderContext::get_surface_texture` (formerly inline in `render()`) recreates the `GraphicsContext` via `reconfigure()` and skips the frame.
- **L2 — `AssetStorage` bounds safety:** `get_mut`/`remove` use `get_mut(handle.index)?` → return `None` on out-of-bounds index. `update_slot` uses `unwrap()` (the index is always valid — it comes from `self.empty_slot`). `HandleIndexOutOfBounds` error variant removed as unused.
- **`insert()`** returns `Handle<A>` directly (no `Result` — it can't fail).

**Files touched:** `app/handler.rs`, `graphics/context.rs`, `assets/storage.rs`, `assets/error.rs`

---

#### ✅ Step 1 — `RenderContext` (minimal)

**Goal:** Create the public render hub wrapping `GraphicsContext`.

**What was done:**
- **`RenderContext`** (`graphics/render.rs`) — wraps `GraphicsContext`. Provides `device()`, `queue()`, `resize_surface()`, `begin_frame()`.
- **`GraphicsConfiguration`** (`graphics/config.rs`) — new builder-style config struct: `power_preference`, `present_mode` (default `AutoVsync`), `alpha_mode`. Plumbed through `ApplicationBuilder::alter_graphics_configuration`. **Resolves C5 (present mode uncontrolled).**
- **`GraphicsContext::new` is synchronous** — `pollster::block_on` moved inside; callers no longer need `async`.
- **`reconfigure()` preserves `gfx_config`** — surface loss recovery reuses the same config.
- **`begin_frame()` handles all surface texture cases** — Success/Suboptimal → render; Timeout/Occluded/Validation → skip; Outdated → reconfigure + skip; Lost → recreate + skip. No panics.
- **`Rc<RefCell<RenderContext>>`** — shared between `ApplicationContext` and `AssetsManager`. Correct for single-threaded; `Arc<Mutex>` deferred until multithreading is needed.
- **`LoadContext` now holds `Rc<RefCell<RenderContext>>`** — aligns with target architecture (loaders access GPU via `RenderContext`, not raw `GraphicsContext`). **Resolves H2.**

**Files added:** `graphics/render.rs`, `graphics/config.rs`
**Files modified:** `graphics/context.rs`, `app.rs`, `app/handler.rs`, `assets.rs`, `assets/load.rs`

---

#### ✅ Step 2 — `Frame` + `RenderPass`

**Goal:** Per-frame abstraction with RAII submit + present, and a scoped render pass recording context.

**What was done:**
- **`Frame<'a>`** (`graphics/frame.rs`) — owns surface texture + view + command encoder. `begin_render_pass(desc)` returns a `RenderPass`. `submit(self)` submits the encoder and presents the surface texture (RAII frame boundary).
- **`RenderPassDescriptor<'a>`** (`graphics/render_pass.rs`) — builder-style descriptor:
  - `label: Option<&str>` — GPU debugger label
  - `color_clear: Option<Color>` — `Some` → clear to color; `None` → load existing content
  - `color_view: Option<&TextureView>` — `None` → frame's surface view; `Some` → off-screen target
  - `depth_clear: Option<f32>` — `None` → no depth; `Some(v)` → clear depth to v
  - Builder methods: `with_label`, `with_color_clear`, `with_color_view`, `with_depth_clear`
- **`RenderPass<'frame>`** (`graphics/render_pass.rs`) — wraps `wgpu::RenderPass<'frame>`. Borrows `Frame` mutably (one pass at a time). Draw methods: `set_pipeline`, `set_bind_group`, `set_vertex_buffer`, `set_index_buffer`, `draw`, `draw_indexed`.
- **`Color`** (`graphics/color.rs`) — engine-native color type with named constants (`BLACK`, `WHITE`, `RED`, `GREEN`, `BLUE`, `YELLOW`, `CYAN`, `MAGENTA`, `TRANSPARENT`). `Into<wgpu::Color>` impl decouples the API surface from wgpu.
- **Surface loss recovery** centralized in `RenderContext::begin_frame()` — no panics.

**Deferred (not blocking for 2D):**
- **`UniformArena`** — per-frame transient uniform uploads (camera/scene globals). Needed for cameras (Step 9) and 3D (Step 12).
- **Depth texture pool** — `depth_clear` is accepted but uses a placeholder view. Needed for 3D (Step 12). The `TODO` comment marks it.
- **`frame_index: u64`** — increments each frame for double-buffering. Cheap to add when needed.

**Files added:** `graphics/frame.rs`, `graphics/render_pass.rs`, `graphics/color.rs`

---

#### ✅ Step 3 — Replace `render()` + add `on_render`

**Goal:** The proxy controls rendering through the public API. The hardcoded clear-color pass is gone.

**What was done:**
- **`ApplicationProxy::on_render`** added: `fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame)`. The proxy receives both the context (for asset access) and the frame.
- **`render()` free function deleted** from `handler.rs`. Replaced by `Application::on_render()` which does: `begin_frame` → `proxy.on_render(ctx, &mut frame)` → `frame.submit()`.
- **`handler.rs` refactored** — now thin: dispatches to `process_events`. The old 90-line handler with inline rendering is gone.
- **`process_events` extracted** — returns `EngineResult`; errors convert to `engine_error` + `exit()`.
- **`on_update` extracted** — fixed-timestep loop moved into a named method.
- **`ApplicationContext` no longer leaks `GraphicsContext`** — holds `Rc<RefCell<RenderContext>>`. **Resolves H3.**
- **`nova-test` renders through `on_render`** — clears screen to `Color::BLUE` via `RenderPassDescriptor::default().with_color_clear(Color::BLUE)`.

**🎉 First visible milestone: a blue window rendered entirely through the public API.**

**Files modified:** `app.rs`, `app/handler.rs`, `nova-test/src/main.rs`

---

#### ✅ Step 4 — Refactor `AssetsManager` to use `RenderContext`

**Goal:** Align the asset system with the target architecture (loaders access GPU via `RenderContext`).

**What was done (during Step 1, carried forward):**
- `LoadContext` holds `Rc<RefCell<RenderContext>>` (not `Arc<GraphicsContext>`).
- `AssetsManager::new(render_ctx: Rc<RefCell<RenderContext>>)`.
- `Application::init()` passes `Rc<RefCell<RenderContext>>` to `AssetsManager::new()`.

**No additional work needed.** `RenderContext` already exposes `device()` / `queue()` accessors that loaders will use.

---

#### ✅ Step 5 — `Handle<T>` + `AssetStorage<T>` generational refactor

**Goal:** Align `Handle` with the target `(index: u32, generation: u32)` design — compact, standard, and usable as batch sort keys.

**What was done:**
- **`Handle<T>`** (`assets/handle.rs`) — now `{ index: u32, generation: u32, _phantom: PhantomData<T> }`. The global monotonic `Counter`-based `id` is gone. `Copy`, `Clone`, `Debug`, `Hash`, `PartialEq`, `Eq` implemented on `(index, generation)`.
- **`AssetStorage<T>`** (`assets/storage.rs`) — generational arena: `Vec<Slot<A>>` where each slot owns its data + `next_empty` link + `generation`. Free-list reuse via `empty_slot`.
  - `insert`: reuse a free slot (keep its generation) or append. Returns `Handle { index, generation }`.
  - `get`/`get_mut`: validate `handle.generation == slot.generation`.
  - `remove`: take data, push index to free list, bump `generations[index]` so stale handles no longer match.
- **`update_slot` uses `unwrap()`** — the index is always valid (comes from `self.empty_slot`).

**Deliverable:** `Handle<T>` is a compact 8-byte generational handle. Batch sort keys can use `Handle` directly. **Resolves L1.**

**Files modified:** `assets/handle.rs`, `assets/storage.rs`

---

#### ✅ Step 6 — Metadata-driven asset system: `Shader` + `Sampler` + `Texture`

**Goal:** The asset system can actually load something — and the load API is built around **asset metadata**, not bare file paths, so refinement parameters and dependencies are first-class.

**Design pivot — metadata-driven loading:**
Instead of `load::<A>(path)`, the API is `load::<A>(metadata: A::Metadata)`. Each asset type declares a `Metadata` associated type that fully describes how to (re)create the asset: its source plus refinement parameters (mip levels, format, sampler config, …) and `Handle`s to dependent assets. The asset **owns its metadata** (accessible via `Asset::metadata()`), making it self-describing — this is the foundation for future serialization, hot-reload, and asset deduplication.

**What was done:**
- **`Asset` trait** (`assets.rs`) — now requires `type Metadata: Any + Send + Sync + Clone + 'static` and `fn metadata(&self) -> &Self::Metadata`. Assets own their metadata.
- **`AssetsManager::load::<A>(metadata)`** — primary load API. Resolves the loader by asset type, passes the metadata through the erased boundary, inserts the resulting asset.
- **`AssetsManager::load_from_file`** — skeleton for file-based loading (serialization deferred). Currently `unimplemented!()`. The future design: the metadata file stores the asset's source plus, for dependencies, **relative paths** to other metadata files; `load_from_file` reads the file, recursively resolves dependencies, converts the file-form metadata into the runtime form (paths → `Handle`s), and delegates to `load`.
- **`AssetLoadersStorage`** (`assets/load.rs`) — indexes loaders by `TypeId::of::<A>()` (one loader per asset type). Extension-based routing and `load_with_hint` removed — the metadata's `source` variant encodes the source type. Re-registering a loader for an asset type overwrites the previous entry.
- **`ErasedLoader`** — `load_erased(Box<dyn Any>)` downcasts to `<A as Asset>::Metadata` at the type-erased boundary.
- **`AssetError`** (`assets/error.rs`) — enriched with payloads, `Display`, `std::error::Error`. Variants: `IoError`, `LoaderNotFound`, `MetadataTypeMismatch`, `DependencyLoadError`, `ImageError`, `LoadingError`. **Resolves L3.**
- **`Shader`** (`graphics/shader.rs`) — `{ module, metadata }`. `ShaderMetadata` with `ShaderSource::{File(PathBuf), Inline(String)}`. `Inline` enables embedded default shaders (Step 9). `ShaderMetadata::from_file` / `from_inline` constructors.
- **`Sampler`** (`graphics/sampler.rs`) — **new asset type**. `{ sampler, metadata }`. `SamplerMetadata` uses engine-native config enums (`AddressMode`, `FilterMode`, `CompareFunction`, `SamplerBorderColor`) kept free of non-serializable `wgpu` types. `SamplerLoader` registered. Shared samplers across textures.
- **`Texture`** (`graphics/texture.rs`) — `{ texture, view, sampler: Handle<Sampler>, metadata }`. No longer creates its own sampler — references a shared `Sampler` asset. `TextureMetadata` with `TextureSource::{File(PathBuf), Raw { data, size: TextureSize }}`. **`TextureSize`** (new) — `{ width, height, depth, tex_dim: wgpu::TextureDimension }` with `new_texture2d` constructor and `Into<wgpu::Extent3d>`. `TextureMetadata::from_file` / `from_raw` constructors.
- **Loaders registered** in `app.rs` `init_assets_manager`: `ShaderLoader`, `SamplerLoader`, `TextureLoader`.
- **`nova-test`** (`nova-test/src/main.rs`) — loads a default `Sampler`, then loads `Shader` and `Texture` via their metadata constructors.

**Dependency form:** Two-struct pattern (decided). Runtime form (with `Handle<Sampler>`) is implemented now; the file form (relative `PathBuf`s to dependency metadata files) is deferred until serialization lands. `load_from_file` will convert between them.

**Serialization:** Deferred — only the skeleton is in place. Field shapes are chosen with serializability in mind (engine-native config enums instead of raw `wgpu` types where they aren't `Clone + Send + Sync`).

**Borrow discipline:** Loaders hold `Rc<RefCell<RenderContext>>`. During `load()`, the loader does `ctx.render_ctx.borrow().device()` to access the GPU. No overlapping `borrow_mut()` is active during loading (it happens outside `begin_frame`).

**Future wins enabled by metadata-driven loading:**
- **Serialization (Step 13+):** serialize `(TypeId, metadata)`; dependency `Handle` → relative `PathBuf` to the dependency's metadata file.
- **Asset deduplication (Step 13+):** `Metadata: Hash + Eq` enables `HashMap<A::Metadata, Handle<A>>`.
- **Hot-reload (Step 13+):** metadata is the asset's identity — reload = load with the same metadata.
- **Default resources (Step 9):** `ShaderSource::Inline` / `TextureSource::Raw` enable embedded/procedural assets.

**Deliverable:** `assets.load::<Shader>(ShaderMetadata::from_file(...))`, `assets.load::<Sampler>(SamplerMetadata::default())`, and `assets.load::<Texture>(TextureMetadata::from_file(..., sampler_handle))` work and return handles. **Resolves H1, H5, L3, L6.**

**Files added:** `graphics/sampler.rs`
**Files modified:** `assets.rs`, `assets/load.rs`, `assets/error.rs`, `graphics/shader.rs`, `graphics/texture.rs`, `graphics.rs`, `app.rs`, `nova-test/src/main.rs`

---

#### ✅ Step 7 — `MaterialTemplate` + `Material` + asset system refinements

**Goal:** The material model that drives pipeline compilation. Templates are assets (shared, metadata-driven); materials are lightweight per-instance objects. Also refined the asset system's borrow model and added shader entry points.

**What was done:**

**Asset system refinements:**
- **`LoadContext<'a>`** now holds `&'a AssetsManager` (immutable ref) + `Rc<RefCell<RenderContext>>`. Loaders can retrieve already-loaded dependencies by handle via `ctx.assets.get_asset(handle)` (e.g. a `MaterialTemplateLoader` resolving its `Shader` dependencies). Loaders do *not* load new assets — dependency path→handle resolution is the caller's job (automated later via serialization/`load_from_file`).
- **`AssetLoader::load` takes `&self`** (was `&mut self`). Loaders are stateless; this keeps `AssetLoadersStorage::get()` immutable. If a loader ever needs internal caching, it should use interior mutability (`RefCell`/`Mutex`).
- **`AssetsManager::load` borrow structure** — two-phase: phase 1–3 borrows `&self` immutably (build `LoadContext`, get loader, run loader), `LoadContext` is dropped, phase 4 borrows `&mut self` to insert. The borrow checker accepts this because the immutable borrow ends before the mutable insert.
- **`AssetsManager` owns `storages` + `loaders` as plain fields** (no `Rc<RefCell>`). Simple, direct ownership. `get_asset`/`get_asset_mut`/`remove_asset` return plain `&A`/`&mut A`/`Option<A>`.
- **`Shader` gained `entry_point`** — `ShaderMetadata` now carries an `entry_point: String` (default `"main"`) with a `with_entry_point` builder. `Shader::entry_point()` accessor. Needed for pipeline creation (Step 8).

**Material types (`graphics/material.rs`):**
- **`MaterialTemplate`** — asset owning `MaterialTemplateMetadata`. Accessors: `vertex_shader()`, `fragment_shader()`, `vertex_buffer_layout()`, `blend_state()`, `depth_stencil()`, `topology()`, `uniform_layout()`. `pipeline_key()` returns an opaque `PipelineKey` (Step 8 will back it with `Handle<MaterialTemplate>`).
- **`MaterialTemplateMetadata`** — uses engine-native enums (not raw `wgpu`) for serializability, consistent with the Step 6 `SamplerMetadata` pattern:
  - `BlendMode` (`None`/`Alpha`/`Additive`) → `Option<wgpu::BlendState>`
  - `DepthStencilConfig` + `DepthFormat` + `DepthCompare` → `wgpu::DepthStencilState` (wgpu 30: `depth_write_enabled: Option<bool>`, `depth_compare: Option<CompareFunction>`)
  - `Topology` → `wgpu::PrimitiveTopology`
  - `ShaderStage` → `wgpu::ShaderStages`
  - `VertexBufferLayout` kept with `wgpu::VertexAttribute` (consistent with `TextureMetadata` keeping `wgpu::TextureFormat` — simple serializable types don't need wrapping)
- **`UniformBinding`** — `{ name, ty: UniformType, binding_slot, visibility: ShaderStage }`. Drives bind group layout creation (Step 8).
- **`UniformType`** — `Mat4`, `Vec4`, `F32` with `size()`.
- **`UniformValue`** — `Mat4(Mat4)`, `Vec4(Vec4)`, `F32(f32)` with `write_bytes(bytes, offset)` using `bytemuck` + `glam`'s column-major (std140) layout.
- **`Material`** — per-instance: `{ template: Handle<MaterialTemplate>, uniforms: HashMap<String, UniformValue>, textures: HashMap<u32, Handle<Texture>> }`. Immutable (no `dirty` flag, no mutators). Builders on `MaterialMetadata`: `with_uniform`, `with_texture`. Load-time validation in `MaterialLoader` (Step 8).
- **`PipelineKey`** — backed by `PipelineCacheKey { material_template_handle, target_format }` (Step 8).
- **`MaterialTemplateLoader`** — trivial: metadata carries resolved `Handle<Shader>`s, loader wraps in `MaterialTemplate::new`. No nested loading at loader level.
- **`MaterialTemplateLoader` registered** in `app.rs` `init_assets_manager` alongside `ShaderLoader`, `SamplerLoader`, `TextureLoader`.

**Dependencies added:** `glam = "0.30"`, `bytemuck = "1.23"` (with `derive` feature).

**Design note — dependency resolution model:** Dependencies are resolved by the *caller* before the loader runs (e.g. caller loads `Shader`s first, passes `Handle<Shader>`s into `MaterialTemplateMetadata`). Loaders retrieve already-loaded deps via `ctx.assets.get_asset(handle)`. Re-entrant loading (loader calling `load()` for new assets) is *not* supported — this keeps the borrow model simple. When serialization lands, `load_from_file` will resolve dependency paths → handles (by loading deps first), then call `load()` with fully-resolved metadata.

**Deliverable:** `MaterialTemplate` + `Material` + `MaterialTemplateLoader` implemented. Materials can be created from templates with per-instance uniforms and textures. `PipelineKey` ready for Step 8. `LoadContext` provides read-only asset access for dependency retrieval.

**Files added:** `graphics/material.rs` (already existed as skeleton, fully written)
**Files modified:** `assets.rs`, `assets/load.rs`, `graphics/shader.rs`, `graphics.rs`, `app.rs`

---

#### ✅ Step 8 — `PipelineCache` + `BindGroupAllocator` + `RenderTarget` refactor

**Goal:** Pipeline compilation is deduplicated; bind groups are allocated efficiently. The rendering pipeline is restructured around `RenderTarget`.

**Why now:** Without pipeline caching, every material recompiles its pipeline on first use. With many materials, this stalls. The cache keys off the template, so materials sharing a template share a pipeline.

**What was done:**

**Uniform system redesign** (`graphics/uniform.rs`):
- **`UniformArena`** — per-frame transient storage for scene-global data (camera, time, lighting). Typed staging entries (`upload(binding_slot, value)`), builds group 0 bind group once per frame via `build_bind_group(device, layout)`, `reset()` at frame start. Scene globals are NOT stored in `Material`.
- **`MaterialUniformPool`** — persistent, batch-built shared `wgpu::Buffer` for all material uniforms. `build(device, materials)` packs all materials' uniform values (in `template.uniform_layout()` order), records per-`Handle<Material>` `(offset, size)` allocations. Caller controls when to (re)build.
- **Uniform types moved to `uniform.rs`** — `UniformType` (Mat4/Vec4/F32 + `size()`), `UniformValue` (typed enum + `write_bytes`), `UniformBinding` (declaration in template metadata). `material.rs` imports them.
- **Removed** the old arena that held all uniforms and returned opaque `UniformId`s (which broke serialization). Removed `UniformId`, `UniformIdGenerator`, `UniformArenaRef`, `UniformBuffer`, `Uniform`, `UniformsBuffer`.

**Immutable materials** (`graphics/material.rs`):
- **`MaterialMetadata.uniforms`** changed from `Vec<UniformId>` to `HashMap<String, UniformValue>` (typed, serializable source of truth). Builders: `with_uniform(name, value)`, `with_texture(binding, handle)`.
- **No `dirty` flag, no mutators.** Materials are immutable once loaded. To change a material, load a new one. This removes the `ensure_bound` invalidation path from the original spec.
- **`MaterialLoader` with validation** — resolves the `MaterialTemplate` via `ctx.assets.get_asset`, validates every uniform (provided + correct type + no extras) and every texture binding (provided + no extras) against the template layout. Failures produce `AssetError::DependencyValidationFailure { asset_name, dependency_name, reason }`.
- **`AssetError::DependencyValidationFailure`** added (`assets/error.rs`) with `Display` impl.

**Bind group allocator** (`graphics/bind.rs`):
- **`BindGroupAllocator`** — owns `MaterialUniformPool` + `HashMap<Handle<Material>, wgpu::BindGroup>` cache. Materials are immutable → bind groups built once, cached forever (perfect cache).
- `build_uniform_pool(device, materials)` — caller calls when material set changes; clears bind group cache.
- `get_or_create(device, ResolvedMaterial, layout)` — per-material bind group, builds from pool buffer (at material's offset) + resolved texture views/samplers.
- Per-uniform buffer entries: each template uniform binding slot gets its own `BindGroupEntry`, computed sequentially from the material's allocation offset.

**Pipeline cache** (`graphics/pipeline.rs`):
- `PipelineCacheKey { material_template_handle, target_format }`. `get_or_compile` compiles on first encounter, cache hits after.
- `Pipeline` holds `wgpu::RenderPipeline` + the material bind group layout (group 1).
- Group 0 layout = `scene_bind_group_layout` (singleton on `RenderContext`). Group 1 layout = derived from template's `uniform_layout()` + `texture_layout()`.
- `textrue_layout()` → `texture_layout()` typo fix in `MaterialTemplate`.

**`RenderTarget` refactor** (`graphics/render_target.rs`, new):
- **`RenderTarget<'a>`** — view-agnostic, borrows `&mut RenderContext` for direct mutable access to `pipeline_cache` and `bind_group_allocator` (no `RefCell`). Owns command encoder + `UniformArena`.
- Methods: `upload_uniform`, `build_scene_bind_group`, `get_or_compile_pipeline`, `build_uniform_pool`, `get_or_create_bind_group`, `begin_render_pass`, `submit`.
- `submit()` finishes encoder + submits to queue (no present — `Frame`'s job).
- `get_or_create_bind_group` takes `ResolvedMaterial` (simplified API).

**`Frame` slimmed** (`graphics/frame.rs`):
- Now just `SurfaceTexture` + `TextureView` + `present(queue)`. No encoder, no arena, no `RenderContext` borrow, no lifetime parameter.

**`RenderContext`** (`graphics/render.rs`):
- Owns `pipeline_cache` + `bind_group_allocator` as `pub(crate)` fields. `scene_bind_group_layout` (binding 0 = camera Mat4 VERTEX, binding 1 = time F32 VERTEX_FRAGMENT). `surface_format()` accessor.
- `begin_frame()` returns `Frame` (no lifetime tied to `RenderContext`).

**`RenderPass`** (`graphics/render_pass.rs`):
- `begin_render_pass` logic moved into `RenderTarget` (handles split-borrow of `self.view` + `self.encoder`). `RenderPass.inner` is `pub(crate)`. Borrows `RenderTarget` mutably.

**`on_render` flow** (`app/handler.rs`):
```
frame = begin_frame()           → Frame (no borrow)
target = RenderTarget::new(&mut render_ctx, frame.view())
proxy.on_render(ctx, &mut target)
target.submit()                 → encoder.finish + queue.submit
frame.present(&queue)           → surface present
```

**`ApplicationProxy::on_render`** signature: `fn on_render(&mut self, ctx: &ApplicationContext, target: &mut RenderTarget<'_>)`.

**Divergence from original spec:**
- `Material::ensure_bound` (dirty-flag-based) replaced by immutable materials + batch uniform pool build + permanent bind group cache. No `dirty` flag needed.
- `RenderTarget::draw_material` convenience method deferred to Step 9 (delivered — begins pass, compiles pipeline, builds bind group, records draw in one split-borrowing call).

**Deliverable:** Pipeline compilation deduplicated by template, per-material bind groups cached, scene globals in a separate arena, materials immutable + validated at load, `RenderTarget` enables view-agnostic rendering with direct cache access.

**Files added:** `graphics/render_target.rs`, `graphics/uniform.rs` (rewritten)
**Files modified:** `graphics/material.rs`, `graphics/bind.rs` (rewritten), `graphics/render.rs`, `graphics/frame.rs`, `graphics/render_pass.rs`, `graphics/pipeline.rs`, `graphics.rs`, `assets/error.rs`, `app.rs`, `app/handler.rs`, `nova-test/src/main.rs`

---

### Progress Summary

| Step | Status | Deliverable |
|------|--------|-------------|
| Step 0 — Immediate fixes | ✅ Done | Surface loss recovery, storage bounds safety |
| Step 1 — `RenderContext` | ✅ Done | Public render hub, present mode config, surface loss handling |
| Step 2 — `Frame` + `RenderPass` | ✅ Done | Per-frame RAII, render pass descriptor, draw methods, `Color` type |
| Step 3 — `on_render` | ✅ Done | **Blue window** — proxy controls rendering via public API |
| Step 4 — `AssetsManager` → `RenderContext` | ✅ Done | Loaders wired to `RenderContext` (done during Step 1) |
| Step 5 — `Handle<T>` generational refactor | ✅ Done | Compact 8-byte generational handle, free-list slot reuse |
| Step 6 — Metadata-driven asset system | ✅ Done | `Shader` + `Sampler` + `Texture` assets + loaders, metadata-driven load API |
| Step 7 — `MaterialTemplate` + `Material` | ✅ Done | Material recipe/instance model, engine-native enums, `LoadContext` borrow refinement, shader entry points |
| Step 8 — `PipelineCache` + `BindGroupAllocator` + `RenderTarget` | ✅ Done | Pipeline caching by template, per-material bind group cache, `UniformArena` (group 0), `MaterialUniformPool`, `RenderTarget` refactor, immutable materials with load-time validation |
| Step 9 — First render test (colored quad) | ✅ Done | **Colored quad on screen** — end-to-end pipeline proven; wgpu leaks wrapped; `draw_material` convenience; uniform offset alignment; per-uniform pool allocations; optional fragment shader |
| Step 10 — `DrawBatch` + `submit_batches` | ✅ Done | Pure-data `DrawBatch` (u16 indices, `new_with_vertices`); `submit_batches` (consumes commander, no sort, auto-builds pool+scene); `BufferLayout` + `VertexBufferLayout`/`InstanceBufferLayout` constructors; batched command submission |

**Critical issues resolved:** C1 (no `on_render`), C2 (no `RenderContext`), C3 (no `Frame`/`RenderPass`), C4 (surface loss), C5 (present mode), H1 (no loaders), H2 (loaders use `GraphicsContext`), H3 (`ApplicationContext` leaks GPU), H4 (handler reaches into `GraphicsContext`), H5 (loader registration hook), L1 (`Handle` non-generational), L2 (`AssetStorage` panics), L3 (`AssetError` no context), L6 (missing deps).

---

## Part B — Remaining Migration Walkthrough

Each step is self-contained and leaves the codebase in a working state.

### ✅ Step 9 — First render test (colored quad)

**Goal:** A colored quad on screen through the full public API. **This is the major end-to-end milestone.**

**Why now:** Proves the entire pipeline works: assets → materials → pipelines → `RenderTarget` → render pass → draw → submit → present.

**Scope decision:** The original plan included engine-level default resources (`defaults/shader_2d_flat.wgsl`, a default white 1×1 texture, a default `MaterialTemplate` baked into `nova-core`). This was **deferred** — the end-to-end pipeline is proven with a test-specific shader in `nova-test/assets/shader.wgsl` instead. Engine-level defaults will be added when `nova-2d` needs them (Step 11).

**What was done:**
1. **`math.rs`** — re-exports `glam` types (`Mat4`, `Vec4`, etc.) so engine users never depend on `glam` directly.
2. **`mem.rs`** — re-exports `bytemuck` (`Pod`, `Zeroable`, `cast_slice`, `bytes_of`).
3. **wgpu leak wrapping** — replaced all raw `wgpu` types in `nova-core`'s public API with engine-native enums + `From`/`TryInto` impls:
   - `buffer.rs`: `VertexFormat` enum; `VertexBufferLayout` now takes `&[VertexFormat]`; `TryInto<wgpu::VertexBufferLayout>` returns `Err(())` when empty (omits the buffer from the pipeline).
   - `texture.rs`: `TextureDimension`, `TextureFormat`, `TextureUsages` (bitflags), `TextureViewDimension`, `SamplerBindingType`.
   - `render_pass.rs`: `set_pipeline` takes `&Pipeline` (not `&wgpu::RenderPipeline`); `IndexFormat` enum.
4. **`RenderTarget::draw_material`** — a convenience that begins a render pass, compiles (or fetches) the pipeline, builds (or fetches) the material bind group, and records the draw — all in one call. It split-borrows `RenderTarget`'s disjoint fields (`render_ctx`, `encoder`, `view`) so the pipeline ref, bind group ref, and render pass coexist without cloning `wgpu` handles.
5. **Uniform buffer offset alignment** — `UniformArena` and `MaterialUniformPool` now align each entry/binding's offset to `device.limits().min_uniform_buffer_offset_alignment` (via Rust's built-in `u64::next_multiple_of`). Fixes the "Buffer offset 64 does not respect … limit 256" validation error.
6. **Per-uniform allocations** — `MaterialUniformPool` allocations are keyed by `(Handle<Material>, binding_slot)`, not per-material. The pool owns all offset math and exposes `binding_resource(handle, slot) -> wgpu::BindingResource` so `bind.rs` never computes offsets.
7. **Optional fragment shader** — `MaterialTemplateMetadata.fragment_shader` changed from `Handle<Shader>` to `Option<Handle<Shader>>`, enabling vertex-only pipelines. `ResolvedMaterialTemplate.fragment_shader` is now `Option<FragmentShader<'a>>`. `pipeline.rs` handles `None` fragment state.
8. **Bug fix: `FragmentShader::try_from`** — was matching the vertex entry point and rejecting the fragment variant (copy-paste from `VertexShader`). Fixed to match `Fragment`/`Both.fs_entry_point`.
9. **`nova-test` renders a colored quad** — shader generates 6 vertices from `vertex_index` (no vertex buffer), color comes from a material uniform (group 1, binding 0). `on_init` loads shader → `MaterialTemplate` → `Material`; `on_render` uploads scene uniforms, builds the uniform pool, resolves the material, and calls `draw_material`.

**Deliverable:** A colored quad on screen, rendered entirely through the public API. Proves the architecture end-to-end.

**Dependencies:** Steps 6–8.

**Files added:** `math.rs`, `mem.rs`
**Files modified:** `graphics/buffer.rs`, `graphics/texture.rs`, `graphics/render_pass.rs`, `graphics/render_target.rs`, `graphics/uniform.rs`, `graphics/pipeline.rs`, `graphics/bind.rs`, `graphics/material.rs`, `graphics/shader.rs`, `assets/resolve.rs`, `Cargo.toml` (added `bitflags`)
**Files modified (nova-test):** `src/main.rs`, `assets/shader.wgsl`

---

### ✅ Step 10 — `DrawBatch` + `submit_batches`

**Goal:** The dimension-agnostic submission contract is ready for batchers.

**What was done:**
1. **`DrawBatch`** (`graphics/draw_batch.rs`) — pure data struct: `Handle<Material>` (unresolved) + raw `Vec<u8>` vertex/index/instance data. Indices are always `u16` (Uint16). No GPU resources — cheap to clone, collect, sort. The commander uploads to GPU buffers at submit time.
2. **`RenderTargetCommander::submit_batches`** — submits multiple batches in one render pass:
   - **Consumes the commander** (`mut self`) — the commander is a one-shot scope: create, submit, done.
   - **No sorting or grouping** — batches are drawn in the exact order given. The caller is responsible for ordering (layer sorting, template grouping for pipeline reuse).
   - Each batch gets its own vertex/index buffer upload and `draw_indexed` call.
   - **Auto-builds the scene bind group** and the **uniform pool from batches** internally — the caller no longer manually builds the pool or scene bind group.
   - One render pass (clear config set once). Destructures `self` to split-borrow all fields (encoder, pipeline_cache, bind_group_allocator, device) so they coexist.
   - Takes `&AssetsManager` for resolving `Handle<Material>` → pipeline + bind group at draw time.
   - Helper methods: `create_buffers`, `set_pipeline`, `draw_call` — extracted for clarity.
3. **`BufferLayout`** (renamed from `VertexBufferLayout`) — now carries `BufferStepMode` (Vertex vs Instance) and a `location_offset` for multi-buffer vertex layouts. `stride()` and `step_mode()` accessors.
   - **`VertexBufferLayout`** and **`InstanceBufferLayout`** are zero-sized constructor types that produce `BufferLayout` with the right step mode + location offset.
4. **`DrawBatch`** — indices stored as `Vec<u16>` (not raw bytes). `new_with_vertices<V: NoUninit>` ergonomic constructor using bytemuck. `add_vertices` for incremental building. `new(material)` creates an empty batch for incremental use.
5. **Batched command submission** — `RenderContext` now accumulates finished `CommandBuffer`s in a `Vec` (`submit_command_encoder`), and `submit_commands` submits them all in one `queue.submit` call. The handler calls `submit_commands()` after the proxy finishes. `RenderTarget::submit(&mut self)` pushes its encoder into the batch (encoder is now `Option`).
6. **`RenderPass::new(encoder, view, desc)`** — constructor that takes the split-borrowed encoder + view directly, avoiding the `&mut self` borrow conflict.
7. **`nova-test` updated** — shader uses a vertex buffer (`@location(0) position: vec2<f32>`), 4 vertices + 6 u16 indices. `on_render` builds a `DrawBatch` via `new_with_vertices` and calls `submit_batches`.

**Deliverable:** `submit_batches()` works. The contract for dimension-specific batchers is ready. Quad still renders.

**Dependencies:** Steps 7–9 (materials + pipeline cache + first render).

**Files added:** `graphics/draw_batch.rs`
**Files modified:** `graphics/render_target.rs`, `graphics/buffer.rs`, `graphics/render_pass.rs`, `graphics/render.rs`, `assets/handle.rs`, `graphics.rs`, `app/handler.rs`
**Files modified (nova-test):** `src/main.rs`, `assets/shader.wgsl`

---

### Step 11 — Split `nova-2d` crate

**Goal:** The first dimension crate, with sprite batching. Textured sprites on screen with a 2D camera.

**Why now:** With the core pipeline proven (Step 9) and the batch contract defined (Step 10), the 2D layer is the first real consumer of the architecture.

**Tasks:**
1. Create `nova-2d/` crate depending on `nova-core`.
2. Define `Vertex2D` (position, UV, color) with `VertexBufferLayout` const.
3. Define `QuadCmd`, `BatchKey2D { template, texture }`.
4. Implement `Batcher2D`: collect, sort by `(layer, BatchKey2D, z)`, flush → `Frame::submit_draw_batch(DrawBatch)`.
5. Implement `Render2D<'a>`: frame-scoped borrower, `draw_quad(cmd)`, `draw_sprite(texture, transform)`. On `Drop`, flush batcher.
6. Implement `SpriteBatch`: dynamic vertex buffer builder for quads.
7. Implement `Camera2D`: orthographic projection → `target.upload_uniform(binding, value)`.
8. Write `shader_2d_textured.wgsl`, create default 2D textured material template.
9. Re-export core types from `nova-2d/src/lib.rs`.
10. In `nova-test`, load a texture, create sprites, render with `Render2D` + `Camera2D`.

**Deliverable:** Textured sprites on screen with a 2D camera. The first real `nova-2d` component.

**Dependencies:** Step 10 (`DrawBatch`). Step 5 (`Handle` as sort key). `glam` for math.

---

### Step 12 — Split `nova-3d` crate

**Goal:** 3D rendering following the same pattern. Meshes on screen with camera and lighting.

**Why now:** With 2D proving the batcher pattern, 3D follows the same structure with depth sorting and instancing.

**Tasks:**
1. Create `nova-3d/` crate depending on `nova-core`.
2. Define `Vertex3D` (position, normal, UV), `MeshCmd`, `BatchKey3D { template, mesh, material }`.
3. Implement `Batcher3D`: collect, sort (material then back-to-front for transparency), flush. Instancing for repeated meshes (`draw_instanced(mesh, material, instances)`).
4. Implement `Render3D<'a>`: `draw_mesh(mesh, material, transform)`, `draw_instanced(mesh, material, instances)`.
5. Implement `MeshRenderer`: draws `Mesh` assets.
6. Implement `Camera3D`: perspective projection → uniform bytes.
7. Implement `LightSystem`: directional/point/spot lights → uniform bind group.
8. Implement `SceneUniforms`: camera + lighting, uploaded once per frame via `UniformArena`.
9. **Implement the depth texture pool** (deferred from Step 2/C6): auto-resizes with surface; `with_depth_clear(1.0)` grabs a depth texture from the pool. Wire it into `RenderPass::new` replacing the placeholder.
10. Write `shader_3d_unlit.wgsl` and `shader_3d_pbr.wgsl`, create default 3D material templates.
11. Re-export core types from `nova-3d/src/lib.rs`.
12. In `nova-test`, load a mesh, render with `Render3D` + `Camera3D` + `LightSystem`.

**Deliverable:** 3D meshes on screen with camera and lighting. The `nova-3d` crate is complete. Brings back the deferred depth pool from Step 2.

**Dependencies:** Step 10 (`DrawBatch`). Step 8 (`UniformArena` on `RenderTarget`). `glam` for math.

---

### Step 13+ — Polish & future

- **Hot-reload:** `reload(handle, new_source)` — propagate through template handles.
- **Optional `nova` umbrella crate:** facade with `2d`/`3d` feature flags.
- **Async loading:** `LoadingHandle<T>`, deferred resolution (two-phase load).
- **ECS integration:** optional backend (feature flag).
- **Culling:** frustum culling in `nova-3d`.
- **Asset deduplication:** `Metadata: Hash + Eq`-based `HashMap<A::Metadata, Handle<A>>`, when memory waste is measured. (Metadata-driven design from Step 6 makes this a natural extension.)
- **`frame_index: u64`** on `RenderTarget` — for double-buffering schemes (cheap to add anytime).

---

## Part C — Remaining Issues Tracker

| Issue | Priority | Status | Resolved in | Notes |
|-------|----------|--------|-------------|-------|
| C1 — No `on_render` | 🔴 Critical | ✅ Resolved | Step 3 | `ApplicationProxy::on_render(ctx, frame)` |
| C2 — No `RenderContext` | 🔴 Critical | ✅ Resolved | Step 1 | `graphics/render.rs` |
| C3 — No `Frame`/`RenderPass` | 🔴 Critical | ✅ Resolved | Step 2 | `graphics/frame.rs`, `render_pass.rs` |
| C4 — Surface loss panics | 🔴 Critical | ✅ Resolved | Step 0/1 | `begin_frame` handles all cases |
| C5 — Present mode uncontrolled | 🔴 Critical | ✅ Resolved | Step 1 | `GraphicsConfiguration` |
| C6 — No depth buffer | 🔴 Critical | ⏳ Deferred | Step 12 | Needed for 3D; placeholder in `RenderPass` |
| H1 — No loaders/asset types | 🟡 High | ✅ Resolved | Step 6 | `ShaderLoader`, `SamplerLoader`, `TextureLoader` (metadata-driven) |
| H2 — `LoadContext` uses `GraphicsContext` | 🟡 High | ✅ Resolved | Step 1/4 | Now `Rc<RefCell<RenderContext>>` |
| H3 — `ApplicationContext` leaks GPU | 🟡 High | ✅ Resolved | Step 3 | Holds `Rc<RefCell<RenderContext>>` |
| H4 — `handler.rs` reaches into `GraphicsContext` | 🟡 High | ✅ Resolved | Step 3 | `render()` deleted; goes through `Frame` |
| H5 — No loader registration hook for 2d/3d | 🟡 High | ✅ Resolved | Step 6 | Loaders registered in `app.rs` `init_assets_manager` |
| L7 — `RenderPass::new` is dead code | 🟢 Low | ✅ Resolved | Step 9 | `RenderPass::new` removed; render pass is begun via `RenderTarget::begin_render_pass` / `draw_material`. |
| L8 — `ResolvedTextureBinding` unused | 🟢 Low | ⏳ Anytime | — | `bind.rs` defines `ResolvedTextureBinding` but `get_or_create` uses `ResolvedMaterial` directly. Remove the dead struct. |
| L9 — `nova-test` unused imports | 🟢 Low | ✅ Resolved | Step 9 | `nova-test` now uses `Shader`, `MaterialTemplate`, `Material`, `UniformBinding`, `UniformValue`, `Vec4`, etc. — all imports are used. |
| L1 — `Handle` non-generational design | 🟢 Low | ✅ Resolved | Step 5 | Refactored to `(index: u32, generation: u32)` |
| L2 — `AssetStorage` panics on bad index | 🟢 Low | ✅ Resolved | Step 0 | `get_mut`/`remove` use `?` |
| L3 — `AssetError` no context | 🟢 Low | ✅ Resolved | Step 6 | Payloads + `Display` + `std::error::Error` |
| L5 — `run_app().unwrap()` | 🟢 Low | ⏳ Anytime | — | Store error into `engine_error` |
| L6 — Missing deps (`glam`, `bytemuck`, `image`) | 🟢 Low | ✅ Resolved | Step 6/7 | `image` added in Step 6; `glam` + `bytemuck` added in Step 7 |

---

## Roadmap at a Glance

```
✅ Step 0  — Immediate fixes (surface loss, storage bounds)
✅ Step 1  — RenderContext (public render hub, present mode)
✅ Step 2  — Frame + RenderPass (RAII frame, render pass descriptor, Color)
✅ Step 3  — on_render (BLUE WINDOW — proxy controls rendering)  ← FIRST VISIBLE MILESTONE
✅ Step 4  — AssetsManager → RenderContext (done during Step 1)
✅ Step 5  — Handle<T> generational refactor
✅ Step 6  — Metadata-driven asset system (Shader + Sampler + Texture)  ← asset system functional
✅ Step 7  — MaterialTemplate + Material + asset system refinements

✅ Step 8  — PipelineCache + BindGroupAllocator + RenderTarget refactor
✅ Step 9  — First render test (colored quad)                  ← END-TO-END MILESTONE
✅ Step 10 — DrawBatch + submit_batches (batcher contract)
⬜ Step 11 — nova-2d crate (sprite batching, Camera2D)
⬜ Step 12 — nova-3d crate (meshes, Camera3D, lights, depth pool)
⬜ Step 13+— Polish (hot-reload, umbrella crate, async, ECS, culling, serialization, dedup)
```

**Next up:** Step 11 (`nova-2d` crate) — sprite batching, `Camera2D`, `Render2D`, textured sprites on screen.