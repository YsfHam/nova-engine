/// The on-screen frame: owns the acquired surface texture and its view.
///
/// `Frame` is deliberately lightweight — it holds only the `SurfaceTexture`
/// (for present) and its `TextureView` (passed to a [`RenderTarget`]). All
/// command recording, uniform uploads, and pipeline/bind-group caching live
/// on [`RenderTarget`], which borrows `&mut RenderContext`.
///
/// `Frame` has no lifetime tied to `RenderContext`; it can outlive the
/// borrow used to create it. Call [`present`](Self::present) after the
/// `RenderTarget` has been submitted.
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

    /// The surface texture view. Pass this to [`RenderTarget::new`].
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Presents the surface texture to the screen. Call this *after* the
    /// `RenderTarget` has been submitted.
    pub fn present(self, queue: &wgpu::Queue) {
        queue.present(self.output);
    }
}