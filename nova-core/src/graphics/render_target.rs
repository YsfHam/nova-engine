use std::cell::RefMut;

use crate::{
    assets::{resolve::ResolvedMaterial, AssetsManager},
    graphics::{
        bind::BindGroupAllocator,
        buffer::{Offset, StagingBufferPool},
        draw_batch::DrawBatch,
        geometry::GeometryPool,
        environment::EnvironmentDescriptor,
        pipeline::{PipelineCache, PipelineDescriptor},
        render::RenderContext,
        render_pass::{IndexFormat, RenderPass, RenderPassDescriptor},
        texture::TextureFormat,
        uniform::{MaterialUniformEntry, UniformArena},
    },
};

/// A view-agnostic render target. Owns a command encoder and a per-target
/// [`UniformArena`], and holds a `RefMut<RenderContext>` guard (obtained from
/// [`RenderContextRef::get_mut`](crate::graphics::render::RenderContextRef::get_mut))
/// for direct access to the pipeline cache and bind group allocator.
///
/// `RenderTarget` works for on-screen rendering (the surface view from
/// [`Frame`](crate::graphics::frame::Frame)) and off-screen rendering (any
/// arbitrary `TextureView`, e.g. a `TextureRenderTarget`).
/// To submit recorded commands, call [`submit`](Self::submit); the caller is
/// responsible for presenting the surface (via
/// [`Frame::present`](crate::graphics::frame::Frame::present)).
///
/// Because `RenderTarget` holds the `RefMut` guard, methods on
/// [`RenderTargetCommander`] can return references whose lifetime is tied to
/// the `RenderTarget` borrow — no nested `RefCell` borrows are needed.
pub struct RenderTarget<'a> {
    /// The mutable borrow of `RenderContext`, held for the target's lifetime.
    /// Gives direct field access (device, pipeline_cache, bind_group_allocator).
    render_ctx: RefMut<'a, RenderContext>,
    pub(crate) view: &'a wgpu::TextureView,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    uniform_arena: UniformArena,
    scene_bind_group_layout: Option<wgpu::BindGroupLayout>,
}

impl<'a> RenderTarget<'a> {
    /// Creates a render target rendering into `view`, holding a mutable borrow
    /// of the `RenderContext` (via the `RefMut` guard from `render_ctx_ref`).
    ///
    /// The caller passes a `RefMut<RenderContext>` obtained from
    /// [`RenderContextRef::get_mut`](crate::graphics::render::RenderContextRef::get_mut).
    pub fn new(render_ctx: RefMut<'a, RenderContext>, view: &'a wgpu::TextureView) -> Self {
        let encoder = render_ctx.device().create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("RenderTarget encoder"),
            },
        );

        Self {
            render_ctx,
            view,
            encoder: Some(encoder),
            uniform_arena: UniformArena::new(),
            scene_bind_group_layout: None,
        }
    }

    /// Creates a `RenderTargetCommander` bound to this target, configured with
    /// the given environment (scene uniforms). The commander borrows the
    /// target's fields and records all draw commands.
    pub fn commander(&mut self, environment: EnvironmentDescriptor) -> RenderTargetCommander<'_> {
        self.set_environment(environment);

        // Deref the RefMut guard to get &mut RenderContext, then borrow its
        // disjoint fields individually. The guard stays alive on self.
        let render_ctx: &mut RenderContext = &mut self.render_ctx;
        RenderTargetCommander {
            device: &render_ctx.gfx.device,
            surface_format: render_ctx.surface_format(),
            pipeline_cache: &mut render_ctx.pipeline_cache,
            bind_group_allocator: &mut render_ctx.bind_group_allocator,
            scene_bind_group_layout: self.scene_bind_group_layout.as_ref().unwrap(),
            uniform_arena: &mut self.uniform_arena,
            encoder: self.encoder.as_mut().expect("Encoder must be Some"),
            view: self.view,
            staging_buffer: &mut render_ctx.staging_buffer_pool,
            geometry_pool: &render_ctx.geometry_pool,
            queue: &render_ctx.gfx.queue
        }
    }

    fn set_environment(&mut self, environment: EnvironmentDescriptor) {
        let mut entries = vec![];
        for uniform in environment.uniforms() {
            let entry = wgpu::BindGroupLayoutEntry {
                binding: uniform.binding_slot,
                visibility: uniform.visibilty.into(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            };

            self.uniform_arena.upload(uniform.binding_slot, uniform.uniform);
            entries.push(entry);
        }

        self.scene_bind_group_layout = Some(self.render_ctx.device().create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Scene bind group layout"),
            entries: &entries
        }));
    }

    /// Finishes the command encoder and submits it to the GPU queue.
    ///
    /// This does **not** present the surface — that is the caller's
    /// responsibility (via [`Frame::present`](crate::graphics::frame::Frame::present))
    /// for on-screen rendering.
    fn submit(&mut self) {
        if let Some(encoder) = self.encoder.take() {
            self.render_ctx.submit_command_encoder(encoder);
        }
    }
}


impl<'a> Drop for RenderTarget<'a> {
    fn drop(&mut self) {
        self.submit();
    }
} 

/// An off-screen render target backed by a texture. Owns a `wgpu::Texture`
/// and its `TextureView`, and produces a [`RenderTarget`] that renders into
/// that view.
///
/// This is the "do whatever you want" off-screen target: the caller creates
/// it (either via [`RenderContext::create_texture_target`](crate::graphics::render::RenderContext) or
/// directly), uses the [`RenderTarget`] to record commands, submits, and then
/// reads the texture (e.g. as a sampling source in a subsequent pass).
///
/// The texture is owned by this struct so it lives as long as needed.
/// [`RenderTarget::submit`] records into the command queue; the texture's
/// contents are available after GPU completion.
pub struct TextureRenderTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl TextureRenderTarget {
    /// Creates a new texture render target with the given dimensions and
    /// format. The texture is created with `RENDER_ATTACHMENT` +
    /// `TEXTURE_BINDING` usage so it can be both rendered into and sampled.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: TextureFormat,
        label: Option<&str>,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format.into(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, view }
    }

    /// The underlying texture (e.g. to create additional views or sample it).
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// The default texture view that the [`RenderTarget`] renders into.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Creates a [`RenderTarget`] that renders into this texture's view,
    /// holding the given `RefMut<RenderContext>` guard for its lifetime.
    pub fn as_render_target<'a>(
        &'a self,
        render_ctx: RefMut<'a, RenderContext>,
    ) -> RenderTarget<'a> {
        RenderTarget::new(render_ctx, &self.view)
    }
}


/// A command-recording scope bound to a [`RenderTarget`]. Created via
/// [`RenderTarget::commander`], it borrows the target's encoder, uniform
/// arena, and the `RenderContext`'s split-borrowed fields (device,
/// pipeline_cache, bind_group_allocator) to record draw commands.
///
/// The commander is the primary draw API: it compiles pipelines, builds bind
/// groups, and records passes. Because the fields are split-borrowed from
/// the `RefMut<RenderContext>` guard at creation time, all field access is
/// plain `&`/`&mut` — no nested `RefCell` borrows, and disjoint fields
/// (pipeline_cache vs bind_group_allocator vs encoder) coexist freely.
pub struct RenderTargetCommander<'a> {
    device: &'a wgpu::Device,
    surface_format: wgpu::TextureFormat,
    pipeline_cache: &'a mut PipelineCache,
    bind_group_allocator: &'a mut BindGroupAllocator,
    scene_bind_group_layout: &'a wgpu::BindGroupLayout,
    uniform_arena: &'a mut UniformArena,
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    staging_buffer: &'a mut StagingBufferPool,
    geometry_pool: &'a GeometryPool,
    queue: &'a wgpu::Queue,
}

// ──────────────────────────────────────────────────────────────────────────
//  PreparedBatch — the single struct produced by consuming the batch
//  iterator once. Contains the original batch (for geometry), the resolved
//  material, and the staging offsets (which may point into the dynamic
//  staging buffer or the persistent shared geometry buffer).
// ──────────────────────────────────────────────────────────────────────────

/// Which GPU buffer an offset points into.
#[derive(Clone, Copy, Debug)]
enum BufferSource {
    /// Offset into the ring-buffered dynamic staging buffer (per-frame data).
    Dynamic(Offset),
    /// Offset into the persistent shared geometry buffer (uploaded once).
    Shared(Offset),
}

/// A batch that has been fully resolved: material resolved, geometry uploaded
/// (or looked up from the shared pool), and offsets recorded.
struct PreparedBatch<'a> {
    batch: DrawBatch,
    resolved: ResolvedMaterial<'a>,
    staging_offsets: BatchStagingBufferOffsets,
}

impl<'a> PreparedBatch<'a> {

    /// Builds a `MaterialUniformEntry` from this prepared batch's resolved
    /// material. Cheap — all fields are references.
    fn uniform_entry(&self) -> MaterialUniformEntry<'a> {
        MaterialUniformEntry {
            handle: self.batch.material,
            material: self.resolved.material,
            template: self.resolved.material_template.material_template,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  submit_batches — the main entry point. Delegates each phase to a
//  dedicated function. The batch iterator is consumed exactly once (in
//  `prepare_batches`). All downstream phases borrow the resulting
//  `Vec<PreparedBatch>` and use iterator combinators — no additional
//  heap allocations beyond the single Vec.
// ──────────────────────────────────────────────────────────────────────────

impl<'a> RenderTargetCommander<'a> {
    pub fn submit_batches<I>(
        self,
        pass_descriptor: RenderPassDescriptor,
        batches: I,
        assets: &AssetsManager,
    )
    where
        I: IntoIterator<Item = DrawBatch>,
    {
        // Destructure self into individual field references. This lets each
        // phase function borrow only the fields it needs, and avoids the
        // "borrow self mutably twice" problem. The `prepared` Vec borrows
        // `assets` (via ResolvedMaterial), not `self` — so it can coexist
        // with mutable field access.
        let Self {
            device,
            surface_format,
            pipeline_cache,
            bind_group_allocator,
            scene_bind_group_layout,
            uniform_arena,
            encoder,
            view,
            staging_buffer,
            geometry_pool,
            queue,
        } = self;

        // Phase 1: scene bind group (group 0).
        let scene_bind_group = uniform_arena
            .build_bind_group(device, scene_bind_group_layout)
            .expect("scene uniforms uploaded");

        // Phase 2: detect removed materials.
        let needs_rebuild = bind_group_allocator.detect_removed(assets);

        // Phase 3: consume the iterator once — resolve materials, upload
        // geometry (owned → dynamic staging; shared → look up permanent
        // offsets from the geometry pool). Produces a single Vec<PreparedBatch>.
        let prepared = prepare_batches(
            batches,
            assets,
            staging_buffer,
            geometry_pool,
            device,
            queue,
            encoder,
        );
        if prepared.is_empty() {
            return;
        }

        // Phase 4: extend or rebuild the uniform pool.
        update_uniform_pool(
            &prepared,
            needs_rebuild,
            bind_group_allocator,
            device,
            queue,
            encoder,
        );

        // Phase 6: record the render pass.
        record_pass(
            pass_descriptor,
            &scene_bind_group,
            &scene_bind_group_layout,
            surface_format,
            &prepared,
            staging_buffer,
            geometry_pool,
            bind_group_allocator,
            encoder,
            view,
            pipeline_cache,
            device,
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  Free functions — each phase of submit_batches. Taking individual field
//  references (not &mut self) avoids borrow conflicts and makes the data
//  flow explicit.
// ──────────────────────────────────────────────────────────────────────────

/// Phase 3: Consumes the batch iterator **once** into a `Vec<PreparedBatch>`.
///
/// For each batch: uploads geometry to the staging buffer (recording offsets),
/// resolves the material handle, and compiles/fetches the pipeline. Batches
/// whose material can't be resolved are silently skipped via `filter_map`.
///
/// The returned `PreparedBatch` items borrow `assets` (through
/// `ResolvedMaterial`) — not any of the commander fields — so the result
/// can coexist with mutable access to `bind_group_allocator` etc.
fn prepare_batches<'a>(
    batches: impl IntoIterator<Item = DrawBatch>,
    assets: &'a AssetsManager,
    staging_buffer: &mut StagingBufferPool,
    geometry_pool: &GeometryPool,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
) -> Vec<PreparedBatch<'a>> {
    batches
        .into_iter()
        .filter_map(|batch| {
            // Geometry: owned → upload to dynamic staging; shared → look up
            // permanent offsets from the geometry pool (no upload).
            let (vertex_offset, index_offset) = if let Some(geo_ref) = batch.shared_geometry() {
                // Shared geometry: offsets are permanent, no upload.
                let (v, i) = geometry_pool.offsets(geo_ref)?;
                (BufferSource::Shared(v), BufferSource::Shared(i))
            } else {
                // Owned geometry: upload to the dynamic staging buffer.
                let vertices = batch.vertices()?;
                let indices = batch.indices()?;
                let v = staging_buffer.upload(vertices, device, queue, encoder);
                let i = staging_buffer.upload(bytemuck::cast_slice(indices), device, queue, encoder);
                (BufferSource::Dynamic(v), BufferSource::Dynamic(i))
            };

            // Instance data is always dynamic (per-frame).
            let instance_offset = batch
                .instances()
                .map(|inst| staging_buffer.upload(inst, device, queue, encoder));

            // Resolve material — skip if the handle is stale.
            let resolved = ResolvedMaterial::new(batch.material, assets).ok()?;

            Some(PreparedBatch {
                batch,
                resolved,
                staging_offsets: BatchStagingBufferOffsets {
                    vertex_offset,
                    index_offset,
                    instance_offset,
                },
            })
        })
        .collect()
}

/// Phase 4: Extends the uniform pool with new materials, or rebuilds it
/// entirely if `needs_rebuild` is true (a tracked material was removed from
/// the asset storage).
///
/// When extending, only materials not already tracked are passed — the
/// `has_material` check filters them. When rebuilding, all materials are
/// passed. No intermediate `Vec` is allocated: the iterator is consumed
/// directly by `extend`/`rebuild`.
fn update_uniform_pool(
    prepared: &[PreparedBatch<'_>],
    needs_rebuild: bool,
    bind_group_allocator: &mut BindGroupAllocator,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
) {
    if needs_rebuild {
        bind_group_allocator.rebuild(
            device,
            queue,
            encoder,
            prepared.iter().map(|p| p.uniform_entry()),
        );
        bind_group_allocator.clear_bind_groups();
    } else {
        // Collect the handles of new materials first, so the iterator passed
        // to `extend` doesn't hold an immutable borrow of `bind_group_allocator`
        // while `extend` needs `&mut`.
        let new_entries: Vec<MaterialUniformEntry> = prepared
            .iter()
            .filter(|p| !bind_group_allocator.uniform_pool().has_material(p.batch.material))
            .map(|p| p.uniform_entry())
            .collect();
        bind_group_allocator.extend(device, queue, encoder, new_entries);
    }
}


fn record_pass(
    pass_descriptor: RenderPassDescriptor,
    scene_bind_group: &wgpu::BindGroup,
    scene_bind_group_layout: &wgpu::BindGroupLayout,
    target_format: wgpu::TextureFormat,
    prepared: &[PreparedBatch<'_>],
    staging_buffer: &mut StagingBufferPool,
    geometry_pool: &GeometryPool,
    bind_group_allocator: &mut BindGroupAllocator,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    piepline_cache: &mut PipelineCache,
    device: &wgpu::Device,
) {
    let dynamic_buffer = staging_buffer.swap_buffers();
    let shared_buffer = geometry_pool.buffer();

    let mut pass = RenderPass::new(encoder, view, pass_descriptor);
    pass.set_bind_group(0, scene_bind_group, &[]);

    let mut old_template_handle = None;

    for p in prepared {
        let template_handle = p.resolved.material_template.handle;
        let pipeline = piepline_cache.get_or_compile(device, PipelineDescriptor {
            material_template: p.resolved.material_template,
            scene_bind_group_layout,
            target_format,
        });
        if old_template_handle.is_none_or(|h| h != template_handle) {
            pass.set_pipeline(pipeline);
            old_template_handle = Some(template_handle);
        };

        if let Some(layout) = pipeline.bind_group_layout.as_ref() {
            let bg = bind_group_allocator.get_or_build_bind_group(device, &p.resolved, layout);
            pass.set_bind_group(1, bg, &[]);
        }

        // Resolve the index count: for owned geometry, it's on the batch;
        // for shared geometry, the batch returns 0 and we need to look up
        // the actual count from the geometry pool's offsets.
        let index_count = match p.staging_offsets.index_offset {
            BufferSource::Shared(o) => (o.size / 2) as u32,
            _ => p.batch.index_count()
        };

        draw_call(
            &mut pass,
            dynamic_buffer,
            shared_buffer,
            &p.staging_offsets,
            index_count,
            p.batch.instance_count(),
        );
    }
}

/// Records a single indexed draw call. The vertex/index buffers may come
/// from the dynamic staging buffer (per-frame data) or the shared geometry
/// buffer (persistent data). Instance data is always dynamic.
fn draw_call(
    pass: &mut RenderPass,
    dynamic_buffer: &wgpu::Buffer,
    shared_buffer: &wgpu::Buffer,
    offsets: &BatchStagingBufferOffsets,
    index_count: u32,
    instance_count: u32,
) {
    let BatchStagingBufferOffsets {
        vertex_offset,
        index_offset,
        instance_offset,
    } = offsets;

    // Vertex buffer: from the right source.
    let vertex_slice = match vertex_offset {
        BufferSource::Dynamic(o) => dynamic_buffer.slice(o.offset..o.offset + o.size),
        BufferSource::Shared(o) => shared_buffer.slice(o.offset..o.offset + o.size),
    };
    pass.set_vertex_buffer(0, vertex_slice);

    // Index buffer: from the right source.
    let index_slice = match index_offset {
        BufferSource::Dynamic(o) => dynamic_buffer.slice(o.offset..o.offset + o.size),
        BufferSource::Shared(o) => shared_buffer.slice(o.offset..o.offset + o.size),
    };
    pass.set_index_buffer(index_slice, IndexFormat::Uint16);

    // Instance buffer: always dynamic (per-frame data).
    if let Some(instance_offset) = instance_offset {
        pass.set_vertex_buffer(
            1,
            dynamic_buffer.slice(instance_offset.offset..instance_offset.offset + instance_offset.size),
        );
    }

    pass.draw_indexed(0..index_count, 0, 0..instance_count);
}

struct BatchStagingBufferOffsets {
    vertex_offset: BufferSource,
    index_offset: BufferSource,
    instance_offset: Option<Offset>,
}