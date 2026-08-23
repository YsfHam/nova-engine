use wgpu::wgt::TextureViewDescriptor;

use crate::graphics::{render::RenderContext, render_pass::{RenderPass, RenderPassDescriptor}};

pub struct Frame<'a> {
    render_ctx: &'a RenderContext,
    pub(crate) view: wgpu::TextureView,
    pub(crate) encoder: wgpu::CommandEncoder,
    output: wgpu::SurfaceTexture,
}

impl<'a> Frame<'a> {

    pub(crate) fn new(render_ctx: &'a RenderContext, output: wgpu::SurfaceTexture) -> Self {
        let view = output.texture.create_view(&TextureViewDescriptor::default());
        let encoder = render_ctx.device().create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Frame Encoder"),
        });

        Self {
            render_ctx,
            view,
            encoder,
            output,
        }
    }

    pub fn begin_render_pass(&mut self, desc: RenderPassDescriptor<'_>) -> RenderPass<'_> {
        RenderPass::new(self, desc)
    }

    pub fn submit(self) {
        let queue = self.render_ctx.queue();
        queue.submit(std::iter::once(self.encoder.finish()));
        queue.present(self.output);
    }
}