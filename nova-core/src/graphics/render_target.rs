use std::cell::RefMut;


use crate::{
    assets::resolve::ResolvedMaterial, graphics::{
        bind::BindGroupAllocator, buffer::{Offset, StagingBufferPool}, draw_batch::DrawBatch, environment::EnvironmentDescriptor, pipeline::{PipelineCache, PipelineDescriptor}, render::RenderContext, render_pass::{IndexFormat, RenderPass, RenderPassDescriptor}, texture::TextureFormat, uniform::{MaterialUniformEntry, UniformArena},
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
    queue: &'a wgpu::Queue,
}

impl<'a> RenderTargetCommander<'a> {

    pub fn submit_batches<I>(
        mut self,
        pass_descriptor: RenderPassDescriptor,
        batches: I,
        assets: &crate::assets::AssetsManager,
    )
    where
        I: IntoIterator<Item = crate::graphics::draw_batch::DrawBatch>,
    {

        let batches: Vec<_> = batches.into_iter().collect();
        if batches.is_empty() {
            return;
        }

        let scene_bind_group = self
            .build_scene_bind_group()
            .expect("scene uniforms uploaded");

        self.build_uniform_pool_from_batches(&batches, assets);


        let Self {
            device,
            surface_format,
            pipeline_cache,
            bind_group_allocator,
            scene_bind_group_layout,
            encoder,
            view,
            staging_buffer,
            queue,
            ..
        } = self;

        
        let batches_with_offsets = Self::build_staging_buffer(
            batches.into_iter(),
            staging_buffer,
            device, 
            queue, 
            encoder
        ).collect::<Vec<_>>();


        let mut pass = RenderPass::new(encoder, view, pass_descriptor);
        pass.set_bind_group(0, &scene_bind_group, &[]);

        let buffer = staging_buffer.swap_buffers();

        // Draw each batch in order — no sorting, no grouping.
        for (batch, offsets) in batches_with_offsets {

            // Resolve the material → template → pipeline + bind group.
            let resolved_material = match crate::assets::resolve::ResolvedMaterial::new(
                batch.material,
                assets,
            ) {
                Ok(rm) => rm,
                Err(_) => continue,
            };

            Self::set_pipeline(&mut pass, device, pipeline_cache, resolved_material, scene_bind_group_layout, surface_format, bind_group_allocator);
            Self::draw_call(&mut pass, buffer, &offsets, batch.index_count(), batch.instance_count());
        }
        // pass dropped here — ends the render pass.
    }

    /// Builds the scene bind group (group 0) from all scene globals uploaded
    /// so far. Call once after uploading all scene globals, then bind the
    /// returned group before drawing. Returns `None` if nothing was uploaded.
    fn build_scene_bind_group(&mut self) -> Option<wgpu::BindGroup> {
        let scene_bind_group_layout = self.scene_bind_group_layout;
        self.uniform_arena.build_bind_group(
            self.device,
            scene_bind_group_layout
        )
    }

    // ─── Bind group allocator (group 1: material) ───────────────────────

    /// (Re)builds the shared material uniform buffer from `materials`.
    ///
    /// Call this when the material set changes (materials added/removed).
    /// On subsequent frames with no material changes, the cached buffer is
    /// reused. This also clears the bind group cache.
    fn build_uniform_pool<'b, I>(&mut self, materials: I)
    where
        I: IntoIterator<Item = MaterialUniformEntry<'b>>,
    {
        self.bind_group_allocator.build_uniform_pool(self.device, materials);
    }

    /// Whether the material uniform pool has been built.
    fn is_uniform_pool_built(&self) -> bool {
        self.bind_group_allocator.is_uniform_pool_built()
    }


    fn build_uniform_pool_from_batches(&mut self, batches: &[DrawBatch], assets: &crate::assets::AssetsManager) {
        if self.is_uniform_pool_built() {
            return;
        }

        let materials = batches.iter().filter_map(|batch| {
            let material_handle = batch.material;
                let material = assets
                    .get_asset(material_handle)?;
                let template = assets
                    .get_asset(material.template())?;

                Some(MaterialUniformEntry {
                    handle: material_handle,
                    material,
                    template,
                })
        });

        self.build_uniform_pool(materials);
    }

    fn build_staging_buffer(
        batches: impl Iterator<Item = DrawBatch>,
        staging_buffer: &mut StagingBufferPool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder
    ) -> impl Iterator<Item = (DrawBatch, BatchStagingBufferOffsets)> {
        batches.map(|batch| {
            let vertices = batch.vertices();
            let indices = batch.indices();
            let instances = batch.instances();

            let vertex_offset = staging_buffer.upload(vertices, device, queue, encoder);
            let index_offset = staging_buffer.upload(bytemuck::cast_slice(indices), device, queue, encoder);
            let instance_offset = instances.map(|instances| staging_buffer.upload(instances, device, queue, encoder));

            let offsets = BatchStagingBufferOffsets {
                vertex_offset,
                index_offset,
                instance_offset,
            };

            (batch, offsets)
        })
    }

    fn set_pipeline(
        pass: &mut RenderPass, 
        device: &wgpu::Device, 
        pipeline_cache: &mut PipelineCache,
        resolved_material: ResolvedMaterial<'_>, 
        scene_layout: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
        bind_group_allocator: &mut BindGroupAllocator,
    ) {
        let pipeline = pipeline_cache.get_or_compile(
                device,
                PipelineDescriptor {
                    material_template: resolved_material.material_template,
                    scene_bind_group_layout: scene_layout,
                    target_format: surface_format,
                },
            );

        let material_bind_group_layout = pipeline.bind_group_layout.as_ref();
        pass.set_pipeline(&pipeline);

        if let Some(bgl) = material_bind_group_layout {
            let bg = bind_group_allocator.get_or_create(
                device,
                resolved_material,
                bgl,
            );
            pass.set_bind_group(1, bg, &[]);
        }
    }

    fn draw_call(
        pass: &mut RenderPass,
        buffer: &wgpu::Buffer,
        offsets: &BatchStagingBufferOffsets,
        index_count: u32,
        instance_count: u32
    ) {

        let BatchStagingBufferOffsets {
            vertex_offset,
            index_offset,
            instance_offset,
        } = offsets;


        pass.set_vertex_buffer(0, buffer.slice(vertex_offset.offset..(vertex_offset.offset + vertex_offset.size)));
        pass.set_index_buffer(buffer.slice(index_offset.offset..(index_offset.offset + index_offset.size)), IndexFormat::Uint16);

        if let Some(instance_offset) = instance_offset {
            pass.set_vertex_buffer(1, buffer.slice(instance_offset.offset..(instance_offset.offset + instance_offset.size)));
        }

        pass.draw_indexed(0..index_count, 0, 0..instance_count);
    }
}

struct BatchStagingBufferOffsets {
    vertex_offset: Offset,
    index_offset: Offset,
    instance_offset: Option<Offset>
}