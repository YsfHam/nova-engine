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
    size: u64,
}

impl UniformArena {
    pub fn new() -> Self {
        Self {
            buffer: None,
            entries: Vec::new(),
            size: 0,
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

        let offset = self.size;
        self.size += size;
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

        if self.buffer.is_none() {
            let mut bytes = vec![0u8; self.size as usize];
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
        self.size = 0;
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  MaterialUniformPool — persistent, batch-built buffer for material uniforms.
//
//  All materials share one `wgpu::Buffer`. Each material gets a fixed
//  `(offset, size)` allocation that never moves (materials are immutable, so
//  allocations are stable across builds). The pool is rebuilt from scratch
//  via [`build`], which the caller (the entity setting up the render pass)
//  invokes when it decides the buffer needs invalidation/recreation.
//
//  This pool feeds bind group 1 (material). The `BindGroupAllocator` creates
//  per-material bind groups referencing this buffer at each material's offset.
// ──────────────────────────────────────────────────────────────────────────

/// A fixed allocation within the material uniform pool: the byte offset and
/// size of a material's uniform data inside the shared buffer.
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
/// their uniform values (in `template.uniform_layout()` order), uploads them
/// to one `wgpu::Buffer`, and records each material's `(offset, size)`.
///
/// The caller controls when to (re)build:
/// - First frame, or after materials were added/removed → call `build`.
/// - Subsequent frames with no material changes → call `buffer` /
///   `allocation` to reuse the cached buffer.
///
/// The pool does not grow dynamically in V1 — it sizes the buffer to exactly
/// fit the materials passed to `build`. If the material set changes, `build`
/// is called again with the new full set, which recreates the buffer.
pub struct MaterialUniformPool {
    buffer: Option<wgpu::Buffer>,
    allocations: HashMap<Handle<Material>, UniformAllocation>,
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
    /// Each material's uniforms are packed in `template.uniform_layout()`
    /// order: for each `UniformBinding` in the layout, the material must
    /// provide a `UniformValue` of the declared type (validated at load time
    /// by `MaterialLoader`, so this is infallible at the packing stage).
    ///
    /// The caller should call this when the material set has changed
    /// (materials added/removed). Otherwise, reuse the cached buffer via
    /// [`buffer`] / [`allocation`].
    pub fn build<'a, I>(
        &mut self,
        device: &wgpu::Device,
        materials: I,
    ) where
        I: IntoIterator<Item = MaterialUniformEntry<'a>>,
    {
        self.allocations.clear();

        // First pass: compute total size and each material's offset.
        let mut total_size: u64 = 0;
        let mut pending: Vec<(Handle<Material>, UniformAllocation, Vec<UniformValue>)> = Vec::new();

        for entry in materials {
            let layout = entry.template.uniform_layout();
            let material_uniforms = entry.material.uniforms();

            // Pack uniform values in layout order.
            let mut values = Vec::with_capacity(layout.len());
            let mut material_size: u64 = 0;
            for binding in layout {
                let value = material_uniforms
                    .get(&binding.name)
                    .copied()
                    .expect("MaterialLoader validation guarantees every declared uniform is provided");
                debug_assert_eq!(value.ty(), binding.ty, "type mismatch — validation should have caught this");
                material_size += value.ty().size();
                values.push(value);
            }

            let allocation = UniformAllocation {
                offset: total_size,
                size: material_size,
            };
            total_size += material_size;
            pending.push((entry.handle, allocation, values));
        }

        // Second pass: write bytes and record allocations.
        let mut bytes = vec![0u8; total_size as usize];
        for (handle, allocation, values) in &pending {
            let mut offset = allocation.offset as usize;
            for value in values {
                value.write_bytes(&mut bytes, offset);
                offset += value.ty().size() as usize;
            }
            self.allocations.insert(*handle, *allocation);
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

    /// Returns the allocation (offset + size) for a material, if the pool
    /// was built with it.
    pub fn allocation(&self, handle: Handle<Material>) -> Option<UniformAllocation> {
        self.allocations.get(&handle).copied()
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