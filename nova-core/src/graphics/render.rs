
use crate::{EngineResult, graphics::{bind::BindGroupAllocator, context::GraphicsContext, frame::Frame, pipeline::PipelineCache}};

pub struct RenderContext {
    pub(crate) gfx: GraphicsContext,
    pub(crate) scene_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) pipeline_cache: PipelineCache,
    pub(crate) bind_group_allocator: BindGroupAllocator,
}

impl RenderContext {
    pub fn new(gfx: GraphicsContext) -> Self {
        let scene_bind_group_layout = Self::create_scene_bind_group_layout(&gfx.device);
        Self {
            scene_bind_group_layout,
            pipeline_cache: PipelineCache::new(),
            bind_group_allocator: BindGroupAllocator::new(),
            gfx,
        }
    }

    /// The singleton bind group layout for group 0 (environment uniforms).
    ///
    /// Every pipeline includes this as its first bind group layout. The
    /// `UniformArena` builds one bind group per frame from this layout.
    /// Shaders must conform to this contract: binding 0 = camera (Mat4),
    /// binding 1 = time (F32). 3D extends this (lights, fog) by extending
    /// the layout — still one layout, one bind group per frame.
    fn create_scene_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Scene bind group layout (group 0)"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    }

    pub fn scene_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.scene_bind_group_layout
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.gfx.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.gfx.queue
    }

    pub fn resize_surface(&self, width: u32, height: u32) {
        self.gfx.resize_surface(width, height);
    }

    /// Current surface texture format. Needed for pipeline compilation
    /// (`PipelineDescriptor::target_format`).
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.gfx.config.format
    }

    pub fn begin_frame(&mut self) -> EngineResult<Option<Frame>> {
        let output_opt = self.get_surface_texture()?;
        match output_opt {
            Some(output) => {
                Ok(Some(Frame::new(output)))
            }
            None => Ok(None)
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