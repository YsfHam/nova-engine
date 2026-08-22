use winit::{event_loop::ControlFlow, window::{Window, WindowAttributes}};

use crate::app::{Application, ApplicationProxy};

pub struct ApplicationBuilder<P: ApplicationProxy> {
    pub(crate) window_attributes: WindowAttributes,
    pub(crate) control_flow: ControlFlow,
    pub(crate) proxy: P,
}

impl<P: ApplicationProxy> ApplicationBuilder<P> {
    pub fn new(proxy: P) -> Self {
        Self {
            window_attributes: Window::default_attributes(),
            control_flow: ControlFlow::Poll,
            proxy
        }
    }

    pub fn alter_window_attributes(mut self, alter_func: impl FnOnce(WindowAttributes) -> WindowAttributes) -> Self {
        self.window_attributes = alter_func(self.window_attributes);
        self
    }

    pub fn with_control_flow(mut self, new_control_flow: ControlFlow) -> Self {
        self.control_flow = new_control_flow;
        self
    }

    pub fn build(self) -> Application<P> {
        Application::from_builder(self)
    }   
}