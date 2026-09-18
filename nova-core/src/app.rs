use std::{cell::RefCell, collections::VecDeque, time::Duration};

use winit::event_loop::{ActiveEventLoop, EventLoop};

mod handler;
mod builder;

pub use builder::ApplicationBuilder;

use crate::{EngineResult, assets::{AssetsManager, defaults::DefaultAssets}, egui::EguiState, errors::EngineError, graphics::{config::GraphicsConfiguration, context::GraphicsContext, frame::Frame, render::RenderContextRef, render_target::TextureRenderTarget}, input::Input, plugin::Plugins, time::Clock, window::{ControlFlow, WindowApi, WindowConfig}};

pub struct AppInfo {
    pub total_frames: u64,
    pub total_time: Duration,
    frame_timestamps: VecDeque<Duration>,
    rolling_fps: f64,
}

impl AppInfo {
    fn new() -> Self {
        Self {
            total_frames: 0,
            total_time: Duration::from_secs(0),
            frame_timestamps: VecDeque::new(),
            rolling_fps: 0.0,
        }
    }

    pub fn fps(&self) -> f64 {
        self.rolling_fps
    }

    pub(crate) fn record_frame(&mut self) {
        self.total_frames += 1;
        self.frame_timestamps.push_back(self.total_time);

        while let Some(&first_timestamp) = self.frame_timestamps.front() {
            if self.total_time - first_timestamp <= Duration::from_secs(1) {
                break;
            }
            self.frame_timestamps.pop_front();
        }

        if let (Some(&first_timestamp), Some(&last_timestamp)) =
            (self.frame_timestamps.front(), self.frame_timestamps.back())
        {
            let elapsed = last_timestamp - first_timestamp;
            if elapsed > Duration::ZERO {
                self.rolling_fps = (self.frame_timestamps.len() - 1) as f64
                    / elapsed.as_secs_f64();
            } else {
                self.rolling_fps = 0.0;
            }
        } else {
            self.rolling_fps = 0.0;
        }
    }
}

pub struct ApplicationContext {
    pub window_api: WindowApi,
    pub render_ctx: RenderContextRef,
    pub assets_manager: AssetsManager,
    pub default_assets: DefaultAssets,

    pub(crate) info: AppInfo,

    pub(crate) input_state: Input,

    pub(crate) egui_state: RefCell<EguiState>,
}

impl ApplicationContext {
    fn request_window_redraw(&self) {
        self.window_api.window.request_redraw();
    }

    pub fn input(&self) -> &Input {
        &self.input_state
    }

    pub fn info(&self) -> &AppInfo {
        &self.info
    }

    
    #[cfg(feature = "egui")]
    pub fn register_texture(
        &mut self,
        target: &TextureRenderTarget,
        filter: crate::graphics::sampler::FilterMode,
    ) -> crate::egui::EguiTextureHandle {
        let render_ctx = self.render_ctx.get();
        let device = render_ctx.device();
        self.egui_state.borrow_mut().register_texture(device, &target.view(), filter)
    }
}

pub trait ApplicationProxy {
    fn on_init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()>;
    fn on_update(&mut self, ctx: &mut ApplicationContext, dt: Duration);
    fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame);
    #[cfg(feature = "egui")]
    fn on_gui(&mut self, ctx: &ApplicationContext, ui: &mut egui::Ui);
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
            frame_time: Duration::from_secs_f64(1.0 / builder.frame_rate.max(1) as f64),
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

        let input_state = Input::new(window_api.window.scale_factor());

        let mut app_ctx = ApplicationContext {
            window_api,
            render_ctx,
            assets_manager,
            default_assets,

            info: AppInfo::new(),

            input_state,
            egui_state: RefCell::new(egui_state)
        };

        self.plugins.init(&mut app_ctx)?;

        app_ctx.window_api.window.set_visible(window_visible);
        self.ctx = Some(app_ctx);

        self.proxy.on_init(self.ctx.as_mut().unwrap())?;

        Ok(())
    }
}