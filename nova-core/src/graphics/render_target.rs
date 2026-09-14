use std::{any::TypeId, cell::RefMut};

use crate::{
    assets::AssetsManager, graphics::{
        buffer::{Offset, StagingBufferPool}, draw_batch::DrawBatch, geometry::GeometryPool, material::{BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry}, pipeline::{MaterialRegistration, PipelineCache}, render::{RenderCache, RenderContext, RenderContextRef}, render_pass::{IndexFormat, RenderPass, RenderPassDescriptor}, texture::TextureFormat, uniform::UniformBuffer,
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
    pub(crate) render_ctx: RefMut<'a, RenderContext>,
    pub(crate) view: &'a wgpu::TextureView,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    uniform_buffer: UniformBuffer,
    scene_bind_group_layout: Option<wgpu::BindGroupLayout>,
    scene_bind_group_layout_native: BindGroupLayout,
}

impl<'a> RenderTarget<'a> {
    pub(crate) fn new(render_ctx: RefMut<'a, RenderContext>, view: &'a wgpu::TextureView) -> Self {
        let encoder = render_ctx.device().create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("RenderTarget encoder"),
            },
        );

        Self {
            render_ctx,
            view,
            encoder: Some(encoder),
            uniform_buffer: UniformBuffer::new(),
            scene_bind_group_layout: None,
            scene_bind_group_layout_native: BindGroupLayout::new(),
        }
    }

    /// Creates a `RenderTargetCommander` bound to this target, configured with
    /// the given environment (a `BindGroup` of scene uniforms). The commander
    /// borrows the target's fields and records all draw commands.
    pub fn commander(&mut self, environment: BindGroup) -> RenderTargetCommander<'_> {
        self.set_environment(environment);

        let render_ctx: &mut RenderContext = &mut self.render_ctx;
        RenderTargetCommander {
            surface_format: render_ctx.surface_format(),
            gpu: GpuResources {
                device: &render_ctx.gfx.device,
                queue: &render_ctx.gfx.queue,
                staging_buffer: &mut render_ctx.staging_buffer_pool,
                geometry_pool: &render_ctx.geometry_pool,
                pipeline_cache: &mut render_ctx.pipeline_cache,
                render_cache: &mut render_ctx.render_cache,
            },
            material_registry: &render_ctx.material_registry,
            scene_bind_group_layout: self.scene_bind_group_layout.as_ref().unwrap(),
            scene_bind_group_layout_native: self.scene_bind_group_layout_native.clone(),
            uniform_buffer: &mut self.uniform_buffer,
            encoder: self.encoder.as_mut().expect("Encoder must be Some"),
            view: self.view,
        }
    }

    fn set_environment(&mut self, environment: BindGroup) {
        // Build the scene bind group layout from the environment's entries.
        // We convert the runtime BindGroup entries into a BindGroupLayout
        // (type-only, no values) and create the wgpu layout from it.
        let mut layout = BindGroupLayout::new();
        for entry in &environment.entries {
            match entry {
                BindGroupEntry::Uniform { binding_slot, visibility, value } => {
                    layout = layout.with_entry(BindGroupLayoutEntry::Uniform {
                        binding_slot: *binding_slot,
                        visibility: *visibility,
                        ty: value.ty(),
                    });

                    self.uniform_buffer.upload(*binding_slot, *value);
                }
                BindGroupEntry::Texture { binding_slot, sampler_binding_slot, visibility, view_dimension, sample_type, .. } => {
                    layout = layout.with_entry(BindGroupLayoutEntry::Texture {
                        binding_slot: *binding_slot,
                        sampler_binding_slot: *sampler_binding_slot,
                        visibility: *visibility,
                        view_dimension: *view_dimension,
                        sample_type: *sample_type,
                    });
                }
            }
        }

        // Store the engine-native layout for use as a pipeline cache key.
        self.scene_bind_group_layout_native = layout.clone();

        self.scene_bind_group_layout = layout.create_layout(self.render_ctx.device(), "Scene bind group layout");
        // If the environment has no entries, create an empty layout so the
        // pipeline can still bind group 0.
        if self.scene_bind_group_layout.is_none() {
            self.scene_bind_group_layout = Some(self.render_ctx.device().create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Scene bind group layout (empty)"),
                entries: &[],
            }));
        }
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
    pub(crate) view: wgpu::TextureView,
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

        Self { view }
    }

    /// Creates a [`RenderTarget`] that renders into this texture's view,
    /// holding the given `RefMut<RenderContext>` guard for its lifetime.
    pub fn as_render_target<'a>(
        &'a self,
        render_ctx: &'a RenderContextRef,
    ) -> RenderTarget<'a> {
        RenderTarget::new(render_ctx.get_mut(), &self.view)
    }
}


/// Bundles GPU resources needed by render pass helper functions.
/// Avoids passing many separate arguments to each phase function.
pub(crate) struct GpuResources<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub staging_buffer: &'a mut StagingBufferPool,
    pub geometry_pool: &'a GeometryPool,
    pub pipeline_cache: &'a mut PipelineCache,
    pub render_cache: &'a mut RenderCache,
}

/// Bundles the immutable pass-level context (descriptor, scene bind group,
/// target format, and target view) so `record_pass` doesn't exceed the
/// clippy argument threshold.
pub(crate) struct PassContext<'a> {
    pub pass_descriptor: RenderPassDescriptor,
    pub scene_bind_group: wgpu::BindGroup,
    pub scene_bind_group_layout: &'a wgpu::BindGroupLayout,
    pub scene_layout_native: BindGroupLayout,
    pub target_format: wgpu::TextureFormat,
    pub view: &'a wgpu::TextureView,
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
    surface_format: wgpu::TextureFormat,
    gpu: GpuResources<'a>,
    material_registry: &'a crate::graphics::pipeline::MaterialRegistry,
    scene_bind_group_layout: &'a wgpu::BindGroupLayout,
    scene_bind_group_layout_native: BindGroupLayout,
    uniform_buffer: &'a mut UniformBuffer,
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
}

// ──────────────────────────────────────────────────────────────────────────
//  PreparedBatch — produced by consuming the batch iterator once.
//  Contains the original batch (for geometry), the material registration
//  (template + hash), the owned BindGroup (from as_bind_group()), and
//  staging offsets.
// ──────────────────────────────────────────────────────────────────────────

/// Which GPU buffer an offset points into.
#[derive(Clone, Copy, Debug)]
enum BufferSource {
    /// Offset into the ring-buffered dynamic staging buffer (per-frame data).
    Dynamic(Offset),
    /// Offset into the persistent shared geometry buffer (uploaded once).
    Shared(Offset),
}

/// A batch that has been fully resolved: geometry uploaded (or looked up from
/// the shared pool), material registration looked up, and bind group created.
struct PreparedBatch<'a> {
    batch: DrawBatch,
    registration: &'a MaterialRegistration,
    bind_group: BindGroup,
    staging_offsets: BatchStagingBufferOffsets,
}

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
        let Self {
            surface_format,
            mut gpu,
            material_registry,
            scene_bind_group_layout,
            scene_bind_group_layout_native,
            uniform_buffer,
            encoder,
            view,
        } = self;

        // Phase 1: scene bind group (group 0).
        let scene_bind_group = uniform_buffer
            .build_bind_group(gpu.device, scene_bind_group_layout)
            .expect("scene uniforms uploaded");

        // Phase 2: consume the iterator — resolve materials, upload geometry.
        let prepared = prepare_batches(
            batches,
            assets,
            material_registry,
            &mut gpu,
            encoder,
        );
        if prepared.is_empty() {
            return;
        }

        // Phase 3: record the render pass.
        let pass_ctx = PassContext {
            pass_descriptor,
            scene_bind_group,
            scene_bind_group_layout,
            scene_layout_native: scene_bind_group_layout_native,
            target_format: surface_format,
            view,
        };
        record_pass(
            &pass_ctx,
            &prepared,
            &mut gpu,
            encoder,
            assets,
        );
    }
}

/// Phase 2: Consumes the batch iterator into a `Vec<PreparedBatch>`.
///
/// For each batch: uploads geometry to the staging buffer, looks up the
/// material registration by TypeId, calls `as_bind_group()` to get the
/// owned `BindGroup`. Batches whose material type is not registered or
/// whose asset can't be resolved are silently skipped.
fn prepare_batches<'a>(
    batches: impl IntoIterator<Item = DrawBatch>,
    assets: &'a AssetsManager,
    material_registry: &'a crate::graphics::pipeline::MaterialRegistry,
    gpu: &mut GpuResources<'_>,
    encoder: &mut wgpu::CommandEncoder,
) -> Vec<PreparedBatch<'a>> {
    let GpuResources { device, queue, staging_buffer, geometry_pool, .. } = gpu;

    batches
        .into_iter()
        .filter_map(|batch| {
            // Geometry: owned → upload to dynamic staging; shared → look up
            // permanent offsets from the geometry pool.
            let (vertex_offset, index_offset) = if let Some(geo_ref) = batch.shared_geometry() {
                let (v, i) = geometry_pool.offsets(geo_ref)?;
                (BufferSource::Shared(v), BufferSource::Shared(i))
            } else {
                let vertices = batch.vertices()?;
                let indices = batch.indices()?;
                let v = staging_buffer.upload(vertices, device, queue, encoder);
                let i = staging_buffer.upload(bytemuck::cast_slice(indices), device, queue, encoder);
                (BufferSource::Dynamic(v), BufferSource::Dynamic(i))
            };

            let instance_offset = batch
                .instances()
                .map(|inst| staging_buffer.upload(inst, device, queue, encoder));

            // Look up material registration by TypeId.
            let registration = material_registry.get(&batch.material.type_id)?;

            // Resolve the material asset and call as_bind_group().
            let as_bind_group = (registration.resolve)(batch.material, assets)?;
            let bind_group = as_bind_group.as_bind_group();

            Some(PreparedBatch {
                batch,
                registration,
                bind_group,
                staging_offsets: BatchStagingBufferOffsets {
                    vertex_offset,
                    index_offset,
                    instance_offset,
                },
            })
        })
        .collect()
}

fn record_pass(
    pass_ctx: &PassContext<'_>,
    prepared: &[PreparedBatch<'_>],
    gpu: &mut GpuResources<'_>,
    encoder: &mut wgpu::CommandEncoder,
    assets: &AssetsManager,
) {
    let PassContext {
        pass_descriptor,
        scene_bind_group,
        scene_bind_group_layout,
        scene_layout_native,
        target_format,
        view,
    } = pass_ctx;
    let target_format = *target_format;
    let pass_descriptor = pass_descriptor.clone();

    let GpuResources {
        device,
        queue,
        staging_buffer,
        geometry_pool,
        pipeline_cache,
        render_cache,
    } = gpu;

    let dynamic_buffer = staging_buffer.swap_buffers();
    let shared_buffer = geometry_pool.buffer();

    let mut pass = RenderPass::new(encoder, view, pass_descriptor);
    pass.set_bind_group(0, scene_bind_group, &[]);

    let mut current_material_type: Option<TypeId> = None;
    let mut material_uniform_buffer = UniformBuffer::new();

    for p in prepared {
        let template = &p.registration.template;

        let shader = render_cache.get_or_compile_shader(&template.shader, device);

        let material_layout = template.bind_group_layout.create_layout(device, "Material bind group layout");

        let pipeline = pipeline_cache.get_or_compile(
            crate::graphics::pipeline::PipelineCompileRequest {
                device,
                shader,
                template,
                scene_layout: scene_bind_group_layout,
                material_layout: material_layout.as_ref(),
                target_format,
                scene_layout_native: scene_layout_native.clone(),
                material_layout_native: template.bind_group_layout.clone(),
            },
        );

        if current_material_type != Some(p.batch.material.type_id) {
            pass.set_pipeline(pipeline);
            current_material_type = Some(p.batch.material.type_id);
        }

        // Build the per-frame wgpu::BindGroup from the owned BindGroup entries.
        // Use the `material_layout` (owned) instead of `pipeline.bind_group_layout`
        // so the `pipeline` borrow (which borrows `pipeline_cache`) ends
        // here (NLL), allowing the `&mut render_cache` borrow for the bind group build.
        if let Some(ref layout) = material_layout {
            let wgpu_bg = build_material_bind_group(
                BindGroupBuildRequest {
                    device,
                    queue,
                    layout,
                    bind_group: &p.bind_group,
                    render_cache,
                    assets,
                    uniform_buffer: &mut material_uniform_buffer,
                },
            );
            pass.set_bind_group(1, &wgpu_bg, &[]);
        }

        let index_count = match p.staging_offsets.index_offset {
            BufferSource::Shared(o) => (o.size / 2) as u32,
            _ => p.batch.index_count(),
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

/// Builds a `wgpu::BindGroup` from a `BindGroup` (owned entries) + GPU resources.
///
/// For uniform entries: uses `UniformBuffer` for aligned buffer creation
/// (shared logic with scene uniforms). For texture entries: resolves the
/// `GenericHandle` to a `GpuTexture` via the render cache and binds the view
/// + sampler.
struct BindGroupBuildRequest<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    layout: &'a wgpu::BindGroupLayout,
    bind_group: &'a BindGroup,
    render_cache: &'a mut RenderCache,
    assets: &'a AssetsManager,
    uniform_buffer: &'a mut UniformBuffer,
}

fn build_material_bind_group(req: BindGroupBuildRequest<'_>) -> wgpu::BindGroup {
    let BindGroupBuildRequest {
        device,
        queue,
        layout,
        bind_group,
        render_cache,
        assets,
        uniform_buffer,
    } = req;

    uniform_buffer.reset();

    for entry in &bind_group.entries {
        if let BindGroupEntry::Uniform { binding_slot, value, .. } = entry {
            uniform_buffer.upload(*binding_slot, *value);
        }
    }
    uniform_buffer.build(device);

    // Ensure all textures and samplers are in the cache (requires &mut).
    for entry in &bind_group.entries {
        if let BindGroupEntry::Texture { texture, .. } = entry {
            let Some(tex) = assets.get_asset(*texture) else { continue };
            render_cache.get_or_create_gpu_texture(*texture, device, queue, tex);
            render_cache.get_or_create_sampler(&tex.config().sampler_config, device);
        }
    }

    // Single immutable pass: build all wgpu::BindGroupEntry from the cache.
    let buffer = uniform_buffer.buffer();
    let mut entries = Vec::new();

    for entry in &bind_group.entries {
        match entry {
            BindGroupEntry::Uniform { binding_slot, value, .. } => {
                if let (Some(buf), Some(offset)) = (buffer, uniform_buffer.offset(*binding_slot)) {
                    entries.push(wgpu::BindGroupEntry {
                        binding: *binding_slot,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: buf,
                            offset,
                            size: std::num::NonZeroU64::new(value.ty().size()),
                        }),
                    });
                }
            }
            BindGroupEntry::Texture { binding_slot, sampler_binding_slot, texture, .. } => {
                if let Some(tex) = assets.get_asset(*texture) {
                    if let Some(gpu) = render_cache.gpu_texture(texture) {
                        entries.push(wgpu::BindGroupEntry {
                            binding: *binding_slot,
                            resource: wgpu::BindingResource::TextureView(gpu.view()),
                        });
                    }
                    if let Some(sampler) = render_cache.sampler(&tex.config().sampler_config) {
                        entries.push(wgpu::BindGroupEntry {
                            binding: *sampler_binding_slot,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        });
                    }
                }
            }
        }
    }

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Material bind group"),
        layout,
        entries: &entries,
    })
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