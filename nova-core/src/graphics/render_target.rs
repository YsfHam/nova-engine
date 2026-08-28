use std::cell::RefMut;

use crate::{
    assets::resolve::ResolvedMaterialTemplate, graphics::{
        bind::BindGroupAllocator, environment::EnvironmentDescriptor, pipeline::{Pipeline, PipelineCache, PipelineDescriptor}, render::RenderContext, render_pass::{RenderPass, RenderPassDescriptor}, texture::TextureFormat, uniform::{MaterialUniformEntry, UniformArena},
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
    pub(crate) encoder: wgpu::CommandEncoder,
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
            encoder,
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
            encoder: &mut self.encoder,
            view: self.view,
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
    pub fn submit(self) {
        let queue = self.render_ctx.queue();
        queue.submit(std::iter::once(self.encoder.finish()));
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
}

impl<'a> RenderTargetCommander<'a> {
    /// Builds the scene bind group (group 0) from all scene globals uploaded
    /// so far. Call once after uploading all scene globals, then bind the
    /// returned group before drawing. Returns `None` if nothing was uploaded.
    pub fn build_scene_bind_group(&mut self) -> Option<wgpu::BindGroup> {
        let scene_bind_group_layout = self.scene_bind_group_layout;
        self.uniform_arena.build_bind_group(
            self.device,
            scene_bind_group_layout
        )
    }


    // ─── Pipeline cache (group 1 layout) ────────────────────────────────

    /// Looks up or compiles a [`Pipeline`] for the given resolved template.
    /// The pipeline is cached in [`RenderContext`]; subsequent calls with
    /// the same template + target format are cache hits.
    pub fn get_or_compile_pipeline(
        &mut self,
        template: ResolvedMaterialTemplate<'_>,
    ) -> &Pipeline {
        let desc = PipelineDescriptor {
            material_template: template,
            scene_bind_group_layout: self.scene_bind_group_layout,
            target_format: self.surface_format,
        };
        self.pipeline_cache.get_or_compile(self.device, desc)
    }

    // ─── Bind group allocator (group 1: material) ───────────────────────

    /// (Re)builds the shared material uniform buffer from `materials`.
    ///
    /// Call this when the material set changes (materials added/removed).
    /// On subsequent frames with no material changes, the cached buffer is
    /// reused. This also clears the bind group cache.
    pub fn build_uniform_pool<'b, I>(&mut self, materials: I)
    where
        I: IntoIterator<Item = MaterialUniformEntry<'b>>,
    {
        self.bind_group_allocator.build_uniform_pool(self.device, materials);
    }

    /// Returns the cached bind group for a material, or builds + caches it
    /// from the uniform pool buffer + the resolved textures.
    ///
    /// `material_bind_group_layout` comes from the pipeline (group 1 layout);
    /// obtain it via [`Pipeline::bind_group_layout`](crate::graphics::pipeline::Pipeline::bind_group_layout).
    pub fn get_or_create_bind_group(
        &mut self,
        material: crate::assets::resolve::ResolvedMaterial<'_>,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> &wgpu::BindGroup {
        self.bind_group_allocator.get_or_create(
            self.device,
            material,
            material_bind_group_layout,
        )
    }

    /// Whether the material uniform pool has been built.
    pub fn is_uniform_pool_built(&self) -> bool {
        self.bind_group_allocator.is_uniform_pool_built()
    }

    /// Draws a single material in one call: begins a render pass, compiles
    /// (or fetches) the pipeline, builds (or fetches) the material bind
    /// group, records the draw, and ends the pass.
    ///
    /// This convenience owns the borrow conflict between the render pass
    /// (which borrows the encoder) and the pipeline/bind-group caches (in
    /// `RenderContext`): the encoder and `render_ctx` are disjoint fields of
    /// `RenderTarget`, so split-borrowing `&mut self` lets the pass and the
    /// cache references coexist in one call — **no cloning of `wgpu`
    /// handles**.
    ///
    /// For multi-material passes or finer control, use [`begin_render_pass`]
    /// then [`get_or_compile_pipeline`] / [`get_or_create_bind_group`]; note
    /// that those return references tied to the `RenderTarget` borrow, so the
    /// pass must be dropped before calling them, or pre-resolve before
    /// beginning the pass.
    ///
    /// - `pass_descriptor` — clear color / view config for the pass.
    /// - `scene_bind_group` — group 0, from [`build_scene_bind_group`].
    /// - `resolved_material` — the resolved material (template + textures).
    /// - `vertices` / `instances` — the draw ranges.
    pub fn draw_material(
        &mut self,
        pass_descriptor: RenderPassDescriptor<'_>,
        scene_bind_group: &wgpu::BindGroup,
        resolved_material: crate::assets::resolve::ResolvedMaterial<'_>,
        vertices: std::ops::Range<u32>,
        instances: std::ops::Range<u32>,
    ) {
        // The commander's fields are already split-borrowed from the
        // RenderContext guard, so pipeline_cache, bind_group_allocator,
        // device, and encoder are all disjoint — they coexist freely.
        let pipeline = self.pipeline_cache.get_or_compile(
            self.device,
            PipelineDescriptor {
                material_template: resolved_material.material_template,
                scene_bind_group_layout: self.scene_bind_group_layout,
                target_format: self.surface_format,
            },
        );

        let material_bind_group_layout = pipeline
            .bind_group_layout
            .as_ref()
            .expect("material declares bindings → group 1 layout exists");
        let material_bind_group = self.bind_group_allocator.get_or_create(
            self.device,
            resolved_material,
            material_bind_group_layout,
        );

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: pass_descriptor.color_view.unwrap_or(self.view),
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: match pass_descriptor.color_clear {
                    Some(color) => wgpu::LoadOp::Clear(color.into()),
                    None => wgpu::LoadOp::Load,
                },
                store: wgpu::StoreOp::Store,
            },
        };
        let color_attachments = [Some(color_attachment)];

        let mut inner = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: pass_descriptor.label,
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        inner.set_pipeline(&pipeline.pipeline);
        inner.set_bind_group(0, scene_bind_group, &[]);
        inner.set_bind_group(1, material_bind_group, &[]);
        inner.draw(vertices, instances);
    }

    // ─── Render pass ────────────────────────────────────────────────────

    /// Begins a render pass on this target's view (or a custom view if
    /// `desc.color_view` is `Some`). Borrows this `RenderTarget` mutably for
    /// the pass duration (wgpu requirement: one pass at a time).
    pub fn begin_render_pass(&mut self, desc: RenderPassDescriptor<'_>) -> RenderPass<'_> {
        let color_view = desc.color_view.unwrap_or(&self.view);

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: match desc.color_clear {
                    Some(color) => wgpu::LoadOp::Clear(color.into()),
                    None => wgpu::LoadOp::Load,
                },
                store: wgpu::StoreOp::Store,
            },
        };
        let color_attachments = [Some(color_attachment)];

        // Depth attachment: for now, depth_clear only signals intent.
        // The actual depth texture view comes from the depth pool (Step 2/C6).
        // Until the pool exists, depth_clear is accepted but not wired.
        let depth_stencil_attachment = desc.depth_clear.map(|depth| {
            wgpu::RenderPassDepthStencilAttachment {
                // TODO: replace with depth texture from pool once implemented
                view: &self.view, // placeholder — will panic if used; see note
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(depth),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }
        });

        let inner = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: desc.label,
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        RenderPass { inner }
    }
}