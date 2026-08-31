use std::collections::HashMap;

use wgpu::CommandEncoder;

use crate::{
    assets::{handle::Handle, resolve::ResolvedMaterial, AssetsManager},
    graphics::{
        material::Material,
        uniform::{MaterialUniformEntry, MaterialUniformPool},
    },
};

// ──────────────────────────────────────────────────────────────────────────
//  BindGroupAllocator — caches per-material bind groups (group 1).
//
//  The allocator owns the [`MaterialUniformPool`] (the shared, persistent
//  `DynamicBuffer` for all material uniforms) and a `HashMap<Handle<Material>,
//  wgpu::BindGroup>` cache. Materials are immutable, so their bind groups
//  are built once and never invalidated — a perfect cache.
//
//  The uniform pool **extends** (appends new materials) rather than rebuilding
//  from scratch every frame. A full rebuild only happens when a material is
//  detected as removed from the asset storage.
//
//  Flow (called from `submit_batches`):
//  1. [`detect_removed`] — checks if any tracked material handle no longer
//     exists in the `AssetsManager`. If so, [`rebuild`] is called with the
//     full current material set, which clears and re-packs everything.
//  2. [`extend`] — appends only the **new** materials (those not already
//     tracked). Returns `true` if anything was added.
//  3. [`build_bind_groups`] — creates bind groups for the newly added
//     materials (those that don't have a cached bind group yet).
//  4. For each draw, [`get_bind_group`] returns the cached bind group.
// ──────────────────────────────────────────────────────────────────────────

pub struct BindGroupAllocator {
    uniform_pool: MaterialUniformPool,
    bind_groups: HashMap<Handle<Material>, wgpu::BindGroup>,
}

impl BindGroupAllocator {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            uniform_pool: MaterialUniformPool::new(device),
            bind_groups: HashMap::new(),
        }
    }

    /// Returns `true` if any tracked material handle no longer exists in the
    /// given `AssetsManager` — i.e. the material was removed from the asset
    /// storage. The caller should call [`rebuild`](Self::rebuild) when this
    /// returns `true`.
    pub(crate) fn detect_removed(&self, assets: &AssetsManager) -> bool {
        self.uniform_pool.detect_removed(assets)
    }

    /// Full rebuild: clears all uniform allocations and bind groups, then
    /// re-packs every material from the given entries. Use when a material
    /// was removed from the asset storage.
    pub(crate) fn rebuild<'a, I>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut CommandEncoder,
        materials: I,
    ) where
        I: IntoIterator<Item = MaterialUniformEntry<'a>>,
    {
        self.uniform_pool.rebuild(device, queue, encoder, materials);
        self.bind_groups.clear();
    }

    /// Appends new materials (those not already tracked) to the uniform
    /// buffer. Returns `true` if any new materials were added.
    pub(crate) fn extend<'a, I>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut CommandEncoder,
        materials: I,
    ) -> bool
    where
        I: IntoIterator<Item = MaterialUniformEntry<'a>>,
    {
        self.uniform_pool.extend(device, queue, encoder, materials)
    }

    pub(crate) fn get_or_build_bind_group(
        &mut self,
        device: &wgpu::Device,
        material: &ResolvedMaterial<'_>,
        layout: &wgpu::BindGroupLayout
    ) -> &wgpu::BindGroup {
        if !self.bind_groups.contains_key(&material.handle) {
            let bg = self.create_bind_group(device, &material, layout);
            self.bind_groups.insert(material.handle, bg);
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
        // binding's `BindingResource`.
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

    /// Access the underlying uniform pool.
    pub(crate) fn uniform_pool(&self) -> &MaterialUniformPool {
        &self.uniform_pool
    }

    /// Whether the uniform pool has at least one material allocated.
    pub fn is_uniform_pool_built(&self) -> bool {
        self.uniform_pool.is_built()
    }

    /// Clears the bind group cache (keeps the uniform pool).
    pub fn clear_bind_groups(&mut self) {
        self.bind_groups.clear();
    }
}