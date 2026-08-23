
use crate::{EngineResult, graphics::{context::GraphicsContext, frame::Frame}};

pub struct RenderContext {
    gfx: GraphicsContext,
}

impl RenderContext {
    pub fn new(gfx: GraphicsContext) -> Self {
        Self {
            gfx
        }
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

    pub fn begin_frame(&mut self) -> EngineResult<Option<Frame<'_>>> {
        let output_opt = self.get_surface_texture()?;
        match output_opt {
            Some(output) => {
                Ok(Some(Frame::new(self, output)))
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