# Nova Engine — Code Review & Migration Walkthrough

> **Companion to:** `docs/architecture.md` (the target architecture).
> **Purpose:** Analyze the current codebase against the target architecture, identify critical changes, and lay out a step-by-step migration path.
> **Last updated:** 2026-08-23

---

## Part A — Critical Analysis of Current Code

### A.1 Current State Summary

The codebase is an early-stage engine with a working event loop, window, and GPU context, but **no rendering API and no usable asset system**. Here's what exists and its maturity:

| Component | File | Status | Maturity |
|-----------|------|--------|----------|
| `Application<P>` — event loop, fixed-timestep update | `app.rs` | ✅ Working | Solid |
| `ApplicationBuilder<P>` — fluent setup | `app/builder.rs` | ✅ Working | Solid |
| `ApplicationHandler` impl — winit wiring | `app/handler.rs` | ✅ Working | Solid (but render is a stub) |
| `WindowApi` — `Arc<Window>` wrapper | `window.rs` | ✅ Working | Minimal but fine |
| `GraphicsContext` — wgpu Instance/Adapter/Device/Queue/Surface | `graphics/context.rs` | ✅ Working | Solid |
| `Clock` — frame timing | `time.rs` | ✅ Working | Solid |
| `EngineError` / `EngineResult` | `errors.rs` | ✅ Working | Minimal |
| `AssetsManager` — TypeMap storage + loader registry | `assets.rs` | ⚠️ Partial | Structurally sound, but **non-functional** (see A.3) |
| `AssetStorage<T>` — generational arena | `assets/storage.rs` | ⚠️ Partial | Works, but `Handle` design is suboptimal (see A.4) |
| `Handle<T>` — typed reference | `assets/handle.rs` | ⚠️ Partial | Works, but `id` scheme is unusual (see A.4) |
| `AssetLoader` / `ErasedLoader` / `AssetLoadersStorage` | `assets/load.rs` | ⚠️ Partial | Structurally sound, but **no loaders registered** (see A.3) |
| `AssetError` | `assets/error.rs` | ⚠️ Partial | Minimal, no messages |
| `render()` free function (hardcoded clear) | `app/handler.rs` | 🔴 Stub | Must be replaced |
| `on_render` on `ApplicationProxy` | `app.rs` | 🔴 Missing | Must be added |
| `RenderContext` | — | 🔴 Missing | Must be created |
| `Frame` / `RenderPass` | — | 🔴 Missing | Must be created |
| `MaterialTemplate` / `Material` | — | 🔴 Missing | Must be created |
| `Shader` / `Texture` / `Sampler` / `Mesh` asset types | — | 🔴 Missing | Must be created |
| Any concrete `AssetLoader` impl | — | 🔴 Missing | Must be created |
| `glam` / `bytemuck` / `image` deps | — | 🔴 Missing | Must be added |

### A.2 Critical Issues (Must Fix)

These are the highest-priority problems — they block the entire rendering pipeline.

#### 🔴 C1. No `on_render` — the proxy cannot render anything

**Where:** `app.rs` (`ApplicationProxy` trait), `app/handler.rs` (`render()` free function).

**Problem:** `ApplicationProxy` only has `on_update`. The `render()` function in `handler.rs` is a hardcoded free function that clears the screen to a fixed color — the user has zero control over rendering. This is the single biggest gap between the current code and the target architecture.

**Fix:** Add `on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame)` to `ApplicationProxy`. Replace the `render()` free function with `begin_frame` → `on_render` → `Frame::drop`. This requires `RenderContext` and `Frame` to exist first.

#### 🔴 C2. No `RenderContext` — no public render hub

**Where:** Missing entirely.

**Problem:** `GraphicsContext` is `pub(crate)` and held directly in `ApplicationContext` as `Arc<GraphicsContext>`. There is no public render API layer. The jump from "raw GPU access" to "high-level rendering" is a cliff — nothing structurally enforces that the high-level renderer avoids touching `wgpu` directly.

**Fix:** Create `RenderContext` wrapping `GraphicsContext` behind `Mutex`. Add `device()`/`queue()`/`surface_format()` accessors, `begin_frame()`, surface management. Move `GraphicsContext` access out of `ApplicationContext` — replace `gfx: Arc<GraphicsContext>` with `render: Arc<RenderContext>`.

#### 🔴 C3. No `Frame` / `RenderPass` — no per-frame abstraction

**Where:** Missing entirely.

**Problem:** The current `render()` function manually acquires the surface texture, creates a view, creates an encoder, begins a render pass, and submits — all inline in `handler.rs`. There's no reusable `Frame` object, no `RenderPass` scope, no uniform arena. Every render path would have to duplicate this boilerplate.

**Fix:** Create `Frame` (surface texture + encoder + uniform arena + frame index) with `begin_pass()` → `RenderPass` and `Drop` impl that submits + presents. Created by `RenderContext::begin_frame()`.

#### 🔴 C4. Surface loss panics

**Where:** `app/handler.rs`, line in `render()`: `wgpu::CurrentSurfaceTexture::Lost => panic!("surface lost")`.

**Problem:** A lost surface is a recoverable condition (reconfigure and skip the frame), but the current code panics, crashing the application.

**Fix:** Replace `panic!` with `ctx.render.reconfigure_surface(); return;` (skip this frame). Move this logic into `RenderContext::begin_frame()` so it's centralized.

#### 🔴 C5. Present mode is uncontrolled

**Where:** `graphics/context.rs`, `create_surface_config`: `present_mode: surface_caps.present_modes[0]`.

**Problem:** The first available present mode is chosen blindly. This may be `AutoVsync`, `Immediate` (no vsync → tearing), or anything. The user has no control, and the behavior is inconsistent across platforms.

**Fix:** Make present mode configurable via `ApplicationBuilder` (e.g., `.with_present_mode(PresentMode::AutoVsync)`). Default to `AutoVsync` for predictable vsync. Move surface config into `RenderContext`.

#### 🔴 C6. No depth buffer management

**Where:** `graphics/context.rs`, `app/handler.rs` (`render()` uses `depth_stencil_attachment: None`).

**Problem:** 3D rendering requires a depth buffer. The current code has no depth texture creation, no depth attachment in the render pass, and no depth format tracking. 3D is impossible without this.

**Fix:** Add a depth texture pool to `RenderContext` (auto-resizes with surface). `Frame` tracks `depth_format: Option<TextureFormat>`. `begin_pass(depth: bool)` grabs a depth texture from the pool when needed.

### A.3 High-Priority Issues (Should Fix Soon)

#### 🟡 H1. Asset system is non-functional — no loaders, no asset types

**Where:** `assets.rs`, `assets/load.rs`, `assets/storage.rs`.

**Problem:** The `AssetsManager` struct exists and the loader dispatch infrastructure is sound, but:
- **No concrete `AssetLoader` implementations exist.** `register_loader` is never called. `load()` will always return `LoaderNotFound` or `UnsupportedExtension`.
- **No concrete asset types exist.** There's no `Shader`, `Texture`, `Sampler`, or `Mesh` struct implementing `Asset`. The `Asset` trait is `pub trait Asset: 'static {}` — minimal, but nothing implements it.
- **`AssetsManager::new()` registers no default loaders.** Even if asset types existed, nothing could load them.

**Fix:** Define `Shader`, `Texture`, `Sampler`, `Mesh` structs implementing `Asset`. Implement `ShaderLoader` (`.wgsl` → `wgpu::ShaderModule`) and `TextureLoader` (`.png`/`.jpg` → `wgpu::Texture` via `image` crate). Register them in `AssetsManager::new()` or at `RenderContext` init.

#### 🟡 H2. `LoadContext` uses `Arc<GraphicsContext>` directly, not `Arc<RenderContext>`

**Where:** `assets/load.rs` (`LoadContext { gfx: Arc<GraphicsContext> }`), `assets.rs` (`AssetsManager::new(gfx: Arc<GraphicsContext>)`).

**Problem:** The target architecture has loaders receive `Arc<RenderContext>` (the public hub), not `Arc<GraphicsContext>` (the raw, `pub(crate)` layer). Currently `GraphicsContext` is `pub(crate)`, but it's passed to `AssetsManager` and `LoadContext` as `Arc<GraphicsContext>` — this works only because they're in the same crate. Once `nova-2d`/`nova-3d` want to register loaders, they can't access `GraphicsContext`.

**Fix:** Change `LoadContext` to hold `Arc<RenderContext>`. Change `AssetsManager::new()` to take `Arc<RenderContext>`. Loaders access GPU via `ctx.render.device()` / `ctx.render.queue()`.

#### 🟡 H3. `ApplicationContext` exposes `gfx: Arc<GraphicsContext>` — leaks raw GPU access

**Where:** `app.rs` (`ApplicationContext { window_api, gfx, assets_manager }`).

**Problem:** `ApplicationContext` holds `Arc<GraphicsContext>` directly. While `GraphicsContext` fields are `pub(crate)`, the struct itself is accessible. The target architecture says the application layer should only see `RenderContext` + `AssetManager` + `WindowApi` — never raw GPU.

**Fix:** Replace `gfx: Arc<GraphicsContext>` with `render: Arc<RenderContext>`. The application (and `nova-2d`/`nova-3d`) access GPU through `RenderContext`'s public accessors only.

#### 🟡 H4. `GraphicsContext` fields are `pub(crate)` but accessed directly in `handler.rs`

**Where:** `app/handler.rs` (`render()` uses `gfx.surface.get_current_texture()`, `gfx.device.create_command_encoder(...)`, `gfx.queue.submit(...)`).

**Problem:** The `render()` free function reaches directly into `GraphicsContext`'s `pub(crate)` fields. This is the "convention, not constraint" problem — the target architecture wants `RenderContext` to be the only layer touching `GraphicsContext`.

**Fix:** This is resolved by C2/C3 — once `RenderContext` and `Frame` exist, `render()` is deleted and all GPU access goes through them.

#### 🟡 H5. `AssetsManager` is `pub` but `new()` is `pub(crate)` — can't be constructed outside core

**Where:** `assets.rs` (`pub struct AssetsManager`, `pub(crate) fn new(...)`).

**Problem:** This is actually correct for the target architecture (the engine constructs `AssetsManager` during `Application::init()`), but it means `nova-2d`/`nova-3d` can't add their own default loaders at init. The engine needs a hook for dimension crates to register loaders.

**Fix:** Either (a) expose a `register_default_loaders` method on `AssetsManager` that `nova-2d`/`nova-3d` call during their setup, or (b) have `Application::init()` call a function in `nova-2d`/`nova-3d` (behind a feature flag) that registers loaders. For now, register built-in loaders (`ShaderLoader`, `TextureLoader`) in `nova-core` during init.

### A.4 Lower-Priority Issues (Can Defer)

#### 🟢 L1. `Handle<T>` uses a `u64 id` instead of `(index, generation)` pair

**Where:** `assets/handle.rs` (`Handle { id: u64, index: usize, _phantom }`).

**Problem:** The current `Handle` packs identity into a monotonically-increasing `u64 id` (from `Counter`), plus a separate `index`. This works (stale handles mismatch on `id`), but:
- The `id` is allocated globally per-storage, not per-slot-generation. A reused slot gets a new `id`, but the `id` counter never resets — it's an ever-growing integer, not a generational index.
- `index` is `usize` (8 bytes on 64-bit) — wasteful; `u32` suffices.
- The target architecture specifies `Handle { index: u32, generation: u32 }` — a classic generational handle that wraps and is compact.

**Fix:** Refactor `Handle<T>` to `{ index: u32, generation: u32, _phantom }`. Refactor `AssetStorage` to use a `generations: Vec<u32>` alongside `slots: Vec<Option<T>>` and a `free_list`. On insert into a freed slot, bump that slot's generation. `Handle` stores the generation it was created with; stale handles mismatch. This is a clean refactor of `storage.rs` + `handle.rs`.

**Why defer:** The current scheme *works* — stale handles return `None`. It's not blocking. But it's non-standard and should be aligned with the target design before the asset system sees heavy use.

#### 🟢 L2. `AssetStorage` panics on invalid handle index (`unwrap()`)

**Where:** `assets/storage.rs` (`get`, `get_mut`, `remove` all do `self.storage.get(handle.index).unwrap()`).

**Problem:** If a handle has an out-of-bounds `index` (shouldn't happen in normal use, but could from a corrupted/deserialized handle), the code panics instead of returning `None`.

**Fix:** Use `self.storage.get(handle.index)?` (return `None` on out-of-bounds) — the `Option`-returning `.get()` on `Vec`/slice. Consistent with "stale handles resolve to `None`, never panic."

#### 🟢 L3. `AssetError` has no context (no file path, no source error)

**Where:** `assets/error.rs` (`enum AssetError { FileNotFound, UnsupportedExtension, ... }` — unit variants, no payloads).

**Problem:** Errors are opaque — `FileNotFound` doesn't say *which* file, `LoaderNotFound` doesn't say *which* extension. Debugging is hard.

**Fix:** Add payloads: `FileNotFound(PathBuf)`, `UnsupportedExtension(String)`, `LoaderNotFound { ext: String, asset_type: TypeId }`. Implement `std::error::Error` + `Display`. Consider an `IoError(io::Error)` variant for underlying I/O failures.

#### 🟢 L4. `AssetLoader::load` takes `&mut self` but `ErasedLoader::load_erased` also takes `&mut self`

**Where:** `assets/load.rs`.

**Problem:** Loaders are stored as `Box<dyn ErasedLoader>` in a `Vec`. The `get_by_ext` / `get_by_type` methods return `&mut Box<dyn ErasedLoader>`, requiring `&mut self` on `AssetLoadersStorage`. This means `AssetsManager::load` needs `&mut self` even though loading is conceptually read-only. Not a bug, but limits future concurrent loading.

**Fix:** Fine for V1 (single-threaded). If loaders need interior state, they can use `Mutex` internally. No change needed now.

#### 🟢 L5. `init().unwrap()` on `GraphicsContext::new` — error discarded into `engine_error`

**Where:** `app.rs` (`init()` uses `?` correctly now, but `run()` does `event_loop.run_app(&mut self).unwrap()`).

**Problem:** `run_app` can fail (e.g., event loop creation), and `.unwrap()` panics instead of returning an `EngineError`.

**Fix:** Wrap `run_app` result into `engine_error` like `init` does, or map it to an `EngineError` variant. Low priority — event loop creation rarely fails after `EventLoop::new()` succeeds.

#### 🟢 L6. No `glam` / `bytemuck` / `image` dependencies

**Where:** `nova-core/Cargo.toml`.

**Problem:** The target architecture needs `glam` (math), `bytemuck` (safe casting for buffer uploads), and `image` (texture decoding). None are in `Cargo.toml`.

**Fix:** Add them when implementing materials, cameras, and texture loading (Phase 1 of the roadmap).

### A.5 What's Already Good (Keep)

- **`Application<P>` event loop + fixed-timestep update** — solid, keep as-is. Only needs `on_render` added.
- **`ApplicationBuilder`** — fluent, ergonomic. Extend with `.with_present_mode(...)`.
- **`GraphicsContext`** — correct wgpu setup (instance, adapter, device, queue, surface config, sRGB format selection). Keep; wrap in `RenderContext`.
- **`Clock`** — simple and correct.
- **`AssetsManager` TypeMap + loader dispatch structure** — the *shape* is right (TypeMap by `TypeId`, extension dispatch, type-erased loaders). It just needs concrete asset types, concrete loaders, and a switch from `Arc<GraphicsContext>` to `Arc<RenderContext>`.
- **`WindowApi`** — minimal but fine. Will need public accessors for `nova-2d`/`nova-3d` (or pass through `AppContext`).
- **Visibility discipline** — `GraphicsContext` fields are `pub(crate)`, `AssetsManager::new` is `pub(crate)`. The intent to hide internals is already there; `RenderContext` will formalize it.

---

## Part B — Step-by-Step Migration Walkthrough

This is the ordered path from the current code to the target architecture. Each step is self-contained and leaves the codebase in a working state.

### Step 0 — Immediate fixes (no new types needed)

**Goal:** Fix the two issues that are pure bugs/regressions, independent of the architecture work.

**Tasks:**
1. **Fix surface loss panic** (C4): In `app/handler.rs`, replace `panic!("surface lost")` with `gfx.configure_surface(); return;`. This is a one-line fix you can do *right now*.
2. **Fix `AssetStorage` panics** (L2): Change `self.storage.get(handle.index).unwrap()` to `self.storage.get(handle.index)?` in `get`, `get_mut`, `remove`. Returns `None` on bad index instead of panicking.

**Deliverable:** No behavior change visible to the user, but the engine no longer crashes on surface loss or corrupted handles.

---

### Step 1 — `RenderContext` (minimal)

**Goal:** Create the public render hub wrapping `GraphicsContext`. This unblocks everything else.

**Why first:** `Frame`, `AssetManager` refactor, and `on_render` all depend on `RenderContext` existing. The roadmap doc notes Steps 1 and 2 are interdependent — build `RenderContext` first (minimal), then `Frame`, then refactor `AssetsManager`.

**Tasks:**
1. Create `nova-core/src/graphics/render_context.rs` with:
   ```rust
   pub struct RenderContext {
       inner: Mutex<GraphicsContext>,
       // pipeline_cache, bind_group_allocator added later
   }
   ```
2. Implement `device()`, `queue()`, `surface_format()` accessors (lock the `Mutex`, return a guard or clone the needed reference).
3. Implement `resize(width, height)` and `reconfigure_surface()` (delegate to `GraphicsContext`).
4. Implement `begin_frame()` — *minimal version*: acquire surface texture (with loss recovery), create view, create encoder, return a `Frame` (created in Step 2). For now, `Frame` can be a thin wrapper.
5. Update `GraphicsContext` visibility: keep fields `pub(crate)`, but move surface config management into `RenderContext`.
6. Make present mode configurable: add `present_mode` to `GraphicsContext::new` / `RenderContext`, default `AutoVsync`. Plumb through `ApplicationBuilder::with_present_mode`.

**Deliverable:** `RenderContext` exists and wraps `GraphicsContext`. `GraphicsContext` is no longer accessed directly outside `RenderContext`.

---

### Step 2 — `Frame` + `RenderPass`

**Goal:** Per-frame abstraction with RAII submit + present.

**Tasks:**
1. Create `nova-core/src/graphics/frame.rs` with `Frame<'a>`:
   - Fields: `view: TextureView`, `encoder: CommandEncoder`, `uniform_arena: UniformArena` (stub for now — empty struct), `color_format`, `depth_format: Option<TextureFormat>`, `frame_index: u64`.
   - `begin_pass(desc) -> RenderPass<'_>` — wraps `wgpu::RenderPass`.
   - `Drop` impl: finish encoder, submit to queue, present surface texture.
2. Create `nova-core/src/graphics/render_pass.rs` with `RenderPass<'frame>`:
   - Wraps `wgpu::RenderPass<'frame>`.
   - `set_pipeline`, `set_bind_group`, `set_vertex_buffer`, `set_index_buffer`, `draw`, `draw_indexed`.
3. Add a depth texture pool to `RenderContext` (auto-resizes with surface). `begin_pass(depth: bool)` grabs one when needed.
4. Implement `UniformArena` (minimal): a per-frame staging buffer allocator. `upload_uniform(bytes) -> BindGroupEntry`. Reset on each frame.

**Deliverable:** `Frame` can be created, a `RenderPass` opened, and on `Frame::drop` the commands submit and present. The hardcoded `render()` function can now be replaced.

---

### Step 3 — Replace `render()` + add `on_render`

**Goal:** The proxy can now render. The hardcoded clear-color pass is gone.

**Tasks:**
1. Add `on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame)` to `ApplicationProxy`.
2. In `app/handler.rs`, replace the `render(&ctx.gfx)` call with:
   ```rust
   let mut frame = ctx.render.begin_frame();
   proxy.on_render(ctx, &mut frame);
   // frame dropped here → submit + present
   ```
3. Update `ApplicationContext`: replace `gfx: Arc<GraphicsContext>` with `render: Arc<RenderContext>`. Update `Application::init()` to construct `RenderContext` (wrap `GraphicsContext`), then `AssetsManager::new(render.clone())`.
4. Update `nova-test/src/main.rs`: implement `on_render` to clear the screen to a color (proving the pipeline works end-to-end).

**Deliverable:** A clear screen rendered through `on_render` — the proxy controls rendering. The hardcoded `render()` function is gone. This is the **first visible milestone** — the architecture is real.

---

### Step 4 — Refactor `AssetsManager` to use `Arc<RenderContext>`

**Goal:** Align the asset system with the target architecture (loaders access GPU via `RenderContext`, not `GraphicsContext`).

**Tasks:**
1. Change `LoadContext` from `{ gfx: Arc<GraphicsContext> }` to `{ render: Arc<RenderContext> }`.
2. Change `AssetsManager::new(gfx: Arc<GraphicsContext>)` to `AssetsManager::new(render: Arc<RenderContext>)`.
3. Update `Application::init()` to pass `Arc<RenderContext>` to `AssetsManager::new()`.
4. Add accessors on `RenderContext` that loaders need: `device()`, `queue()`, `surface_format()`.

**Deliverable:** The asset system is wired to `RenderContext`. Loaders (once written) can access GPU through the public hub.

---

### Step 5 — Refactor `Handle<T>` + `AssetStorage<T>` to generational design

**Goal:** Align `Handle` with the target `(index: u32, generation: u32)` design.

**Tasks:**
1. Change `Handle<T>` to `{ index: u32, generation: u32, _phantom: PhantomData<T> }`. Remove the `Counter`-based `id`.
2. Refactor `AssetStorage<T>` to `{ slots: Vec<Option<T>>, generations: Vec<u32>, free_list: Vec<u32> }`.
   - `insert`: reuse a free slot (bump its generation) or append. Return `Handle { index, generation }`.
   - `get`/`get_mut`: validate `handle.generation == self.generations[handle.index]`.
   - `remove`: take data, push index to `free_list`, bump `generations[index]`.
3. Update `Hash`/`Eq`/`PartialEq` on `Handle` to hash/compare `(index, generation)`.

**Deliverable:** `Handle<T>` is a compact, standard generational handle. Batch sort keys can use `Handle` directly as a `u64`-equivalent key.

---

### Step 6 — First asset types: `Shader` + `Texture`

**Goal:** The asset system can actually load something.

**Tasks:**
1. Add `glam`, `bytemuck`, `image` to `nova-core/Cargo.toml`.
2. Define `Shader` asset: `{ module: wgpu::ShaderModule }`. Implement `Asset`.
3. Define `Texture` asset: `{ texture: wgpu::Texture, view: wgpu::TextureView }`. Implement `Asset`.
4. Implement `ShaderLoader`: reads `.wgsl` file → `device.create_shader_module(...)`. Extensions: `["wgsl"]`.
5. Implement `TextureLoader`: reads `.png`/`.jpg` → `image::load` → decode → `device.create_texture` + `queue.write_texture`. Extensions: `["png", "jpg"]`.
6. Register `ShaderLoader` and `TextureLoader` in `AssetsManager::new()` (or a `register_default_loaders` method).
7. Improve `AssetError`: add payloads (`FileNotFound(PathBuf)`, `IoError(io::Error)`, etc.) and implement `Display` + `std::error::Error`.

**Deliverable:** `assets.load::<Shader>("shader.wgsl")` and `assets.load::<Texture>("sprite.png")` work and return handles.

---

### Step 7 — `MaterialTemplate` + `Material`

**Goal:** The material model that drives pipeline compilation.

**Tasks:**
1. Define `MaterialTemplate` asset: `{ vertex_shader, fragment_shader, vertex_layout, blend_state, depth_stencil, topology, uniform_layout }`. Implement `Asset`.
2. Define `UniformBinding`, `UniformType`, `UniformValue`.
3. Implement `MaterialTemplate::pipeline_key() -> PipelineKey`.
4. Define `Material`: `{ template: Handle<MaterialTemplate>, uniforms: Vec<UniformValue>, textures: Vec<Handle<Texture>>, uniform_buffer, bind_groups, dirty }`.
5. Implement `Material::set_uniform(name, value)`, `Material::set_texture(binding, texture)`, `Material::new(template)`.
6. Implement `MaterialTemplateLoader`: parses `.mat.toml` (or `.mat.ron`), loads nested `Shader` assets via `ctx.load::<Shader>(...)`, builds the template. Extensions: `["mat.toml"]`.
7. Register `MaterialTemplateLoader`.

**Deliverable:** Materials can be created from templates. Templates can be loaded from files with nested shader dependencies.

---

### Step 8 — `PipelineCache` + `BindGroupAllocator` in `RenderContext`

**Goal:** Pipeline compilation is deduplicated; bind groups are allocated efficiently.

**Tasks:**
1. Implement `PipelineCache`: `HashMap<(PipelineKey, TextureFormat), wgpu::RenderPipeline>`. `get_or_compile(key, format, device)`.
2. Implement `BindGroupAllocator`: creates `wgpu::BindGroup` from material data + layout. Pool descriptor sets to avoid churn.
3. Add both to `RenderContext`.
4. Implement `Material::ensure_bound(device, queue)` — update uniform buffer + rebuild bind groups if `dirty`.
5. Implement `RenderPass::draw_material(&material, ...)` — convenience: bind pipeline + bind groups + draw.

**Deliverable:** Materials compile pipelines (cached by template) and bind groups on first use; subsequent uses are cache hits.

---

### Step 9 — Default resources + first render test

**Goal:** A colored quad on screen through the full public API.

**Tasks:**
1. Write `nova-core/src/graphics/defaults/shader_2d_flat.wgsl` (vertex: position + color + ortho projection uniform; fragment: output color).
2. Register default shaders at `RenderContext` init via `include_str!`.
3. Create a default `MaterialTemplate` (2D flat) + default white 1×1 `Texture`.
4. In `nova-test`, create a `Material` from the default template, set `u_color`, define quad vertices, and render in `on_render` using `RenderPass::draw_material`.

**Deliverable:** A colored quad on screen, rendered entirely through the public API. Proves the architecture end-to-end. **This is the major milestone** — the core rendering pipeline is complete.

---

### Step 10 — `DrawBatch` + `submit_draw_batch`

**Goal:** The dimension-agnostic submission contract is ready for batchers.

**Tasks:**
1. Define `DrawBatch` in `nova-core`: `{ template_key, material, bind_groups, vertex_buffer, vertex_count, instance_count, uniform_data, render_pass_descriptor }`.
2. Implement `Frame::submit_draw_batch(batch)` — pipeline lookup, bind group creation, uniform upload, command recording.
3. This is the seam where `nova-2d`/`nova-3d` batchers will plug in.

**Deliverable:** `Frame::submit_draw_batch()` works. The contract for dimension-specific batchers is ready.

---

### Step 11 — Split `nova-2d` crate

**Goal:** The first dimension crate, with sprite batching.

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

**Deliverable:** Textured sprites on screen with a 2D camera. The first real `nova-2d` component, built entirely on the shared base.

---

### Step 12 — Split `nova-3d` crate

**Goal:** 3D rendering following the same pattern.

**Tasks:**
1. Create `nova-3d/` crate depending on `nova-core`.
2. Define `Vertex3D` (position, normal, UV), `MeshCmd`, `BatchKey3D { template, mesh, material }`.
3. Implement `Batcher3D`: collect, sort (material then back-to-front for transparency), flush. Instancing for repeated meshes.
4. Implement `Render3D<'a>`: `draw_mesh(mesh, material, transform)`, `draw_instanced(mesh, material, instances)`.
5. Implement `MeshRenderer`: draws `Mesh` assets.
6. Implement `Camera3D`: perspective projection → uniform bytes.
7. Implement `LightSystem`: directional/point/spot lights → uniform bind group.
8. Implement `SceneUniforms`: camera + lighting, uploaded once per frame via `UniformArena`.
9. Write `shader_3d_unlit.wgsl` and `shader_3d_pbr.wgsl`, create default 3D material templates.
10. Re-export core types from `nova-3d/src/lib.rs`.
11. In `nova-test`, load a mesh, render with `Render3D` + `Camera3D` + `LightSystem`.

**Deliverable:** 3D meshes on screen with camera and lighting. The `nova-3d` crate is complete.

---

### Step 13+ — Polish & future

- **Hot-reload:** `reload(handle, new_source)` — propagate through template handles.
- **Optional `nova` umbrella crate:** facade with `2d`/`3d` feature flags.
- **Async loading:** `LoadingHandle<T>`, deferred resolution (two-phase load).
- **ECS integration:** optional backend (feature flag).
- **Culling:** frustum culling in `nova-3d`.
- **Asset deduplication:** source-hash based, when memory waste is measured.

---

## Summary: Priority at a Glance

| Priority | Issue | Step | Effort |
|----------|-------|------|--------|
| 🔴 Critical | Surface loss panics (C4) | Step 0 | Trivial (1 line) |
| 🔴 Critical | No `on_render` (C1) | Step 3 | Small (trait + handler) |
| 🔴 Critical | No `RenderContext` (C2) | Step 1 | Medium |
| 🔴 Critical | No `Frame`/`RenderPass` (C3) | Step 2 | Medium |
| 🔴 Critical | Present mode uncontrolled (C5) | Step 1 | Small |
| 🔴 Critical | No depth buffer (C6) | Step 2 | Small |
| 🟡 High | No loaders/asset types (H1) | Step 6 | Medium |
| 🟡 High | `LoadContext` uses `GraphicsContext` (H2) | Step 4 | Small |
| 🟡 High | `ApplicationContext` leaks `GraphicsContext` (H3) | Step 3 | Small |
| 🟡 High | `handler.rs` reaches into `GraphicsContext` (H4) | Step 3 | Resolved by C2/C3 |
| 🟡 High | No loader registration hook for 2d/3d (H5) | Step 6+ | Small |
| 🟢 Low | `Handle` non-generational design (L1) | Step 5 | Small refactor |
| 🟢 Low | `AssetStorage` panics on bad index (L2) | Step 0 | Trivial |
| 🟢 Low | `AssetError` no context (L3) | Step 6 | Small |
| 🟢 Low | `run_app().unwrap()` (L5) | Anytime | Trivial |
| 🟢 Low | Missing deps (L6) | Step 6 | Trivial |

**The critical path:** Step 0 → Step 1 → Step 2 → Step 3. After Step 3, the proxy can render and the architecture is real. Steps 4–9 build out the asset + material pipeline. Steps 10–12 add dimension crates. Everything after is polish.