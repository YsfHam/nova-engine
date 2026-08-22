use std::time::Duration;

use winit::{event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::WindowAttributes};

mod handler;
mod builder;

pub use builder::ApplicationBuilder;

use crate::{EngineResult, errors::EngineError, graphics::context::GraphicsContext, time::Clock, window::WindowApi};

pub struct ApplicationContext {
    window_api: WindowApi,
    gfx: GraphicsContext
}

impl ApplicationContext {
    fn request_window_redraw(&self) {
        self.window_api.window.request_redraw();
    }
}

pub trait ApplicationProxy {
    fn on_update(&mut self, ctx: &ApplicationContext, dt: Duration);
}

pub struct Application<P: ApplicationProxy> {
    window_attributes: WindowAttributes,
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

        let gfx = pollster::block_on(GraphicsContext::new(window_api.window.clone()))?;


        window_api.window.set_visible(window_visible);

        self.ctx = Some(ApplicationContext {
            window_api,
            gfx,
        });

        Ok(())
    }
}