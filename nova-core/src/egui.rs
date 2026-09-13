use std::sync::Arc;

use winit::{event::WindowEvent, window::Window};

use crate::graphics::{render::RenderContext, render_pass::{RenderPass, RenderPassDescriptor}, render_target::RenderTarget};

#[cfg(feature = "egui")]
pub use egui;

#[cfg(feature = "egui")]
pub(crate) struct EguiState {
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    window: Arc<Window>,
}

#[cfg(not(feature = "egui"))]
pub(crate) struct EguiState;

#[cfg(feature = "egui")]
impl EguiState {
    pub(crate) fn new(render_ctx: &RenderContext, window: Arc<Window>) -> Self {
        let context = egui::Context::default();
        let state = egui_winit::State::new(
            context,
            egui::ViewportId::from_hash_of(u64::from(window.id())),
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            None
        );


        let renderer = egui_wgpu::Renderer::new(
            render_ctx.device(),
            render_ctx.gfx.config.format,
            egui_wgpu::RendererOptions::default()   
        );

        Self {
            state,
            renderer,
            window,
        }
    }

    pub(crate) fn context(&self) -> &egui::Context {
        self.state.egui_ctx()
    }

    pub(crate) fn on_event(&mut self, event: &WindowEvent) -> egui_winit::EventResponse {
        self.state.on_window_event(&self.window, event)
    }

    pub(crate) fn new_frame(&mut self) {
        let input = self.state.take_egui_input(&self.window);
        self.state.egui_ctx().begin_pass(input);
    }

    pub(crate) fn submit_frame(&mut self, mut render_target: RenderTarget<'_>) {
        let full_output = self.state.egui_ctx().end_pass();
        self.state.handle_platform_output(&self.window, full_output.platform_output);
        let tesselated = self.state.egui_ctx().tessellate(full_output.shapes, full_output.pixels_per_point);
        let render_ctx = &render_target.render_ctx;
        let device = render_ctx.device();
        let queue = render_ctx.queue();
        let mut encoder = render_target.encoder.as_mut().unwrap();

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, &image_delta[0]);
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: self.window.inner_size().into(),
            pixels_per_point: self.window.scale_factor() as f32,
        };

        self.renderer.update_buffers(device, queue, &mut encoder, &tesselated, &screen_descriptor);

        let rpass = RenderPass::new(
            &mut encoder, 
            render_target.view, 
            RenderPassDescriptor::new()
            .with_label("Egui Render pass")
        );
        let mut rpass = rpass.inner.forget_lifetime();
        self.renderer.render(&mut rpass, &tesselated, &screen_descriptor);

        for tex in &full_output.textures_delta.free {
            self.renderer.free_texture(tex);
        }

    }
}