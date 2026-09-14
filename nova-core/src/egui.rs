use std::sync::Arc;

use winit::{event::WindowEvent, window::Window};

use crate::graphics::{render::RenderContext, render_pass::{RenderPass, RenderPassDescriptor}, render_target::RenderTarget};

#[cfg(feature = "egui")]
pub use egui;
#[cfg(feature = "egui")]
pub use egui::Ui;

#[cfg(feature = "egui")]
/// A handle to a texture registered with the egui renderer. Use this with
/// `ui.image()` to display an off-screen render target inside the GUI.
pub struct EguiTextureHandle {
    pub id: egui::TextureId,
}

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

    pub(crate) fn on_event(&mut self, event: &WindowEvent) -> egui_winit::EventResponse {
        self.state.on_window_event(&self.window, event)
    }

    /// Registers a native wgpu texture view with the egui renderer and returns
    /// a `TextureId` that can be used in `ui.image()`. The texture must have
    /// the format `wgpu::TextureFormat::Rgba8Unorm`.
    pub(crate) fn register_texture(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        filter: wgpu::FilterMode,
    ) -> EguiTextureHandle {
        let id = self.renderer.register_native_texture(device, view, filter);
        EguiTextureHandle { id }
    }

    pub(crate) fn ui(
        &mut self, 
        mut render_target: RenderTarget<'_>, 
        run_ui: impl FnMut(&mut Ui)
    ) {
        let input = self.state.take_egui_input(&self.window);
        let full_output = self.state.egui_ctx().run_ui(input, run_ui);
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