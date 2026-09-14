use std::sync::Arc;

use winit::{dpi::LogicalSize, event_loop::ActiveEventLoop, window::{Window, WindowAttributes}};

use crate::EngineResult;

pub struct WindowApi {
    pub(crate) window: Arc<Window>,
}

impl WindowApi {
    pub(crate) fn new(event_loop: &ActiveEventLoop, window_config: WindowConfig) -> EngineResult<Self> {
        let window = event_loop.create_window(window_config.into())?;

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


#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ControlFlow {
    Poll,
    Wait,
}

impl From<ControlFlow> for winit::event_loop::ControlFlow {
    fn from(value: ControlFlow) -> Self {
        match value {
            ControlFlow::Poll => winit::event_loop::ControlFlow::Poll,
            ControlFlow::Wait => winit::event_loop::ControlFlow::Wait,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: String,
    pub size: (u32, u32),
    pub min_size: Option<(u32, u32)>,
    pub max_size: Option<(u32, u32)>,
    pub resizable: bool,
    pub maximized: bool,
    pub visible: bool,
    pub decoration: bool,
}

impl WindowConfig {
    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_size(mut self, size: (u32, u32)) -> Self {
        self.size = size;
        self
    }

    pub fn with_min_size(mut self, min_size: (u32, u32)) -> Self {
        self.min_size = Some(min_size);
        self
    }

    pub fn with_max_size(mut self, max_size: (u32, u32)) -> Self {
        self.max_size = Some(max_size);
        self
    }

    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn with_maximized(mut self, maximized: bool) -> Self {
        self.maximized = maximized;
        self
    }

    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn with_decoration(mut self, decoration: bool) -> Self {
        self.decoration = decoration;
        self
    }
}


impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "New Window".into(),
            size: (500, 400),
            min_size: None,
            max_size: None,
            resizable: true,
            maximized: false,
            visible: true,
            decoration: true,
        }
    }
}

impl From<WindowConfig> for WindowAttributes {
    fn from(value: WindowConfig) -> Self {
        let mut attr = WindowAttributes::default()
        .with_title(value.title)
        .with_inner_size(LogicalSize::<u32>::from(value.size));
        
        attr.min_inner_size = value.min_size.map(|(width, height)| LogicalSize::new(width, height).into());
        attr.max_inner_size = value.max_size.map(|(width, height)| LogicalSize::new(width, height).into());
        attr
        .with_resizable(value.resizable)
        .with_maximized(value.maximized)
        .with_visible(value.visible)
        .with_decorations(value.decoration)
    }
}