use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use winit::window::Window;

use crate::EngineResult;
use crate::graphics::config::GraphicsConfiguration;

pub struct GraphicsContext {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) width: AtomicU32,
    pub(crate) height: AtomicU32,
    window: Arc<Window>,
    gfx_config: GraphicsConfiguration,
}

impl GraphicsContext {

    pub(crate) fn new(window: Arc<Window>, gfx_config: GraphicsConfiguration) -> EngineResult<Self> {
        pollster::block_on(Self::new_async(window, gfx_config))
    }

    async fn new_async(window: Arc<Window>, gfx_config: GraphicsConfiguration) -> EngineResult<Self> {
        let size = window.inner_size();
        let instance = Self::create_instance();
        let surface = instance.create_surface(window.clone())?;
        let adapter = Self::create_adapter(instance, &surface, &gfx_config).await?;
        let (device, queue) = Self::create_device_and_queue(&adapter).await?;
        let config = Self::create_surface_config(&surface, &adapter, size.width, size.height, &gfx_config);

        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            width: AtomicU32::new(size.width),
            height: AtomicU32::new(size.height),
            config,
            window,
            gfx_config
        })
    }

    pub(crate) fn resize_surface(&self, width: u32, height: u32) {
        if width > 0 && height > 0
            && (width != self.width.load(Ordering::Relaxed)
                || height != self.height.load(Ordering::Relaxed))
        {
            self.width.store(width, Ordering::Relaxed);
            self.height.store(height, Ordering::Relaxed);
            self.configure_surface();
        }
    }

    pub(crate) fn configure_surface(&self) {
        let mut config = self.config.clone();
        config.width = self.width.load(Ordering::Relaxed);
        config.height = self.height.load(Ordering::Relaxed);
        self.surface.configure(&self.device, &config);
    }

    pub(crate) fn reconfigure(&self) -> EngineResult<Self> {
        Self::new(self.window.clone(), self.gfx_config)
    }


    fn create_instance() -> wgpu::Instance {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        })
    }

    async fn create_adapter(instance:wgpu::Instance, surface: &wgpu::Surface<'_>, gfx_config: &GraphicsConfiguration) -> EngineResult<wgpu::Adapter> {
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: gfx_config.power_preference,
            force_fallback_adapter: false,
            compatible_surface: Some(surface),
            apply_limit_buckets: false,
        }).await?;

        Ok(adapter)
    }

    async fn create_device_and_queue(adapter: &wgpu::Adapter) -> EngineResult<(wgpu::Device, wgpu::Queue)> {
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Nova engine graphics device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
        }).await?;


        Ok((device, queue))
    }

    fn create_surface_config(surface: &wgpu::Surface<'_>, adapter: &wgpu::Adapter, width: u32, height: u32, gfx_config: &GraphicsConfiguration) -> wgpu::SurfaceConfiguration {
        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: gfx_config.present_mode,
            alpha_mode: gfx_config.alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        }
    }
}