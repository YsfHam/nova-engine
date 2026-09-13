# Renderer Major Update — Execution Plan

> Status: **DRAFT — revised from implementation notes**
> Last updated: 2026-09-12

## Design decisions (locked)

| # | Decision | Rationale |
|---|---|---|
| D1 | `Texture` CPU asset **keeps** raw pixel data after GPU upload | Hot-reload, re-upload, CPU reads |
| D2 | `GenericHandle` becomes **public**; `DrawBatch` holds it | Material trait objects can't be keyed by concrete `Handle<A>` |
| D3 | `Material` exposes only static type-level template information; `AsBindGroup` describes instance data | Templates are registered once; bind groups are rebuilt each frame |
| D4 | `Quad` is **deleted** from nova-2d; `Sprite` is the only 2D drawable | Doc says "Quads will be removed" |
| D5 | `MaterialTemplate` is **owned by the Material impl** (not an asset) | `template()` returns the complete static template |

---

## Phase overview

```
Phase 1: Asset system restructure and type-erased lookup (nova-core)
    ├── 1A: Remove Asset::Metadata, AssetsLoader, resolution, load_from_file
    ├── 1B: Redefine Asset trait (insert directly)
    ├── 1C: Make GenericHandle public
    └── 1D: Add trait-based asset query

Phase 2: CPU assets, renderer resources, and bind-group descriptors (nova-core)
    ├── 2A: Texture → CPU asset (raw data + config); GpuTexture → renderer cache
    ├── 2B: Sampler → renderer-managed (config in Texture); GpuSampler cache
    ├── 2C: Shader → renderer-managed (`ShaderSource`); `Shader` cache
    └── 2D: Material template + AsBindGroup + BindGroup

Phase 3: Material registration and rendering pipeline rework (nova-core)
    ├── 3A: DrawBatch with GenericHandle + builder pattern
    ├── 3B: Material registration and template cache
    ├── 3C: Per-frame AsBindGroup → bind groups
    ├── 3D: submit_batches rework
    └── 3E: Resource caching strategy (samplers, textures, shaders, pipelines only)

Phase 4: Nova-2d rework
    ├── 4A: Delete Quad, rewrite Sprite as the sole drawable
    ├── 4B: Implement a 2D Material (ColorMaterial → SpriteMaterial)
    ├── 4C: Batcher2D rework (groups by GenericHandle material)
    ├── 4D: Render2D API (sprite + transform + material → batcher)
    └── 4E: Plugin + defaults rework

Phase 5: Integration & validation
    ├── 5A: nova-test: colored sprite (ColorMaterial)
    ├── 5B: nova-test: textured sprite (SpriteMaterial)
    └── 5C: Build clean + run + success mark met
```

---

## Phase 1 — Asset system restructure

### 1A: Remove old loading infrastructure
**Files:** `assets/load.rs`, `assets/resolve.rs`, `assets.rs`

- Remove `AssetLoader` trait, `AssetLoadersStorage`, `ErasedLoader`, `LoadContext`
- Remove `AssetsManager::register_loader`, `AssetsManager::load`, `AssetsManager::load_from_file`
- Remove `resolve.rs` entirely (ResolvedMaterial, ResolvedMaterialTemplate, ResolvedTexture)
- `AssetsManager` keeps: `storages`, `render_ctx`, `insert_asset`, `get_asset`, `get_asset_mut`, `remove_asset`

### 1B: Redefine Asset trait
**Files:** `assets.rs`

```rust
pub trait Asset: 'static {}
```
- No `Metadata` associated type
- Assets are inserted directly via `AssetsManager::insert_asset(asset)` or constructed by the user

### 1C: Make GenericHandle public
**Files:** `assets/handle.rs`

- Move `GenericHandle` from `pub(crate)` to `pub`
- `Handle<A>::into_generic()` → `GenericHandle` (public method)
- `GenericHandle::try_into_handle<A>()` → `Result<Handle<A>, ()>` (public method)

### 1D: Add trait-based asset query
**Files:** `assets.rs`

- Preserve the typed path: `get_asset(handle: Handle<A>) -> Option<&A>`.
- Add a generic path: `get_asset_any(generic: GenericHandle) -> Option<&dyn Any>`.
- Do not promise that `Any` can be directly cast to an arbitrary trait object. Rust cannot recover an `&dyn Material` from `&dyn Any` without a registered adapter.
- Add material registration in Phase 3. Registration stores a type-erased lookup function for material type `M`; that function downcasts the concrete asset and returns `&dyn AsBindGroup` (or the registered material trait object).
- Keep `GenericHandle` type identity and generation checks in the manager; invalid or stale handles return `None`.

**Validation checkpoint:** `cargo check` passes on nova-core after Phase 1. All existing code that calls `load()` will be broken — that's expected; it gets rewritten in later phases.

---

## Phase 2 — CPU assets, renderer resources, and bind-group descriptors

### 2A: Texture → CPU asset; GpuTexture → renderer cache
**Files:** `graphics/texture.rs`, `graphics/render.rs` (new cache)

**CPU `Texture` (asset):**
```rust
pub struct Texture {
    source: TextureSource,
    data: Vec<u8>,            // raw pixels, kept per D1
    config: TextureConfig,
}
impl Asset for Texture {}
```
- Introduce `TextureConfig` to group size, format, mip count, sample count, usage, label, and `SamplerConfig`.
- Constructors: `Texture::from_file(path, config)` and `Texture::from_raw(data, config)`.
- File construction decodes the source into CPU data before insertion; GPU upload happens only when the renderer first needs the texture.
- No `wgpu::Texture` — purely CPU data

**`GpuTexture` (renderer-owned):**
```rust
pub(crate) struct GpuTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,  // created from SamplerConfig
}
```
- `RenderContext` gains: `texture_cache: HashMap<Handle<Texture>, GpuTexture>`
- `get_or_create_gpu_texture(handle, texture_asset, device, queue) -> &GpuTexture` — creates on first access, cached

### 2B: Sampler → renderer-managed
**Files:** `graphics/sampler.rs` → becomes `SamplerConfig` only; GPU sampler in render cache

- `SamplerConfig` (was `SamplerMetadata`): address modes, filters, lod, compare, anisotropy, border — all engine-native enums (serializable)
- `GpuSampler`: `wgpu::Sampler` — cached in `RenderContext` by `SamplerConfig` (hash + eq)
- Sampler is no longer an asset; no `Handle<Sampler>`

### 2C: Shader → renderer-managed
**Files:** `graphics/shader.rs`

**`ShaderSource` (CPU, stored by `MaterialTemplate`):**
```rust
pub enum ShaderSource {
    File(PathBuf),
    Inline(String),
    // entry points included
}
```

**`Shader` (renderer-owned):**
```rust
pub(crate) struct Shader {
    module: wgpu::ShaderModule,
    entry_point: ShaderEntryPoint,
}
```
- Rename the current GPU-backed `GpuShader` concept to `Shader`; there is no public CPU shader asset.
- `RenderContext` gains a shader cache keyed by a stable source/entry-point hash.
- `get_or_compile_shader(source, device) -> &Shader` loads file sources before hashing/compilation.

### 2D: Material template + AsBindGroup + BindGroup
**Files:** `graphics/material.rs` (rewritten)

**`MaterialTemplate`:**
```rust
pub struct MaterialTemplate {
    pub shader: ShaderSource,
    pub buffer_layout: BufferLayout,
    pub instance_layout: Option<BufferLayout>,
    pub blend_state: BlendMode,
    pub depth_stencil: Option<DepthStencilConfig>,
    pub topology: Topology,
    pub bind_group_layout: BindGroupLayoutDescriptor,
}
```
- `MaterialTemplate` contains all information required to create a pipeline, including `ShaderSource` and bind-group layout information.
- It replaces `MaterialTemplateDescriptor`; shader fields are no longer split between two types.

**`Material` trait:**
```rust
pub trait Material: Asset + AsBindGroup + Send + Sync + 'static {
    fn template() -> MaterialTemplate;
}
```
- `template()` is the unique static function. It is called during `register_material::<M>()`, not once per draw.
- Concrete materials own their instance fields and implement `AsBindGroup`.

**`AsBindGroup` and `BindGroup`:**
```rust
pub trait AsBindGroup {
    fn as_bind_group(&self) -> BindGroup;
}

pub struct BindGroup {
    pub entries: Vec<BindGroupEntry>,
}

pub enum BindGroupEntry {
    Uniform { binding_slot: u32, visibility: ShaderStage, value: UniformValue },
    Texture { binding_slot: u32, sampler_binding_slot: u32, visibility: ShaderStage, texture: GenericHandle,
              view_dimension: TextureViewDimension, sample_type: TextureSampleType },
}
```
- `BindGroup::layout_descriptor()` produces the `wgpu::BindGroupLayoutDescriptor` data.
- `BindGroup::descriptor(...)` produces the `wgpu::BindGroupDescriptor` after the renderer resolves texture handles and derives samplers from `TextureConfig`.
- `EnvironmentDescriptor` becomes a `BindGroup` built from environment uniform entries; environment and material binding descriptions use the same abstraction.
- `GpuMaterial` is removed. There is no material bind-group cache: `AsBindGroup` is evaluated and its bind group is recreated each frame.

- Remove old `Material` struct, `MaterialMetadata`, `MaterialTemplateMetadata`, `MaterialLoader`, `MaterialTemplateLoader`, and asset-backed shader/material-template loading.

**Validation checkpoint:** `cargo check` on nova-core. Old material system gone; new types defined.

---

## Phase 3 — Rendering pipeline rework

### 3A: DrawBatch with GenericHandle + builder pattern
**Files:** `graphics/draw_batch.rs`

```rust
pub struct DrawBatch {
    material: GenericHandle,           // was Handle<Material>
    geometry: BatchGeometry,
    instance_batch: Option<InstanceBatch>,
}
```
- Builder pattern: `DrawBatch::new(material: Handle<Material>).with_instances(...)`
- Immutable after construction

### 3B: Pipeline cache rework
**Files:** `graphics/pipeline.rs`

- Add material registration before drawing: `register_material::<M>()` calls `M::template()` and stores a type-erased material registration keyed by `TypeId`.
- Registration stores the template hash and the adapter needed to query `M` from a `GenericHandle`.
- `PipelineCacheKey` contains `{ template_hash, target_format, scene_bind_group_layout_key, material_bind_group_layout_key }`.
- The shader identity is part of the registered template hash because `MaterialTemplate` contains `ShaderSource`.
- `get_or_compile(key: PipelineCacheKey) -> &Pipeline` uses the registered template's bind-group layout; it does not inspect individual material values.
- Hashing must use stable engine-native values, not raw `wgpu` handles.

### 3C: Per-frame material compilation → bind groups
**Files:** `graphics/bind.rs` (rewritten)

- Remove material bind-group caching and `BindGroupAllocator`'s material-handle map.
- For each batch, use the material registration adapter to obtain `&dyn AsBindGroup` from the `GenericHandle`.
- Call `as_bind_group()` and validate its entries against the registered `MaterialTemplate` layout.
- Create uniform buffers/bind-group entries from uniform values; resolve texture handles through the renderer texture cache and derive the sampler from each texture's `TextureConfig`.
- Build the `wgpu::BindGroup` for every material submission frame. Reuse only reusable upload buffers, not material bind groups.

### 3D: Submit batches rework
**Files:** `graphics/render_target.rs`

Flow per doc:
1. Iterator of `DrawBatch` submitted
2. Upload geometry + instances (staging / geometry pool)
3. For each batch:
    - Resolve the registered material type from `GenericHandle`
    - Get the registered `MaterialTemplate` and compile/load its `Shader` (cached)
    - Create or retrieve the pipeline using the template and all bind-group layout keys
    - Call `AsBindGroup::as_bind_group()` and create a fresh material bind group
   - Set buffers + draw call

### 3E: Resource caching strategy
- **Shaders**: cached by source hash (in RenderContext)
- **Pipelines**: cached by template hash, target format, and both bind-group layout keys
- **Textures**: cached by GenericHandle (asset identity)
- **Samplers**: cached by SamplerConfig (hash + eq)
- **Material bind groups**: deliberately not cached until material change detection exists

**Validation checkpoint:** `cargo check` on nova-core. Rendering pipeline compiles.

---

## Phase 4 — Nova-2d rework

### 4A: Delete Quad, rewrite Sprite
**Files:** `quad.rs` (delete), `sprite.rs` (rewrite)

- Delete `quad.rs` and all `Quad` references
- `Sprite` becomes the sole drawable: holds material (Handle<Material>), transform, color, z_index, uv

### 4B: Implement 2D materials
**Files:** new `materials.rs` or per-material files

- `ColorMaterial`: implements `Material` and `AsBindGroup`, with a color uniform and no texture entry.
- `SpriteMaterial`: implements `Material` and `AsBindGroup`, with a texture handle and color tint uniform.
- Each defines one static `MaterialTemplate` containing its WGSL source and layout.
- Registration happens during plugin initialization via `register_material::<ColorMaterial>()` and `register_material::<SpriteMaterial>()`.

### 4C: Batcher2D rework
**Files:** `batcher.rs`

- Groups by `z_index` then by `Handle<Material>` (material identity)
- `into_iter()` → `DrawBatch` with `Handle<Material>`

### 4D: Render2D API
**Files:** `render2d.rs`

```rust
renderer.draw_sprite(sprite, transform);
```
- `draw_sprite` takes a `Sprite` + `Transform`, builds `InstanceData2D`, adds to batcher
- `end_scene` → `submit_batches`

### 4E: Plugin + defaults rework
**Files:** `plugin.rs`, `defaults.rs`

- Plugin registers the material types and inserts concrete material instances as ordinary assets.
- Remove old MaterialTemplate, Shader, Sampler, and loader registration.
- Keep base quad geometry registration (internal to sprite rendering)

**Validation checkpoint:** `cargo check` on nova-2d.

---

## Phase 5 — Integration & validation

### 5A: Colored sprite (ColorMaterial)
**Files:** `nova-test/src/main.rs`

- Render a colored sprite using `ColorMaterial` via the new API
- Verify: window opens, colored quad renders

### 5B: Textured sprite (SpriteMaterial)
**Files:** `nova-test/src/main.rs`

- Render a textured sprite using `SpriteMaterial`
- Verify: texture renders correctly

### 5C: Success mark
- Build clean (no warnings)
- App runs
- Both colored and textured sprites render with custom `Material` trait impls

---

## Execution order & dependencies

```
1A → 1B → 1C → 1D    (asset system — sequential, all in nova-core)
        ↓
2A → 2B → 2C → 2D    (resource split — 2A/2B/2C can parallelize, 2D depends on all)
        ↓
3A → 3B → 3C → 3D → 3E   (rendering pipeline — sequential)
        ↓
4A → 4B → 4C → 4D → 4E   (nova-2d — 4A/4B can parallelize, 4C depends on 4A+4B)
        ↓
5A → 5B → 5C          (integration — sequential)
```

## Risk areas
- **Borrow checker in submit_batches**: the current split-borrow pattern is complex; the new per-frame material compilation adds more mutable access patterns. May need restructuring of RenderTargetCommander.
- **Trait object resolution**: `GenericHandle` → `&dyn AsBindGroup` requires a registration adapter; a raw `&dyn Any` is insufficient. Registration must reject unregistered material types and stale handles cleanly.
- **Bind-group layout consistency**: the `BindGroup` entries returned by `AsBindGroup` must match the registered template exactly; validation should report missing, extra, duplicate, or incompatible entries.
- **Pipeline key hashing**: `MaterialTemplate` and both bind-group layouts need stable hashes based on engine-native values. Raw `wgpu` handles must not be hashed.
- **Texture upload timing**: CPU texture assets may be constructed before a renderer exists; upload and cache insertion happen lazily at first use.

## Validation checkpoints and acceptance tests

Each phase must pass its checkpoint before the next phase begins:

1. **Asset API checkpoint:** typed handles still return `&A`; generic handles return `&dyn Any`; registered material adapters return `&dyn AsBindGroup`; stale/unregistered handles fail without panic.
2. **Resource checkpoint:** a CPU `Texture` contains no `wgpu` objects, retains its pixel data, and first render lazily creates exactly one cached GPU texture and sampler.
3. **Bind-group checkpoint:** a material can produce a `BindGroup`; environment uniforms use the same entry model; missing/extra/wrong entries are rejected; bind groups are recreated on successive frames.
4. **Pipeline checkpoint:** two materials of the same registered type share a pipeline; different templates or layout keys do not; shader compilation is cached.
5. **2D checkpoint:** `ColorMaterial` renders an untextured colored sprite and `SpriteMaterial` renders a textured sprite through the same `Render2D` submission path.
6. **Final checkpoint:** `cargo check --workspace`, focused tests for assets/bind groups/cache keys, and a manual `nova-test` run complete without new warnings or validation errors.