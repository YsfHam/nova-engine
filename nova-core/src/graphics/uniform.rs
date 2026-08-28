use std::{collections::HashMap, num::NonZeroU64};

use wgpu::util::DeviceExt;

use glam::{Mat4, Vec4};

use crate::assets::handle::Handle;
use crate::graphics::material::{Material, MaterialTemplate};

// ──────────────────────────────────────────────────────────────────────────
//  Uniform types — declaration and values.
//
//  These are the engine-native primitives shared across the uniform system.
//  `material.rs` imports them; they live here so there is one place to
//  understand the uniform value model.
// ──────────────────────────────────────────────────────────────────────────

/// Declares a single uniform binding as a `MaterialTemplate`'s shaders expect
/// it. Lives in `MaterialTemplateMetadata.uniform_layout` and drives both
/// bind group layout creation and material load-time validation.
#[derive(Clone, Debug)]
pub struct UniformBinding {
    pub name: String,
    pub ty: UniformType,
    pub binding_slot: u32,
    pub visibility: crate::graphics::shader::ShaderStage,
}

/// The type of a uniform value. Grows as new shaders need more types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniformType {
    Mat4,
    Vec4,
    F32,
}

impl UniformType {
    /// Size in bytes of a value of this type — used to size uniform buffers.
    pub fn size(&self) -> u64 {
        match self {
            UniformType::Mat4 => 64,
            UniformType::Vec4 => 16,
            UniformType::F32 => 4,
        }
    }
}

/// A runtime uniform value. Kept as a typed enum so the material can pack
/// values into a uniform buffer without the caller worrying about layout,
/// and so the material's data stays serializable (the GPU buffer is a
/// derived cache, not the source of truth).
#[derive(Clone, Copy, Debug)]
pub enum UniformValue {
    Mat4(Mat4),
    Vec4(Vec4),
    F32(f32),
}

impl UniformValue {
    pub fn ty(&self) -> UniformType {
        match self {
            UniformValue::Mat4(_) => UniformType::Mat4,
            UniformValue::Vec4(_) => UniformType::Vec4,
            UniformValue::F32(_) => UniformType::F32,
        }
    }

    /// Writes the value into `bytes` at `offset` using WGSL's std140 layout.
    pub fn write_bytes(&self, bytes: &mut [u8], offset: usize) {
        match self {
            UniformValue::Mat4(m) => {
                let cols = m.to_cols_array();
                bytes[offset..offset + 64].copy_from_slice(bytemuck::cast_slice(&cols));
            }
            UniformValue::Vec4(v) => {
                let arr = v.to_array();
                bytes[offset..offset + 16].copy_from_slice(bytemuck::cast_slice(&arr));
            }
            UniformValue::F32(x) => {
                bytes[offset..offset + 4].copy_from_slice(bytemuck::cast_slice(&[*x]));
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  UniformArena — per-frame transient uploads for scene-global data.
//
//  Scene globals (camera projection/view, time, lighting) are reconstructed
//  every frame from runtime state. They do NOT belong in `Material` (which is
//  immutable, asset-owned, serializable). The arena keeps typed staging
//  entries so the buffer can be rebuilt when invalidated, builds one bind
//  group for group 0 per frame, and resets at frame start.
//
//  The arena is the owner of bind group 0 (environment). The material uniform
//  pool (below) owns the per-material uniform buffer for bind group 1.
// ──────────────────────────────────────────────────────────────────────────

/// A typed staging entry held by the [`UniformArena`] until the buffer is
/// built. Keeping the typed value (not just bytes) lets the arena rebuild the
/// GPU buffer when a new upload invalidates a previous one, and supports
/// debug introspection.
struct UniformEntry {
    binding_slot: u32,
    offset: u64,
    size: u64,
    value: UniformValue,
}

/// Per-frame transient uniform storage for scene-global data (camera, time,
/// lighting). Owned by [`Frame`](crate::graphics::frame::Frame). Reset each
/// frame.
///
/// The arena builds **bind group 0** (environment). Call [`build_bind_group`]
/// once per frame after uploading all scene globals; bind the returned group
/// once before drawing. The bind group layout is the singleton scene layout
/// owned by [`RenderContext`](crate::graphics::render::RenderContext).
pub struct UniformArena {
    buffer: Option<wgpu::Buffer>,
    entries: Vec<UniformEntry>,
    /// Total byte size of the packed buffer, with each entry's offset
    /// padded to `min_uniform_buffer_offset_alignment`. Recomputed in
    /// `build_bind_group` from the device's alignment limit.
    packed_size: u64,
    /// The device's `min_uniform_buffer_offset_alignment`, captured on the
    /// first `build_bind_group` so offsets respect the GPU's requirement.
    /// Defaults to 1 (no alignment) until a device is seen.
    min_offset_alignment: u64,
}

impl UniformArena {
    pub fn new() -> Self {
        Self {
            buffer: None,
            entries: Vec::new(),
            packed_size: 0,
            min_offset_alignment: 1,
        }
    }

    /// Uploads a scene-global uniform value at the given binding slot. The
    /// binding slot must match the scene bind group layout (group 0 contract).
    ///
    /// Invalidates any previously built buffer; the next
    /// [`build_bind_group`] call will recreate it.
    pub fn upload(&mut self, binding_slot: u32, value: UniformValue) {
        let size = value.ty().size();

        // If a value for this binding slot was already uploaded this frame,
        // replace it in place (the offset/size stay the same; only the bytes
        // change). This handles e.g. a camera being updated twice.
        if let Some(existing) = self.entries.iter_mut().find(|e| e.binding_slot == binding_slot) {
            existing.value = value;
            self.buffer = None; // bytes changed → invalidate
            return;
        }

        let offset = self.packed_size;
        self.packed_size += size;
        self.entries.push(UniformEntry {
            binding_slot,
            offset,
            size,
            value,
        });
        self.buffer = None; // layout changed → invalidate
    }

    /// Builds (or returns the cached) GPU buffer from the staged entries,
    /// then creates the scene bind group (group 0) using `layout`.
    ///
    /// Returns `None` if no uniforms have been uploaded.
    pub fn build_bind_group(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> Option<wgpu::BindGroup> {
        if self.entries.is_empty() {
            return None;
        }

        // Capture the device's uniform-buffer offset alignment so the bind
        // group entries respect `min_uniform_buffer_offset_alignment`.
        // Tight packing (offset += size) would place e.g. a 4-byte f32 at
        // offset 64 after a mat4, which the GPU rejects (must be 256-aligned
        // on typical hardware). We recompute each entry's offset to the next
        // multiple of the alignment, and size the buffer to fit the last
        // padded entry.
        self.min_offset_alignment = device
            .limits()
            .min_uniform_buffer_offset_alignment
            .max(1) as u64;
        let align = self.min_offset_alignment;

        if self.buffer.is_none() {
            // Recompute padded offsets and total size.
            let mut cursor: u64 = 0;
            for entry in &mut self.entries {
                cursor = cursor.next_multiple_of(align);
                entry.offset = cursor;
                cursor += entry.size;
            }
            let total = cursor.next_multiple_of(align);

            let mut bytes = vec![0u8; total as usize];
            for entry in &self.entries {
                entry.value.write_bytes(&mut bytes, entry.offset as usize);
            }

            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Scene uniform arena buffer"),
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            self.buffer = Some(buffer);
        }

        let buffer = self.buffer.as_ref().unwrap();

        let entries: Vec<wgpu::BindGroupEntry> = self
            .entries
            .iter()
            .map(|entry| wgpu::BindGroupEntry {
                binding: entry.binding_slot,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: entry.offset,
                    size: Some(NonZeroU64::new(entry.size).unwrap()),
                }),
            })
            .collect();

        Some(
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Scene bind group"),
                layout,
                entries: &entries,
            }),
        )
    }

    /// Resets the arena for a new frame. Called by `Frame::new` / `begin_frame`.
    pub fn reset(&mut self) {
        self.buffer = None;
        self.entries.clear();
        self.packed_size = 0;
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  MaterialUniformPool — persistent, batch-built buffer for material uniforms.
//
//  All materials share one `wgpu::Buffer`. Allocations are **per-uniform**:
//  each `(Handle<Material>, binding_slot)` pair maps to its own fixed
//  `(offset, size)` slot inside the shared buffer. Materials are immutable,
//  so allocations are stable across builds. The pool is rebuilt from scratch
//  via [`build`], which the caller (the entity setting up the render pass)
//  invokes when it decides the buffer needs invalidation/recreation.
//
//  This pool feeds bind group 1 (material). The `BindGroupAllocator` asks
//  the pool for a [`wgpu::BindingResource`] per uniform binding (by material
//  handle + binding slot) when building a material's bind group — it never
//  computes offsets itself.
// ──────────────────────────────────────────────────────────────────────────

/// A fixed allocation for a single uniform binding within the material
/// uniform pool: the byte offset and size of that binding's data inside the
/// shared buffer. Keyed by `(Handle<Material>, binding_slot)`.
#[derive(Clone, Copy, Debug)]
pub struct UniformAllocation {
    pub offset: u64,
    pub size: u64,
}

/// A material entry ready to be packed into the uniform pool. Produced by
/// the caller when invoking [`MaterialUniformPool::build`].
pub struct MaterialUniformEntry<'a> {
    pub handle: Handle<Material>,
    pub material: &'a Material,
    pub template: &'a MaterialTemplate,
}

/// The shared, persistent uniform buffer for all material uniforms.
///
/// The buffer is built in a **batch** from an iterator of materials: the
/// caller passes every material that should be in the buffer, the pool packs
/// each uniform binding (in `template.uniform_layout()` order) into the
/// shared `wgpu::Buffer` at its own aligned offset, and records the
/// `(offset, size)` per `(material handle, binding slot)`.
///
/// The caller controls when to (re)build:
/// - First frame, or after materials were added/removed → call `build`.
/// - Subsequent frames with no material changes → reuse the cached buffer
///   via [`binding_resource`] / [`buffer`].
///
/// The pool does not grow dynamically in V1 — it sizes the buffer to exactly
/// fit the materials passed to `build`. If the material set changes, `build`
/// is called again with the new full set, which recreates the buffer.
pub struct MaterialUniformPool {
    buffer: Option<wgpu::Buffer>,
    /// Per-uniform allocations, keyed by `(material handle, binding slot)`.
    allocations: HashMap<(Handle<Material>, u32), UniformAllocation>,
}

impl MaterialUniformPool {
    pub fn new() -> Self {
        Self {
            buffer: None,
            allocations: HashMap::new(),
        }
    }

    /// Builds the shared uniform buffer from the given materials.
    ///
    /// Each uniform binding in a material's template layout gets its own
    /// aligned slot in the shared buffer. The material must provide a
    /// `UniformValue` of the declared type for every binding (validated at
    /// load time by `MaterialLoader`, so this is infallible at the packing
    /// stage).
    ///
    /// The caller should call this when the material set has changed
    /// (materials added/removed). Otherwise, reuse the cached buffer via
    /// [`binding_resource`] / [`buffer`].
    pub fn build<'a, I>(
        &mut self,
        device: &wgpu::Device,
        materials: I,
    ) where
        I: IntoIterator<Item = MaterialUniformEntry<'a>>,
    {
        self.allocations.clear();

        // GPU uniform-buffer offsets must respect
        // `min_uniform_buffer_offset_alignment` (typically 256). Each uniform
        // binding gets its own aligned offset, so every BindGroupEntry that
        // references the shared buffer is correctly aligned.
        let align = device
            .limits()
            .min_uniform_buffer_offset_alignment
            .max(1) as u64;

        // First pass: assign each uniform binding its own aligned offset and
        // collect the values to write.
        let mut cursor: u64 = 0;
        // (material handle, binding slot, offset, value)
        let mut pending: Vec<(Handle<Material>, u32, u64, UniformValue)> = Vec::new();

        for entry in materials {
            let layout = entry.template.uniform_layout();
            let material_uniforms = entry.material.uniforms();

            for binding in layout {
                let value = material_uniforms
                    .get(&binding.name)
                    .copied()
                    .expect("MaterialLoader validation guarantees every declared uniform is provided");
                debug_assert_eq!(value.ty(), binding.ty, "type mismatch — validation should have caught this");

                let size = value.ty().size();
                cursor = cursor.next_multiple_of(align);
                let offset = cursor;

                self.allocations.insert(
                    (entry.handle, binding.binding_slot),
                    UniformAllocation { offset, size },
                );
                pending.push((entry.handle, binding.binding_slot, offset, value));
                cursor += size;
            }
        }

        // Pad the total to the alignment so the buffer size is a clean
        // multiple (avoids trailing-misalignment issues on some drivers).
        let total_size = cursor.next_multiple_of(align);

        // Second pass: write each uniform's bytes at its assigned offset.
        let mut bytes = vec![0u8; total_size as usize];
        for (_handle, _slot, offset, value) in pending {
            value.write_bytes(&mut bytes, offset as usize);
        }

        self.buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Material uniform pool buffer"),
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        }));
    }

    /// Returns the cached GPU buffer, if built.
    pub fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Returns the per-uniform allocation for a `(material, binding_slot)`
    /// pair, if the pool was built with it.
    pub fn allocation(&self, handle: Handle<Material>, binding_slot: u32) -> Option<UniformAllocation> {
        self.allocations.get(&(handle, binding_slot)).copied()
    }

    /// Builds a [`wgpu::BindingResource`] for a single uniform binding,
    /// referencing the shared buffer at the binding's aligned offset. This is
    /// what `BindGroupAllocator` uses when creating a material's bind group —
    /// it asks the pool for each binding's resource by `(handle, slot)` and
    /// never computes offsets itself.
    ///
    /// # Panics
    /// Panics if the pool has not been built or the `(material, slot)` pair
    /// was not included in the last `build` call.
    pub fn binding_resource<'a>(
        &'a self,
        handle: Handle<Material>,
        binding_slot: u32,
    ) -> wgpu::BindingResource<'a> {
        let buffer = self.buffer()
            .expect("MaterialUniformPool buffer has not been built — call build first");
        let allocation = self.allocation(handle, binding_slot)
            .expect("MaterialUniformPool has no allocation for this (material, binding slot) — call build first");
        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: allocation.offset,
            size: Some(std::num::NonZeroU64::new(allocation.size).unwrap()),
        })
    }

    /// Whether the pool currently holds a built buffer.
    pub fn is_built(&self) -> bool {
        self.buffer.is_some()
    }

    /// Clears the pool (buffer + allocations). The next `build` will recreate.
    pub fn clear(&mut self) {
        self.buffer = None;
        self.allocations.clear();
    }
}