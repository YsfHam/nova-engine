use std::collections::HashMap;

use crate::{
    assets::{handle::Handle, resolve::ResolvedMaterial},
    graphics::{
        material::Material,
        uniform::{MaterialUniformEntry, MaterialUniformPool},
    },
};

// ──────────────────────────────────────────────────────────────────────────
//  BindGroupAllocator — caches per-material bind groups (group 1).
//
//  The allocator owns the [`MaterialUniformPool`] (the shared, persistent
//  buffer for all material uniforms) and a `HashMap<Handle<Material>,
//  wgpu::BindGroup>` cache. Materials are immutable, so their bind groups
//  are built once and never invalidated — a perfect cache.
//
//  Flow:
//  1. The caller (the entity setting up the render pass) calls
//     [`build_uniform_pool`] with every material that will be drawn this
//     frame, when it decides the pool needs (re)building. This packs all
//     uniform values into one `wgpu::Buffer` and records each material's
//     `(offset, size)`.
//  2. For each draw, the caller calls [`get_or_create`] with the material
//     handle, the resolved textures, and the material bind group layout.
//     The allocator looks up the cached bind group, or builds it from the
//     pool's buffer (at the material's offset) + the texture views/samplers.
// ──────────────────────────────────────────────────────────────────────────

/// A resolved texture ready for bind group entry creation: a texture view +
/// its sampler (both already resolved from handles by the caller).
pub struct ResolvedTextureBinding<'a> {
    pub view: &'a wgpu::TextureView,
    pub sampler: &'a wgpu::Sampler,
}

pub struct BindGroupAllocator {
    uniform_pool: MaterialUniformPool,
    bind_groups: HashMap<Handle<Material>, wgpu::BindGroup>,
}

impl BindGroupAllocator {
    pub fn new() -> Self {
        Self {
            uniform_pool: MaterialUniformPool::new(),
            bind_groups: HashMap::new(),
        }
    }

    /// (Re)builds the shared material uniform buffer from the given materials.
    ///
    /// The caller decides when to call this: on the first frame, or after
    /// materials have been added/removed. On subsequent frames with no
    /// material changes, the cached buffer is reused (accessed via
    /// [`uniform_pool`](Self::uniform_pool) or [`get_or_create`]).
    ///
    /// This also clears the bind group cache, since offsets may have changed.
    pub fn build_uniform_pool<'a, I>(
        &mut self,
        device: &wgpu::Device,
        materials: I,
    ) where
        I: IntoIterator<Item = MaterialUniformEntry<'a>>,
    {
        self.uniform_pool.build(device, materials);
        self.bind_groups.clear();
    }

    /// Returns the cached bind group for `material_handle`, or builds + caches
    /// it from the uniform pool buffer + the resolved textures.
    ///
    /// - The template's uniform layout is iterated; for each binding the pool
    ///   is asked for a `BindingResource` by `(material handle, binding slot)`.
    ///   The pool owns per-uniform allocations and offset alignment — the
    ///   allocator does no offset math.
    /// - `texture_bindings`: the template's texture layout (binding slots).
    /// - `resolved_textures`: a map from texture binding slot to the resolved
    ///   `ResolvedTextureBinding` (view + sampler). The caller resolves these
    ///   from `Handle<Texture>` / `Handle<Sampler>` via `AssetsManager`.
    /// - `material_bind_group_layout`: from the pipeline (group 1 layout).
    ///
    /// # Panics
    /// Panics if the uniform pool has not been built or if the material was
    /// not included in the last `build_uniform_pool` call.
    pub fn get_or_create(
        &mut self,
        device: &wgpu::Device,
        material: ResolvedMaterial<'_>,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> &wgpu::BindGroup {
        if !self.bind_groups.contains_key(&material.handle) {
            let bind_group = self.create_bind_group(
                device,
                &material,
                material_bind_group_layout,
            );
            self.bind_groups.insert(material.handle, bind_group);
        }
        self.bind_groups.get(&material.handle).unwrap()
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        material: &ResolvedMaterial<'_>,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        let material_handle = material.handle;

        let template = material.material_template.material_template;
        let uniform_bindings = template.uniform_layout();
        let texture_bindings = template.texture_layout();

        let mut entries = Vec::new();

        // Per-uniform buffer entries. The pool owns per-uniform allocations
        // keyed by (material handle, binding slot); we simply ask it for each
        // binding's `BindingResource`. The pool handles offset alignment to
        // `min_uniform_buffer_offset_alignment` — no offset math here.
        for binding in uniform_bindings {
            entries.push(wgpu::BindGroupEntry {
                binding: binding.binding_slot,
                resource: self.uniform_pool.binding_resource(material_handle, binding.binding_slot),
            });
        }

        // Texture + sampler entries.
        for tex_binding in texture_bindings {
            let resolved = material.textures
                .get(&tex_binding.texture_binding_slot)
                .unwrap_or_else(|| {
                    panic!("texture binding slot {} not resolved for material {:?}", tex_binding.texture_binding_slot, material_handle)
                });

            entries.push(wgpu::BindGroupEntry {
                binding: tex_binding.texture_binding_slot,
                resource: wgpu::BindingResource::TextureView(resolved.texture.view()),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: tex_binding.sample_binding_slot,
                resource: wgpu::BindingResource::Sampler(resolved.sampler.sampler()),
            });
        }

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material bind group"),
            layout: material_bind_group_layout,
            entries: &entries,
        })
    }

    /// Access the underlying uniform pool (e.g., to check `is_built`).
    pub fn uniform_pool(&self) -> &MaterialUniformPool {
        &self.uniform_pool
    }

    /// Mutable access to the underlying uniform pool (e.g., to `clear`).
    pub fn uniform_pool_mut(&mut self) -> &mut MaterialUniformPool {
        &mut self.uniform_pool
    }

    /// Whether the uniform pool has been built.
    pub fn is_uniform_pool_built(&self) -> bool {
        self.uniform_pool.is_built()
    }

    /// Clears the bind group cache (keeps the uniform pool). Call if
    /// bind groups need recreation but the buffer is still valid.
    pub fn clear_bind_groups(&mut self) {
        self.bind_groups.clear();
    }
}