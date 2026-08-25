# Nova Engine — Code Review & Migration Walkthrough

> **Companion to:** `docs/architecture.md` (the target architecture).
> **Purpose:** Track progress against the target architecture, identify remaining work, and lay out a step-by-step migration path.
> **Last updated:** 2026-08-25

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
- **`Material`** — per-instance: `{ template: Handle<MaterialTemplate>, uniforms: HashMap<String, UniformValue>, textures: HashMap<u32, Handle<Texture>>, dirty: bool }`. Methods: `new`, `set_uniform`, `set_texture`, `is_dirty`/`clear_dirty`. The `dirty` flag drives Step 8's `Material::ensure_bound`.
- **`PipelineKey`** — opaque type (`_private: ()`); placeholder. Step 8 will store `Handle<MaterialTemplate>` inside and use `(PipelineKey, TextureFormat)` as the `PipelineCache` key.
- **`MaterialTemplateLoader`** — trivial: metadata carries resolved `Handle<Shader>`s, loader wraps in `MaterialTemplate::new`. No nested loading at loader level.
- **`MaterialTemplateLoader` registered** in `app.rs` `init_assets_manager` alongside `ShaderLoader`, `SamplerLoader`, `TextureLoader`.

**Dependencies added:** `glam = "0.30"`, `bytemuck = "1.23"` (with `derive` feature).

**Design note — dependency resolution model:** Dependencies are resolved by the *caller* before the loader runs (e.g. caller loads `Shader`s first, passes `Handle<Shader>`s into `MaterialTemplateMetadata`). Loaders retrieve already-loaded deps via `ctx.assets.get_asset(handle)`. Re-entrant loading (loader calling `load()` for new assets) is *not* supported — this keeps the borrow model simple. When serialization lands, `load_from_file` will resolve dependency paths → handles (by loading deps first), then call `load()` with fully-resolved metadata.

**Deliverable:** `MaterialTemplate` + `Material` + `MaterialTemplateLoader` implemented. Materials can be created from templates with per-instance uniforms and textures. `PipelineKey` ready for Step 8. `LoadContext` provides read-only asset access for dependency retrieval.

**Files added:** `graphics/material.rs` (already existed as skeleton, fully written)
**Files modified:** `assets.rs`, `assets/load.rs`, `graphics/shader.rs`, `graphics.rs`, `app.rs`

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

**Critical issues resolved:** C1 (no `on_render`), C2 (no `RenderContext`), C3 (no `Frame`/`RenderPass`), C4 (surface loss), C5 (present mode), H1 (no loaders), H2 (loaders use `GraphicsContext`), H3 (`ApplicationContext` leaks GPU), H4 (handler reaches into `GraphicsContext`), H5 (loader registration hook), L1 (`Handle` non-generational), L2 (`AssetStorage` panics), L3 (`AssetError` no context), L6 (missing deps).

---

## Part B — Remaining Migration Walkthrough

Each step is self-contained and leaves the codebase in a working state.

### Step 8 — `PipelineCache` + `BindGroupAllocator` in `RenderContext`

**Goal:** Pipeline compilation is deduplicated; bind groups are allocated efficiently.

**Why now:** Without pipeline caching, every material recompiles its pipeline on first use. With many materials, this stalls. The cache keys off the template, so materials sharing a template share a pipeline.

**Tasks:**
1. Implement `PipelineCache`: `HashMap<(PipelineKey, TextureFormat), wgpu::RenderPipeline>`. `PipelineKey` is produced by `MaterialTemplate::pipeline_key()` (Step 7; currently opaque — back it with `Handle<MaterialTemplate>`). `get_or_compile(key, format, device)` resolves the template handle to fetch shaders (via `AssetsManager::get_asset`), reads `entry_point()`/`vertex_buffer_layout()`/`blend_state()`/`depth_stencil()`/`topology()`, and compiles the pipeline.
2. Implement `BindGroupAllocator`: creates `wgpu::BindGroup` from material data + the template's `uniform_layout()` (engine-native enums → `wgpu` via `Into` impls). Pool descriptor sets to avoid churn.
3. Add both to `RenderContext`.
4. Implement `Material::ensure_bound(device, queue)` — pack uniforms via `UniformValue::write_bytes`, update uniform buffer + rebuild bind groups if `is_dirty()`, then `clear_dirty()`.
5. Implement `RenderPass::draw_material(&material, ...)` — convenience: bind pipeline + bind groups + draw.

**Deliverable:** Materials compile pipelines (cached by template) and bind groups on first use; subsequent uses are cache hits.

**Dependencies:** Step 7 (`MaterialTemplate` + `Material` + `PipelineKey` + engine-native enums + `UniformValue::write_bytes`).

---

### Step 9 — Default resources + first render test (colored quad)

**Goal:** A colored quad on screen through the full public API. **This is the major end-to-end milestone.**

**Why now:** Proves the entire pipeline works: assets → materials → pipelines → frame → render pass → draw → submit → present.

**Tasks:**
1. Write `nova-core/src/graphics/defaults/shader_2d_flat.wgsl` (vertex: position + color + ortho projection uniform; fragment: output color). Embedded via `include_str!`.
2. Register default shaders via `ShaderMetadata::from_inline(...)` + `AssetsManager::load::<Shader>` (the `ShaderSource::Inline` variant already supports this).
3. Create a default `MaterialTemplate` (2D flat) + default white 1×1 `Texture` via `TextureMetadata::from_raw(...)` (the `TextureSource::Raw` variant already supports this).
4. Add `UniformArena` to `Frame` (minimal: per-frame staging buffer for the camera projection uniform).
5. In `nova-test`, create a `Material` from the default template, set `u_color`, define quad vertices, and render in `on_render` using `RenderPass::draw_material` (or direct `set_pipeline` + `set_bind_group` + `draw`).

**Deliverable:** A colored quad on screen, rendered entirely through the public API. Proves the architecture end-to-end.

**Dependencies:** Steps 6–8. Brings back the deferred `UniformArena` from Step 2.

---

### Step 10 — `DrawBatch` + `submit_draw_batch`

**Goal:** The dimension-agnostic submission contract is ready for batchers.

**Tasks:**
1. Define `DrawBatch` in `nova-core`: `{ template_key, material, bind_groups, vertex_buffer, vertex_count, instance_count, uniform_data, render_pass_descriptor }`.
2. Implement `Frame::submit_draw_batch(batch)` — pipeline lookup, bind group creation, uniform upload, command recording.
3. This is the seam where `nova-2d`/`nova-3d` batchers will plug in.

**Deliverable:** `Frame::submit_draw_batch()` works. The contract for dimension-specific batchers is ready.

**Dependencies:** Steps 7–8 (materials + pipeline cache).

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
7. Implement `Camera2D`: orthographic projection → `frame.upload_uniform(bytes)`.
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

**Dependencies:** Step 10 (`DrawBatch`). Step 9 (`UniformArena`). `glam` for math.

---

### Step 13+ — Polish & future

- **Hot-reload:** `reload(handle, new_source)` — propagate through template handles.
- **Optional `nova` umbrella crate:** facade with `2d`/`3d` feature flags.
- **Async loading:** `LoadingHandle<T>`, deferred resolution (two-phase load).
- **ECS integration:** optional backend (feature flag).
- **Culling:** frustum culling in `nova-3d`.
- **Asset deduplication:** `Metadata: Hash + Eq`-based `HashMap<A::Metadata, Handle<A>>`, when memory waste is measured. (Metadata-driven design from Step 6 makes this a natural extension.)
- **`frame_index: u64`** on `Frame` — for double-buffering schemes (cheap to add anytime).

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

⬜ Step 8  — PipelineCache + BindGroupAllocator
⬜ Step 9  — Default resources + colored quad                 ← END-TO-END MILESTONE
⬜ Step 10 — DrawBatch + submit_draw_batch (batcher contract)
⬜ Step 11 — nova-2d crate (sprite batching, Camera2D)
⬜ Step 12 — nova-3d crate (meshes, Camera3D, lights, depth pool)
⬜ Step 13+— Polish (hot-reload, umbrella crate, async, ECS, culling, serialization, dedup)
```

**Next up:** Step 8 (`PipelineCache` + `BindGroupAllocator`) — pipeline compilation deduplicated by `MaterialTemplate`, bind groups built from `UniformValue` data. Builds on Step 7's `PipelineKey`, `MaterialTemplate` accessors, and engine-native enums.