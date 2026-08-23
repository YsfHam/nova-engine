use crate::graphics::{color::Color, frame::Frame};

/// Describes how to configure a render pass.
///
/// `color_view` defaults to the frame's surface view when `None`.
///
/// `color_clear`: `Some(color)` clears the target to `color`; `None` loads existing content.
/// `depth_clear`: `Some(value)` clears depth to `value` (typically `1.0`); `None` = no depth attachment.
pub struct RenderPassDescriptor<'a> {
    pub label: Option<&'a str>,
    pub color_clear: Option<Color>,
    pub color_view: Option<&'a wgpu::TextureView>,
    pub depth_clear: Option<f32>,
}

impl<'a> RenderPassDescriptor<'a> {
    /// Default screen-clearing pass: clears color to black, no depth.
    pub fn new() -> Self {
        Self {
            label: None,
            color_clear: Some(Color::BLACK),
            color_view: None,
            depth_clear: None,
        }
    }

    /// Set the debug label for this pass (shows up in GPU debuggers).
    pub fn with_label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Clear the color target to `color`. Omit to load existing content instead.
    pub fn with_color_clear(mut self, color: Color) -> Self {
        self.color_clear = Some(color);
        self
    }

    /// Render to a custom texture view instead of the frame's surface view.
    pub fn with_color_view(mut self, view: &'a wgpu::TextureView) -> Self {
        self.color_view = Some(view);
        self
    }

    /// Enable a depth attachment and clear it to `value` (typically `1.0`).
    pub fn with_depth_clear(mut self, value: f32) -> Self {
        self.depth_clear = Some(value);
        self
    }
}

impl<'a> Default for RenderPassDescriptor<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// A scoped render pass recording context. Borrows the `Frame` mutably while alive,
/// so only one pass can be active at a time (wgpu requirement).
pub struct RenderPass<'frame> {
    inner: wgpu::RenderPass<'frame>,
}

impl<'frame> RenderPass<'frame> {
    pub fn new(frame: &'frame mut Frame<'_>, desc: RenderPassDescriptor<'_>) -> Self {
        let color_view = desc.color_view.unwrap_or(&frame.view);

        let color_attachment = Some(wgpu::RenderPassColorAttachment {
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
        });

        // Depth attachment: for now, depth_clear only signals intent.
        // The actual depth texture view comes from the depth pool (Step 2/C6).
        // Until the pool exists, depth_clear is accepted but not wired.
        let depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment> =
            desc.depth_clear.map(|depth| {
                wgpu::RenderPassDepthStencilAttachment {
                    // TODO: replace with depth texture from pool once implemented
                    view: &frame.view, // placeholder — will panic if used; see note
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(depth),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }
            });

        let inner = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: desc.label,
            color_attachments: &[color_attachment],
            depth_stencil_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        Self { inner }
    }

    // --- Draw methods (thin wrappers over wgpu::RenderPass) ---

    pub fn set_pipeline(&mut self, pipeline: &wgpu::RenderPipeline) {
        self.inner.set_pipeline(pipeline);
    }

    pub fn set_bind_group(&mut self, index: u32, bind_group: &wgpu::BindGroup, offsets: &[wgpu::DynamicOffset]) {
        self.inner.set_bind_group(index, bind_group, offsets);
    }

    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: wgpu::BufferSlice<'_>) {
        self.inner.set_vertex_buffer(slot, buffer);
    }

    pub fn set_index_buffer(&mut self, buffer: wgpu::BufferSlice<'_>, index_format: wgpu::IndexFormat) {
        self.inner.set_index_buffer(buffer, index_format);
    }

    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.inner.draw(vertices, instances);
    }

    pub fn draw_indexed(&mut self, indices: std::ops::Range<u32>, base_vertex: i32, instances: std::ops::Range<u32>) {
        self.inner.draw_indexed(indices, base_vertex, instances);
    }
}