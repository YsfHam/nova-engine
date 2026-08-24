use std::{cell::RefCell, rc::Rc, time::Duration};

use winit::{event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::WindowAttributes};

mod handler;
mod builder;

pub use builder::ApplicationBuilder;

use crate::{EngineResult, assets::AssetsManager, errors::EngineError, graphics::{config::GraphicsConfiguration, context::GraphicsContext, frame::Frame, render::RenderContext, sampler::SamplerLoader, shader::ShaderLoader, texture::TextureLoader}, time::Clock, window::WindowApi};

pub struct ApplicationContext {
    window_api: WindowApi,
    render_ctx: Rc<RefCell<RenderContext>>,
    pub assets_manager: AssetsManager,
}

impl ApplicationContext {
    fn request_window_redraw(&self) {
        self.window_api.window.request_redraw();
    }
}

pub trait ApplicationProxy {
    fn on_init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()>;
    fn on_update(&mut self, ctx: &mut ApplicationContext, dt: Duration);
    fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame);
}

pub struct Application<P: ApplicationProxy> {
    window_attributes: WindowAttributes,
    gfx_config: GraphicsConfiguration,
    proxy: P,
    control_flow: ControlFlow,
    ctx: Option<ApplicationContext>,
    frame_clock: Clock,
    frame_time: Duration,

    engine_error: Option<EngineError>,
}

impl<P: ApplicationProxy> Application<P> {
    pub fn run(mut self) -> EngineResult<()> {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(self.control_flow);

        event_loop.run_app(&mut self).unwrap();
        match self.engine_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn from_builder(builder: ApplicationBuilder<P>) -> Self {
        Self {
            window_attributes: builder.window_attributes,
            gfx_config: builder.gfx_config,
            proxy: builder.proxy,
            control_flow: builder.control_flow,
            ctx: None,
            frame_clock: Clock::new(),
            frame_time: Duration::from_millis(1000 / builder.frame_rate),
            engine_error: None,
        }
    }

    fn init(&mut self, event_loop: &ActiveEventLoop) -> EngineResult<()> {
        let window_visible= self.window_attributes.visible;

        let window_attributes = 
            self.window_attributes.clone()
            .with_visible(false)
        ;
        let window_api = WindowApi::new(event_loop, window_attributes)?;
        let gfx = GraphicsContext::new(window_api.window.clone(), self.gfx_config)?;
        let render_ctx = Rc::new(RefCell::new(RenderContext::new(gfx)));

        let mut assets_manager = AssetsManager::new(render_ctx.clone());
        Self::init_assets_manager(&mut assets_manager);

        window_api.window.set_visible(window_visible);
        self.ctx = Some(ApplicationContext {
            window_api,
            render_ctx,
            assets_manager,
        });

        self.proxy.on_init(self.ctx.as_mut().unwrap())?;

        Ok(())
    }


    fn init_assets_manager(assets_manager: &mut AssetsManager) {
        assets_manager.register_loader(ShaderLoader);
        assets_manager.register_loader(SamplerLoader);
        assets_manager.register_loader(TextureLoader);
    }
}