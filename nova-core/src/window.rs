use std::sync::Arc;

use winit::{event_loop::ActiveEventLoop, window::{Window, WindowAttributes}};

use crate::EngineResult;

mod reexports {
    pub use winit::dpi::LogicalSize;
    pub use winit::dpi::PhysicalSize;
}

pub use reexports::*;

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

    pub fn size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        let logical = size.to_logical(self.window.scale_factor());
        (logical.width, logical.height)
    }
}