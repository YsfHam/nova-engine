use crate::{
    assets::resolve::ResolvedMaterialTemplate,
    graphics::{
        pipeline::{Pipeline, PipelineDescriptor},
        render::RenderContext,
        render_pass::{RenderPass, RenderPassDescriptor},
        uniform::{MaterialUniformEntry, UniformArena, UniformValue},
    },
};

/// A view-agnostic render target. Owns a command encoder and a per-target
/// [`UniformArena`], and borrows `&mut [`RenderContext`]` for direct access to
/// the pipeline cache and bind group allocator.
///
/// `RenderTarget` is generic over its target view: it works for on-screen
/// rendering (the surface view from [`Frame`](crate::graphics::frame::Frame))
/// and off-screen rendering (any arbitrary `TextureView`). To submit
/// recorded commands, call [`submit`](Self::submit); the caller is
/// responsible for presenting the surface (via `Frame::present`).
///
/// Because `RenderTarget` holds `&mut RenderContext`, methods like
/// [`get_or_compile_pipeline`] and [`get_or_create_bind_group`] can return
/// references whose lifetime is tied to the `RenderTarget` borrow — no
/// `RefCell` or guard types are needed.
pub struct RenderTarget<'a> {
    render_ctx: &'a mut RenderContext,
    pub(crate) view: &'a wgpu::TextureView,
    pub(crate) encoder: wgpu::CommandEncoder,
    uniform_arena: UniformArena,
}

impl<'a> RenderTarget<'a> {
    /// Creates a render target rendering into `view`, borrowing
    /// `render_ctx` mutably for pipeline-cache and bind-group-allocator
    /// access.
    pub fn new(render_ctx: &'a mut RenderContext, view: &'a wgpu::TextureView) -> Self {
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
        }
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

    // ─── Uniform arena (group 0: environment) ───────────────────────────

    /// Uploads a scene-global uniform value at `binding_slot` (group 0).
    pub fn upload_uniform(&mut self, binding_slot: u32, value: UniformValue) {
        self.uniform_arena.upload(binding_slot, value);
    }

    /// Builds the scene bind group (group 0) from all scene globals uploaded
    /// so far. Call once after uploading all scene globals, then bind the
    /// returned group before drawing. Returns `None` if nothing was uploaded.
    pub fn build_scene_bind_group(&mut self) -> Option<wgpu::BindGroup> {
        self.uniform_arena.build_bind_group(
            self.render_ctx.device(),
            self.render_ctx.scene_bind_group_layout(),
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
            scene_bind_group_layout: &self.render_ctx.scene_bind_group_layout,
            target_format: self.render_ctx.surface_format(),
        };
        self.render_ctx
            .pipeline_cache
            .get_or_compile(&self.render_ctx.gfx.device, desc)
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
        self.render_ctx
            .bind_group_allocator
            .build_uniform_pool(&self.render_ctx.gfx.device, materials);
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
        self.render_ctx.bind_group_allocator.get_or_create(
            &self.render_ctx.gfx.device,
            material,
            material_bind_group_layout,
        )
    }

    /// Whether the material uniform pool has been built.
    pub fn is_uniform_pool_built(&self) -> bool {
        self.render_ctx.bind_group_allocator.is_uniform_pool_built()
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