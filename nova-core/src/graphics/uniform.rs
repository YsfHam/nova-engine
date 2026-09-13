use std::num::NonZeroU64;

use glam::{Mat4, Vec4};
use wgpu::util::DeviceExt;

// ──────────────────────────────────────────────────────────────────────────
//  Uniform types — declaration and values.
//
//  These are the engine-native primitives shared across the uniform system.
//  `material.rs` imports them; they live here so there is one place to
//  understand the uniform value model.
// ──────────────────────────────────────────────────────────────────────────

/// Declares a single uniform binding as a `MaterialTemplate`'s shaders expect
/// it. Used for documentation / layout derivation.
#[derive(Clone, Debug)]
pub struct UniformBinding {
    pub name: String,
    pub ty: UniformType,
    pub binding_slot: u32,
    pub visibility: crate::graphics::shader::ShaderStage,
}

/// The type of a uniform value. Grows as new shaders need more types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// A runtime uniform value. Kept as a typed enum so materials can pack
/// values into a uniform buffer without the caller worrying about layout.
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
        let as_bytes = self.as_bytes();
        let size = self.ty().size();

        bytes[offset..offset+size as usize].copy_from_slice(&as_bytes);
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            UniformValue::Mat4(m) => {
                let cols = m.to_cols_array();
                bytemuck::cast_slice(&cols).to_vec()
            }
            UniformValue::Vec4(v) => {
                let arr = v.to_array();
                bytemuck::cast_slice(&arr).to_vec()
            }
            UniformValue::F32(x) => {
                bytemuck::cast_slice(&[*x]).to_vec()
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  UniformBuffer — per-frame transient uploads with GPU alignment.
//
//  Scene globals (camera projection/view, time, lighting) are reconstructed
//  every frame from runtime state. They do NOT belong in `Material` (which is
//  immutable, asset-owned, serializable). `UniformBuffer` keeps typed staging
//  entries, builds one aligned GPU buffer, and exposes per-binding offsets so
//  callers can create bind groups from the buffer + offsets.
//
//  Both scene (group 0) and material (group 1) uniforms use this type,
//  eliminating duplicated alignment logic.
// ──────────────────────────────────────────────────────────────────────────

/// A typed staging entry held by [`UniformBuffer`] until the buffer is built.
struct UniformBufferEntry {
    binding_slot: u32,
    offset: u64,
    size: u64,
    value: UniformValue,
}

/// Per-frame transient uniform storage with GPU-aligned buffer creation.
/// Used for both scene globals and per-material uniforms. Call `upload` for
/// each uniform value, then `build` to create the GPU buffer, then `offset`
/// to get each binding's aligned offset for bind group construction.
/// Reset each frame via `reset`.
pub struct UniformBuffer {
    entries: Vec<UniformBufferEntry>,
    buffer: Option<wgpu::Buffer>,
    min_offset_alignment: u64,
}

impl UniformBuffer {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            buffer: None,
            min_offset_alignment: 1,
        }
    }

    /// Uploads a uniform value at the given binding slot. If a value for this
    /// slot was already uploaded, replaces it in place.
    pub fn upload(&mut self, binding_slot: u32, value: UniformValue) {
        let size = value.ty().size();

        if let Some(existing) = self.entries.iter_mut().find(|e| e.binding_slot == binding_slot) {
            existing.value = value;
            self.buffer = None;
            return;
        }

        self.entries.push(UniformBufferEntry {
            binding_slot,
            offset: 0,
            size,
            value,
        });
        self.buffer = None;
    }

    /// Creates the GPU buffer with aligned offsets. Stores the computed offset
    /// in each entry. Does nothing if no uniforms have been uploaded.
    pub fn build(&mut self, device: &wgpu::Device) {
        if self.entries.is_empty() {
            return;
        }

        self.min_offset_alignment = device
            .limits()
            .min_uniform_buffer_offset_alignment
            .max(1) as u64;
        let align = self.min_offset_alignment;

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

        self.buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform buffer"),
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        }));
    }

    /// Returns the aligned offset for a binding slot, or `None` if not uploaded.
    pub fn offset(&self, binding_slot: u32) -> Option<u64> {
        self.entries
            .iter()
            .find(|e| e.binding_slot == binding_slot)
            .map(|e| e.offset)
    }

    /// Returns the GPU buffer, or `None` if `build` hasn't been called or
    /// no uniforms were uploaded.
    pub fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Builds the GPU buffer (if not already built) and creates a bind group
    /// using the given layout. Convenience for scene (group 0) uniforms.
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
            self.build(device);
        }

        let buffer = self.buffer.as_ref()?;
        let entries: Vec<wgpu::BindGroupEntry> = self
            .entries
            .iter()
            .map(|entry| wgpu::BindGroupEntry {
                binding: entry.binding_slot,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: entry.offset,
                    size: NonZeroU64::new(entry.size),
                }),
            })
            .collect();

        Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform bind group"),
            layout,
            entries: &entries,
        }))
    }

    /// Resets the buffer for a new frame.
    pub fn reset(&mut self) {
        self.entries.clear();
        self.buffer = None;
    }
}