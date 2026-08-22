use std::sync::Arc;

use winit::{event_loop::ActiveEventLoop, window::{Window, WindowAttributes}};

use crate::EngineResult;

pub struct WindowApi {
    pub(crate) window: Arc<Window>,
}

impl WindowApi {
    pub(crate) fn new(event_loop: &ActiveEventLoop, window_attributes: WindowAttributes) -> EngineResult<Self> {
        let window = event_loop.create_window(window_attributes.clone())?;

        Ok(Self {
            window: Arc::new(window),
        })
    }
}