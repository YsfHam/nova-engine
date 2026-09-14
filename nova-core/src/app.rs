use std::time::Duration;

use winit::event_loop::{ActiveEventLoop, EventLoop};

mod handler;
mod builder;

pub use builder::ApplicationBuilder;

use crate::{EngineResult, assets::{AssetsManager, defaults::DefaultAssets}, egui::EguiState, errors::EngineError, graphics::{config::GraphicsConfiguration, context::GraphicsContext, frame::Frame, render::RenderContextRef, render_target::TextureRenderTarget}, plugin::Plugins, time::Clock, window::{ControlFlow, WindowApi, WindowConfig}};

pub struct ApplicationContext {
    pub window_api: WindowApi,
    pub render_ctx: RenderContextRef,
    pub assets_manager: AssetsManager,
    pub default_assets: DefaultAssets,

    pub(crate) egui_state: EguiState,
}

impl ApplicationContext {
    fn request_window_redraw(&self) {
        self.window_api.window.request_redraw();
    }

    /// Registers an off-screen render target's texture with the egui renderer
    /// so it can be displayed inside the GUI via `ui.image()`.
    ///
    /// The texture target must use `TextureFormat::Rgba8Unorm` for egui
    /// compatibility. Call this once during `on_init` after creating the
    /// texture target, then use the returned handle's `id` with
    /// `ui.image(egui::load::SizedTexture::new(handle.id, size))`.
    #[cfg(feature = "egui")]
    pub fn register_texture(
        &mut self,
        target: &TextureRenderTarget,
        filter: crate::graphics::sampler::FilterMode,
    ) -> crate::egui::EguiTextureHandle {
        let ctx = &mut *self;
        let render_ctx = ctx.render_ctx.get();
        let device = render_ctx.device();
        let wgpu_filter = match filter {
            crate::graphics::sampler::FilterMode::Nearest => wgpu::FilterMode::Nearest,
            crate::graphics::sampler::FilterMode::Linear => wgpu::FilterMode::Linear,
        };
        ctx.egui_state.register_texture(device, target.view(), wgpu_filter)
    }

    /// Creates a `RenderTarget` bound to an off-screen `TextureRenderTarget`.
    /// Use this to render content into the texture (e.g. a 2D scene), then
    /// display the texture in the GUI via `register_texture` + `ui.image()`.
    ///
    /// The returned `RenderTarget` holds a `RefMut<RenderContext>` guard.
    /// When it is dropped, its command encoder is submitted automatically.
    /// Only one `RenderTarget` may exist at a time (the `RefCell` enforces this).
    pub fn texture_render_target<'a>(
        &'a self,
        target: &'a TextureRenderTarget,
    ) -> crate::graphics::render_target::RenderTarget<'a> {
        target.as_render_target(self.render_ctx.get_mut())
    }

    /// Returns the current surface texture format. Use this when creating
    /// off-screen texture targets that need to share pipelines with the
    /// on-screen render path.
    pub fn surface_format(&self) -> crate::graphics::texture::TextureFormat {
        let wgpu_format = self.render_ctx.get().surface_format();
        // Map back to engine-native TextureFormat.
        match wgpu_format {
            wgpu::TextureFormat::Rgba8Unorm => crate::graphics::texture::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb => crate::graphics::texture::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Bgra8UnormSrgb => crate::graphics::texture::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::R8Unorm => crate::graphics::texture::TextureFormat::R8Unorm,
            wgpu::TextureFormat::R32Float => crate::graphics::texture::TextureFormat::R32Float,
            wgpu::TextureFormat::Rgba32Float => crate::graphics::texture::TextureFormat::Rgba32Float,
            _ => crate::graphics::texture::TextureFormat::Bgra8UnormSrgb, // fallback
        }
    }
}

pub trait ApplicationProxy {
    fn on_init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()>;
    fn on_update(&mut self, ctx: &mut ApplicationContext, dt: Duration);
    fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame);
    #[cfg(feature = "egui")]
    fn on_gui(&mut self, ui: &mut egui::Ui);
}

pub struct Application<P: ApplicationProxy> {
    window_config: WindowConfig,
    gfx_config: GraphicsConfiguration,
    proxy: P,
    control_flow: ControlFlow,
    ctx: Option<ApplicationContext>,
    frame_clock: Clock,
    frame_time: Duration,
    plugins: Plugins,

    engine_error: Option<EngineError>,
}

impl<P: ApplicationProxy> Application<P> {
    pub fn run(mut self) -> EngineResult<()> {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(self.control_flow.into());

        event_loop.run_app(&mut self).unwrap();
        match self.engine_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn from_builder(builder: ApplicationBuilder<P>) -> Self {
        Self {
            window_config: builder.window_config,
            gfx_config: builder.gfx_config,
            proxy: builder.proxy,
            control_flow: builder.control_flow,
            ctx: None,
            frame_clock: Clock::new(),
            frame_time: Duration::from_millis(1000 / builder.frame_rate),
            plugins: builder.plugins,
            engine_error: None,
        }
    }

    fn init(&mut self, event_loop: &ActiveEventLoop) -> EngineResult<()> {
        let window_visible= self.window_config.visible;

        let window_config = 
            self.window_config.clone()
            .with_visible(false)
        ;
        let window_api = WindowApi::new(event_loop, window_config)?;
        let gfx = GraphicsContext::new(window_api.window.clone(), self.gfx_config)?;
        let render_ctx = RenderContextRef::new(gfx);
        #[cfg(feature = "egui")]
        let egui_state = EguiState::new(&render_ctx.get(), window_api.window.clone());
        #[cfg(not(feature = "egui"))]
        let egui_state = EguiState;

        let assets_manager = AssetsManager::new();

        let default_assets = DefaultAssets::new();

        let mut app_ctx = ApplicationContext {
            window_api,
            render_ctx,
            assets_manager,
            default_assets,

            egui_state
        };

        self.plugins.init(&mut app_ctx)?;

        app_ctx.window_api.window.set_visible(window_visible);
        self.ctx = Some(app_ctx);

        self.proxy.on_init(self.ctx.as_mut().unwrap())?;

        Ok(())
    }
}