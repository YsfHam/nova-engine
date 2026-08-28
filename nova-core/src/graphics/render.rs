
use std::{cell::{Ref, RefCell, RefMut}, rc::Rc};

use crate::{EngineResult, graphics::{bind::BindGroupAllocator, context::GraphicsContext, frame::Frame, pipeline::PipelineCache, render_target::TextureRenderTarget, texture::TextureFormat}};


/// A clonable, interior-mutable handle to the [`RenderContext`].
///
/// This is the type the outside world (proxy, batchers, etc.) sees and passes
/// around. It wraps `Rc<RefCell<RenderContext>>` so the render context can be
/// shared between the application, the asset manager, and render targets
/// without `&mut` threading issues. Cloning is cheap (Rc bump).
///
/// The inner `RenderContext` is `pub(crate)` — outsiders never access it
/// directly. Instead they call [`get_mut`](Self::get_mut) to obtain a
/// `RefMut<RenderContext>` guard, which they pass to a `RenderTarget` (the
/// target holds the guard for its lifetime, giving it direct field access).
pub struct RenderContextRef {
    inner: Rc<RefCell<RenderContext>>,
}

impl RenderContextRef {
    pub(crate) fn new(gfx: GraphicsContext) -> Self {
        Self {
            inner: Rc::new(RefCell::new(RenderContext::new(gfx)))
        }
    }

    /// Clones the handle (cheap — `Rc` bump). `pub(crate)` because the
    /// `AssetsManager` needs it, but external code should obtain the ref
    /// from `ApplicationContext.render_ctx`.
    pub(crate) fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }

    /// Immutably borrows the inner `RenderContext`. `pub(crate)` — used by
    /// the handler for `resize_surface` etc.
    pub(crate) fn get(&self) -> Ref<'_, RenderContext> {
        self.inner.borrow()
    }

    /// Mutably borrows the inner `RenderContext`, returning a guard. This is
    /// **public**: `RenderTarget::new` and `Frame::render_target` call it to
    /// obtain the guard they hold for their lifetime. The guard gives direct
    /// field access (device, pipeline_cache, bind_group_allocator) without
    /// any borrow-through-RefCell ceremony.
    pub(crate) fn get_mut(&self) -> RefMut<'_, RenderContext> {
        self.inner.borrow_mut()
    }

    /// Creates an off-screen [`TextureRenderTarget`] with the given
    /// dimensions and format. The texture uses `RENDER_ATTACHMENT` +
    /// `TEXTURE_BINDING` usage so it can be rendered into and then sampled.
    ///
    /// This is the delegated creation path: the render context is the single
    /// responsible component for creating render-target backing textures,
    /// keeping GPU resource creation centralized.
    pub fn create_texture_target(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
        label: Option<&str>,
    ) -> TextureRenderTarget {
        self.inner.borrow().create_texture_target(width, height, format, label)
    }
}

/// The central GPU state hub: device, queue, pipeline cache, bind group
/// allocator, and surface management. The struct is `pub` (it appears in
/// public return types like `RenderContextRef::get_mut`'s `RefMut`), but all
/// its fields are `pub(crate)` — external code accesses it only via the guard
/// from [`RenderContextRef::get_mut`], passed to `RenderTarget::new`.
pub struct RenderContext {
    pub(crate) gfx: GraphicsContext,
    pub(crate) pipeline_cache: PipelineCache,
    pub(crate) bind_group_allocator: BindGroupAllocator,

    command_buffers: Option<Vec<wgpu::CommandBuffer>>,
}

impl RenderContext {
    pub(crate) fn new(gfx: GraphicsContext) -> Self {
        Self {
            pipeline_cache: PipelineCache::new(),
            bind_group_allocator: BindGroupAllocator::new(),
            gfx,
            command_buffers: Some(Vec::new()),
        }
    }

    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.gfx.device
    }

    pub(crate) fn queue(&self) -> &wgpu::Queue {
        &self.gfx.queue
    }

    pub(crate) fn resize_surface(&self, width: u32, height: u32) {
        self.gfx.resize_surface(width, height);
    }

    /// Current surface texture format. Needed for pipeline compilation
    /// (`PipelineDescriptor::target_format`).
    pub(crate) fn surface_format(&self) -> wgpu::TextureFormat {
        self.gfx.config.format
    }

    /// Creates an off-screen [`TextureRenderTarget`] with the given
    /// dimensions and format. The texture uses `RENDER_ATTACHMENT` +
    /// `TEXTURE_BINDING` usage so it can be rendered into and then sampled.
    ///
    /// This is the delegated creation path: the render context is the single
    /// responsible component for creating render-target backing textures,
    /// keeping GPU resource creation centralized.
    pub fn create_texture_target(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
        label: Option<&str>,
    ) -> TextureRenderTarget {
        TextureRenderTarget::new(&self.gfx.device, width, height, format, label)
    }

    pub(crate) fn begin_frame(&mut self) -> EngineResult<Option<Frame>> {
        let output_opt = self.get_surface_texture()?;
        match output_opt {
            Some(output) => {
                Ok(Some(Frame::new(output)))
            }
            None => Ok(None)
        }
    }

    pub(crate) fn submit_command_encoder(&mut self, encoder: wgpu::CommandEncoder) {
        if let Some(command_buffers) = self.command_buffers.as_mut() {
            command_buffers.push(encoder.finish());
        }
    }

    pub(crate) fn submit_commands(&mut self) {
        let queue = &self.gfx.queue;
        if let Some(command_buffers) = self.command_buffers.replace(Vec::new()) {
            queue.submit(command_buffers);
        }
    }

    fn get_surface_texture(&mut self) -> EngineResult<Option<wgpu::SurfaceTexture>> {
        match self.gfx.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) |
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                Ok(Some(surface_texture))
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
               Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.gfx.configure_surface();
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.gfx = self.gfx.reconfigure()?;
                Ok(None)
            }
        }
    }
}