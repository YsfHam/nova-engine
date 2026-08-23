# Nova Engine — Code Review & Migration Walkthrough

> **Companion to:** `docs/architecture.md` (the target architecture).
> **Purpose:** Track progress against the target architecture, identify remaining work, and lay out a step-by-step migration path.
> **Last updated:** 2026-08-24

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

### Progress Summary

| Step | Status | Deliverable |
|------|--------|-------------|
| Step 0 — Immediate fixes | ✅ Done | Surface loss recovery, storage bounds safety |
| Step 1 — `RenderContext` | ✅ Done | Public render hub, present mode config, surface loss handling |
| Step 2 — `Frame` + `RenderPass` | ✅ Done | Per-frame RAII, render pass descriptor, draw methods, `Color` type |
| Step 3 — `on_render` | ✅ Done | **Blue window** — proxy controls rendering via public API |
| Step 4 — `AssetsManager` → `RenderContext` | ✅ Done | Loaders wired to `RenderContext` (done during Step 1) |

**Critical issues resolved:** C1 (no `on_render`), C2 (no `RenderContext`), C3 (no `Frame`/`RenderPass`), C4 (surface loss), C5 (present mode), H2 (loaders use `GraphicsContext`), H3 (`ApplicationContext` leaks GPU).

**Deferred (not blocking):** C6 (depth buffer — needed for 3D), `UniformArena` (needed for cameras/3D), `frame_index` (double-buffering).

---

## Part B — Remaining Migration Walkthrough

Each step is self-contained and leaves the codebase in a working state.

### Step 5 — Refactor `Handle<T>` + `AssetStorage<T>` to generational design

**Goal:** Align `Handle` with the target `(index: u32, generation: u32)` design — compact, standard, and usable as batch sort keys.

**Why now:** Before the asset system sees heavy use (loaders, materials), align the handle design. The current `Handle { id: u64, index: usize }` uses a global monotonic counter instead of per-slot generations. It works but is non-standard and wasteful (`index` is `usize` = 8 bytes).

**Tasks:**
1. Change `Handle<T>` to `{ index: u32, generation: u32, _phantom: PhantomData<T> }`. Remove the `Counter`-based `id`.
2. Refactor `AssetStorage<T>` to `{ slots: Vec<Option<T>>, generations: Vec<u32>, free_list: Vec<u32> }`.
   - `insert`: reuse a free slot (bump its generation) or append. Return `Handle { index, generation }`.
   - `get`/`get_mut`: validate `handle.generation == self.generations[handle.index]`.
   - `remove`: take data, push index to `free_list`, bump `generations[index]`.
3. Update `Hash`/`Eq`/`PartialEq` on `Handle` to hash/compare `(index, generation)`.

**Deliverable:** `Handle<T>` is a compact 8-byte generational handle. Batch sort keys can use `Handle` directly.

**Dependencies:** None — pure refactor of `assets/handle.rs` + `assets/storage.rs`.

---

### Step 6 — First asset types: `Shader` + `Texture`

**Goal:** The asset system can actually load something. `assets.load::<Shader>("shader.wgsl")` and `assets.load::<Texture>("sprite.png")` work and return handles.

**Why now:** This is the foundation for materials (Step 7) and any visible geometry (Step 9). Without loadable assets, the engine can only clear the screen.

**Tasks:**
1. Add `glam`, `bytemuck`, `image` to `nova-core/Cargo.toml`.
2. Define `Shader` asset: `{ module: wgpu::ShaderModule }`. Implement `Asset`.
3. Define `Texture` asset: `{ texture: wgpu::Texture, view: wgpu::TextureView }`. Implement `Asset`.
4. Implement `ShaderLoader`: reads `.wgsl` file → `device.create_shader_module(...)`. Extensions: `["wgsl"]`.
5. Implement `TextureLoader`: reads `.png`/`.jpg` → `image::load` → decode → `device.create_texture` + `queue.write_texture`. Extensions: `["png", "jpg"]`.
6. Register `ShaderLoader` and `TextureLoader` in `AssetsManager::new()` (or a `register_default_loaders` method).
7. Improve `AssetError`: add payloads (`FileNotFound(PathBuf)`, `IoError(io::Error)`, etc.) and implement `Display` + `std::error::Error`.

**Deliverable:** `assets.load::<Shader>("shader.wgsl")` and `assets.load::<Texture>("sprite.png")` work and return handles.

**Dependencies:** `AssetsManager` already wired to `RenderContext` (Step 4). `Handle` refactor (Step 5) recommended but not strictly required.

**Note on borrow discipline:** Loaders hold `Rc<RefCell<RenderContext>>`. During `load()`, the loader does `ctx.render_ctx.borrow().device()` to access the GPU. Ensure no overlapping `borrow_mut()` is active during loading (it isn't — loading happens outside `begin_frame`).

---

### Step 7 — `MaterialTemplate` + `Material`

**Goal:** The material model that drives pipeline compilation. Templates are assets (loaded from file, shared); materials are lightweight instances.

**Why now:** Materials are the bridge between assets (shaders, textures) and rendering. Without them, you can't draw anything with a pipeline.

**Tasks:**
1. Define `MaterialTemplate` asset: `{ vertex_shader, fragment_shader, vertex_layout, blend_state, depth_stencil, topology, uniform_layout }`. Implement `Asset`.
2. Define `UniformBinding`, `UniformType`, `UniformValue`.
3. Implement `MaterialTemplate::pipeline_key() -> PipelineKey`.
4. Define `Material`: `{ template: Handle<MaterialTemplate>, uniforms: Vec<UniformValue>, textures: Vec<Handle<Texture>>, uniform_buffer, bind_groups, dirty }`.
5. Implement `Material::set_uniform(name, value)`, `Material::set_texture(binding, texture)`, `Material::new(template)`.
6. Implement `MaterialTemplateLoader`: parses `.mat.toml` (or `.mat.ron`), loads nested `Shader` assets via `ctx.load::<Shader>(...)`, builds the template. Extensions: `["mat.toml"]`.
7. Register `MaterialTemplateLoader`.

**Deliverable:** Materials can be created from templates. Templates can be loaded from files with nested shader dependencies.

**Dependencies:** Step 6 (`Shader` + `Texture` assets). Step 5 (`Handle` refactor) recommended.

---

### Step 8 — `PipelineCache` + `BindGroupAllocator` in `RenderContext`

**Goal:** Pipeline compilation is deduplicated; bind groups are allocated efficiently.

**Why now:** Without pipeline caching, every material recompiles its pipeline on first use. With many materials, this stalls. The cache keys off the template, so materials sharing a template share a pipeline.

**Tasks:**
1. Implement `PipelineCache`: `HashMap<(PipelineKey, TextureFormat), wgpu::RenderPipeline>`. `get_or_compile(key, format, device)`.
2. Implement `BindGroupAllocator`: creates `wgpu::BindGroup` from material data + layout. Pool descriptor sets to avoid churn.
3. Add both to `RenderContext`.
4. Implement `Material::ensure_bound(device, queue)` — update uniform buffer + rebuild bind groups if `dirty`.
5. Implement `RenderPass::draw_material(&material, ...)` — convenience: bind pipeline + bind groups + draw.

**Deliverable:** Materials compile pipelines (cached by template) and bind groups on first use; subsequent uses are cache hits.

**Dependencies:** Step 7 (`MaterialTemplate` + `Material`).

---

### Step 9 — Default resources + first render test (colored quad)

**Goal:** A colored quad on screen through the full public API. **This is the major end-to-end milestone.**

**Why now:** Proves the entire pipeline works: assets → materials → pipelines → frame → render pass → draw → submit → present.

**Tasks:**
1. Write `nova-core/src/graphics/defaults/shader_2d_flat.wgsl` (vertex: position + color + ortho projection uniform; fragment: output color). Embedded via `include_str!`.
2. Register default shaders at `RenderContext` init.
3. Create a default `MaterialTemplate` (2D flat) + default white 1×1 `Texture`.
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
- **Asset deduplication:** source-hash based, when memory waste is measured.
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
| H1 — No loaders/asset types | 🟡 High | ⏳ Step 6 | — | `ShaderLoader`, `TextureLoader` |
| H2 — `LoadContext` uses `GraphicsContext` | 🟡 High | ✅ Resolved | Step 1/4 | Now `Rc<RefCell<RenderContext>>` |
| H3 — `ApplicationContext` leaks GPU | 🟡 High | ✅ Resolved | Step 3 | Holds `Rc<RefCell<RenderContext>>` |
| H4 — `handler.rs` reaches into `GraphicsContext` | 🟡 High | ✅ Resolved | Step 3 | `render()` deleted; goes through `Frame` |
| H5 — No loader registration hook for 2d/3d | 🟡 High | ⏳ Step 6 | — | Register defaults in `AssetsManager::new` |
| L1 — `Handle` non-generational design | 🟢 Low | ⏳ Step 5 | — | Refactor to `(index: u32, generation: u32)` |
| L2 — `AssetStorage` panics on bad index | 🟢 Low | ✅ Resolved | Step 0 | `get_mut`/`remove` use `?` |
| L3 — `AssetError` no context | 🟢 Low | ⏳ Step 6 | — | Add payloads + `Display` |
| L5 — `run_app().unwrap()` | 🟢 Low | ⏳ Anytime | — | Store error into `engine_error` |
| L6 — Missing deps (`glam`, `bytemuck`, `image`) | 🟢 Low | ⏳ Step 6 | — | Add when implementing loaders |

---

## Roadmap at a Glance

```
✅ Step 0  — Immediate fixes (surface loss, storage bounds)
✅ Step 1  — RenderContext (public render hub, present mode)
✅ Step 2  — Frame + RenderPass (RAII frame, render pass descriptor, Color)
✅ Step 3  — on_render (BLUE WINDOW — proxy controls rendering)  ← FIRST VISIBLE MILESTONE
✅ Step 4  — AssetsManager → RenderContext (done during Step 1)

⬜ Step 5  — Handle<T> generational refactor
⬜ Step 6  — Shader + Texture assets + loaders                ← asset system becomes functional
⬜ Step 7  — MaterialTemplate + Material
⬜ Step 8  — PipelineCache + BindGroupAllocator
⬜ Step 9  — Default resources + colored quad                 ← END-TO-END MILESTONE
⬜ Step 10 — DrawBatch + submit_draw_batch (batcher contract)
⬜ Step 11 — nova-2d crate (sprite batching, Camera2D)
⬜ Step 12 — nova-3d crate (meshes, Camera3D, lights, depth pool)
⬜ Step 13+— Polish (hot-reload, umbrella crate, async, ECS, culling)
```

**Next up:** Step 5 (Handle refactor) or Step 6 (first asset types). Step 5 is recommended first to align the handle design before the asset system sees heavy use, but Step 6 can proceed without it if you want to see results faster.