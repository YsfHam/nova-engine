use winit::{event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::WindowAttributes};

mod handler;
mod builder;

pub use builder::ApplicationBuilder;

use crate::window::WindowApi;

pub struct ApplicationContext {
    window_api: WindowApi,
}

impl ApplicationContext {
    fn request_window_redraw(&self) {
        self.window_api.window.request_redraw();
    }
}

pub trait ApplicationProxy {
    fn on_update(&mut self, ctx: &ApplicationContext);
}

pub struct Application<P: ApplicationProxy> {
    window_attributes: WindowAttributes,
    proxy: P,
    control_flow: ControlFlow,
    ctx: Option<ApplicationContext>,
}

impl<P: ApplicationProxy> Application<P> {
    pub fn run(mut self) {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(self.control_flow);

        event_loop.run_app(&mut self).unwrap();
    }

    fn from_builder(builder: ApplicationBuilder<P>) -> Self {
        Self {
            window_attributes: builder.window_attributes,
            proxy: builder.proxy,
            control_flow: builder.control_flow,
            ctx: None,
        }
    }

    fn init(&mut self, event_loop: &ActiveEventLoop) {
        self.ctx = Some(ApplicationContext {
            window_api: WindowApi::new(event_loop, self.window_attributes.clone())
        })
    }
}