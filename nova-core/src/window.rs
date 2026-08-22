use winit::{event_loop::ActiveEventLoop, window::{Window, WindowAttributes}};

pub struct WindowApi {
    pub(crate) window: Window,
}

impl WindowApi {
    pub(crate) fn new(event_loop: &ActiveEventLoop, window_attributes: WindowAttributes) -> Self {
        let window = event_loop.create_window(window_attributes.clone()).unwrap();

        Self {
            window,
        }
    }
}