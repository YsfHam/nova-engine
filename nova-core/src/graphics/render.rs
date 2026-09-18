
use std::{cell::{Ref, RefCell, RefMut}, collections::HashMap, rc::Rc};

use crate::{EngineResult, assets::handle::Handle, graphics::{buffer::StagingBufferPool, context::GraphicsContext, frame::Frame, geometry::GeometryPool, pipeline::{MaterialRegistry, PipelineCache}, render_target::TextureRenderTarget, sampler::SamplerConfig, shader::{self, Shader, ShaderInfo}, texture::{GpuTexture, Texture, TextureConfig}}};


/// Groups the three GPU resource caches (textures, samplers, shaders) into a
/// single struct so `RenderContext` has one field and the commander borrows
/// one `&mut RenderCache` instead of three separate cache references.
pub(crate) struct RenderCache {
    texture_cache: HashMap<Handle<Texture>, GpuTexture>,
    sampler_cache: HashMap<SamplerConfig, wgpu::Sampler>,
    shader_cache: HashMap<ShaderInfo, Shader>,
}

impl RenderCache {
    pub(crate) fn new() -> Self {
        Self {
            texture_cache: HashMap::new(),
            sampler_cache: HashMap::new(),
            shader_cache: HashMap::new(),
        }
    }

    /// Returns the cached `GpuTexture` for `handle`, or creates one from the
    /// CPU `Texture` asset and caches it.
    pub(crate) fn get_or_create_gpu_texture(
        &mut self,
        handle: Handle<Texture>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &Texture,
    ) -> &GpuTexture {
        if !self.texture_cache.contains_key(&handle) {
            let gpu = GpuTexture::new(device, queue, texture.data(), texture.config());
            self.texture_cache.insert(handle, gpu);
        }
        self.texture_cache.get(&handle).unwrap()
    }

    pub(crate) fn remove_texture(&mut self, handle: Handle<Texture>) {
        self.texture_cache.remove(&handle);
    }

    /// Returns a reference to a cached `GpuTexture` without creating one.
    /// The caller must have already called `get_or_create_gpu_texture`.
    pub(crate) fn gpu_texture(&self, handle: &Handle<Texture>) -> Option<&GpuTexture> {
        self.texture_cache.get(handle)
    }

    /// Returns a reference to a cached sampler without creating one.
    /// The caller must have already called `get_or_create_sampler`.
    pub(crate) fn sampler(&self, config: &SamplerConfig) -> Option<&wgpu::Sampler> {
        self.sampler_cache.get(config)
    }

    /// Returns the cached sampler for `config`, or creates one and caches it.
    pub(crate) fn get_or_create_sampler(
        &mut self,
        config: &SamplerConfig,
        device: &wgpu::Device,
    ) -> &wgpu::Sampler {
        if !self.sampler_cache.contains_key(config) {
            self.sampler_cache
                .insert(config.clone(), device.create_sampler(&config.descriptor()));
        }
        self.sampler_cache.get(config).unwrap()
    }

    /// Returns the cached shader for `ShaderInfo`, or compiles one and caches it.
    pub(crate) fn get_or_compile_shader(
        &mut self,
        shader_info: &ShaderInfo,
        device: &wgpu::Device,
    ) -> &Shader {
        self.shader_cache
            .entry(shader_info.clone())
            .or_insert_with(|| {
                let source_str = shader::load_source(&shader_info.source)
                    .unwrap_or_else(|e| panic!("Failed to load shader source: {e}"));
                let label = shader::source_label(&shader_info.source);
                Shader::new(device, &label, &source_str, shader_info.entry_point.clone())
            })
    }
}


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

    /// Creates an off-screen [`TextureRenderTarget`] from a `TextureConfig`
    /// and a sample count. The texture uses `RENDER_ATTACHMENT` +
    /// `TEXTURE_BINDING` usage so it can be rendered into and then sampled.
    ///
    /// Pass `sample_count = 1` for no MSAA, or `4` for 4× MSAA.
    pub fn create_texture_target(
        &self,
        config: TextureConfig,
    ) -> TextureRenderTarget {
        self.inner
            .borrow()
            .create_texture_target(config)
    }

    /// Registers a material type so the renderer can resolve it by `TypeId`
    /// at draw time. Call during plugin init for each material type.
    pub fn register_material<M: crate::graphics::material::Material>(&self) {
        self.inner.borrow_mut().register_material::<M>();
    }

    /// Inserts shared geometry into the persistent [`GeometryPool`] and
    /// returns a [`GeometryRef`] that can be used in
    /// [`DrawBatch::with_shared_geometry`](crate::graphics::draw_batch::DrawBatch::with_shared_geometry).
    ///
    /// The geometry is uploaded immediately and its offsets are permanent.
    /// Call this once (at init, when a mesh loads, etc.) — not per frame.
    pub fn insert_geometry(
        &self,
        vertices: &[u8],
        indices: &[u16],
    ) -> crate::graphics::geometry::GeometryRef {
        self.inner.borrow_mut().insert_geometry(vertices, indices)
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
    pub(crate) staging_buffer_pool: StagingBufferPool,
    pub(crate) geometry_pool: GeometryPool,
    pub(crate) render_cache: RenderCache,
    pub(crate) material_registry: MaterialRegistry,
    command_buffers: Option<Vec<wgpu::CommandBuffer>>,
}

impl RenderContext {
    pub(crate) fn new(gfx: GraphicsContext) -> Self {
        Self {
            pipeline_cache: PipelineCache::new(),
            staging_buffer_pool: StagingBufferPool::new(&gfx.device, 1024 * 1024),
            geometry_pool: GeometryPool::new(&gfx.device),
            render_cache: RenderCache::new(),
            material_registry: MaterialRegistry::new(),
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

    /// Registers a material type so the renderer can resolve it by `TypeId`
    /// at draw time. Call during plugin init for each material type.
    pub fn register_material<M: crate::graphics::material::Material>(&mut self) {
        self.material_registry.register::<M>();
    }

    /// Creates an off-screen [`TextureRenderTarget`] from a `TextureConfig`
    /// and a sample count.
    fn create_texture_target(
        &self,
        config: TextureConfig,
    ) -> TextureRenderTarget {
        TextureRenderTarget::new(&self.gfx.device, config)
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


    /// Inserts shared geometry into the persistent [`GeometryPool`] and
    /// returns a [`GeometryRef`] that can be used in
    /// [`DrawBatch::with_shared_geometry`](crate::graphics::draw_batch::DrawBatch::with_shared_geometry).
    ///
    /// The geometry is uploaded immediately and its offsets are permanent.
    /// Call this once (at init, when a mesh loads, etc.) — not per frame.
    pub fn insert_geometry(
        &mut self,
        vertices: &[u8],
        indices: &[u16],
    ) -> crate::graphics::geometry::GeometryRef {

        let mut encoder = 
            self
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let geo = self.geometry_pool.insert(
            vertices,
            indices,
            &self.gfx.device,
            &self.gfx.queue,
            &mut encoder,
        );

        self.submit_command_encoder(encoder);

        geo
    }
}