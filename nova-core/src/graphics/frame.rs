use crate::graphics::{render::RenderContextRef, render_target::RenderTarget};

/// The on-screen frame: owns the acquired surface texture and its view.
///
/// `Frame` is lightweight — it holds only the `SurfaceTexture` (for present)
/// and its `TextureView` (passed to a [`RenderTarget`]). All command recording
/// lives on [`RenderTarget`], which holds a `RefMut<RenderContext>` guard.
///
/// Call [`render_target`](Self::render_target) to obtain a `RenderTarget`
/// bound to this frame's view, record commands via its commander, submit it,
/// then call [`present`](Self::present) to present the surface texture.
pub struct Frame {
    output: wgpu::SurfaceTexture,
    view: wgpu::TextureView,
}

impl Frame {
    pub(crate) fn new(output: wgpu::SurfaceTexture) -> Self {
        let view = output
            .texture
            .create_view(&wgpu::wgt::TextureViewDescriptor::default());
        Self { output, view }
    }

    /// The surface texture view.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Creates a `RenderTarget` bound to this frame's surface view. The
    /// target holds a `RefMut<RenderContext>` guard (obtained from
    /// `render_ctx`) for its lifetime. Use the target's [`commander`] to
    /// record draw commands, then call [`submit`](RenderTarget::submit) to
    /// submit the encoder.
    ///
    /// After the target is submitted (and dropped), call [`present`](Self::present)
    /// with the same `render_ctx` to present the surface texture.
    ///
    /// [`commander`]: crate::graphics::render_target::RenderTarget::commander
    pub fn render_target<'a>(&'a mut self, render_ctx: &'a RenderContextRef) -> RenderTarget<'a> {
        RenderTarget::new(render_ctx.get_mut(), &self.view)
    }

    /// Presents the surface texture to the screen. Call this after the
    /// `RenderTarget` has been submitted (and dropped). The `render_ctx` is
    /// used to access the queue for presentation.
    pub fn present(self, render_ctx: &RenderContextRef) {
        render_ctx.get().queue().present(self.output);
    }
}