use winit::{event_loop::ControlFlow, window::{Window, WindowAttributes}};

use crate::{app::{Application, ApplicationProxy}, graphics::config::GraphicsConfiguration};

pub struct ApplicationBuilder<P: ApplicationProxy> {
    pub(crate) window_attributes: WindowAttributes,
    pub(crate) gfx_config: GraphicsConfiguration,
    pub(crate) control_flow: ControlFlow,
    pub(crate) proxy: P,
    pub(crate) frame_rate: u64,
}

impl<P: ApplicationProxy> ApplicationBuilder<P> {
    pub fn new(proxy: P) -> Self {
        Self {
            window_attributes: Window::default_attributes(),
            gfx_config: GraphicsConfiguration::default(),
            control_flow: ControlFlow::Poll,
            proxy,
            frame_rate: 60,
        }
    }

    pub fn alter_window_attributes(mut self, alter_func: impl FnOnce(WindowAttributes) -> WindowAttributes) -> Self {
        self.window_attributes = alter_func(self.window_attributes);
        self
    }

    pub fn alter_graphics_configuration(mut self, alter_func: impl FnOnce(GraphicsConfiguration) -> GraphicsConfiguration) -> Self {
        self.gfx_config = alter_func(self.gfx_config);
        self
    }

    pub fn with_control_flow(mut self, new_control_flow: ControlFlow) -> Self {
        self.control_flow = new_control_flow;
        self
    }

    pub fn with_frame_rate(mut self, frame_rate: u64) -> Self {
        self.frame_rate = frame_rate;
        self
    }

    pub fn build(self) -> Application<P> {
        Application::from_builder(self)
    }   
}