
use crate::{app::{Application, ApplicationProxy, framerate::FramerateLimit}, graphics::config::GraphicsConfiguration, plugin::{Plugin, Plugins, PluginsGroup}, window::{ControlFlow, WindowConfig}};

pub struct ApplicationBuilder<P: ApplicationProxy> {
    pub(crate) window_config: WindowConfig,
    pub(crate) gfx_config: GraphicsConfiguration,
    pub(crate) control_flow: ControlFlow,
    pub(crate) proxy: P,
    pub(crate) framerate_limit: FramerateLimit,
    pub(crate) plugins: Plugins,
}

impl<P: ApplicationProxy> ApplicationBuilder<P> {
    pub fn new(proxy: P) -> Self {
        Self {
            window_config: WindowConfig::default(),
            gfx_config: GraphicsConfiguration::default(),
            control_flow: ControlFlow::Poll,
            proxy,
            framerate_limit: FramerateLimit::Auto,
            plugins: Plugins::new(),
        }
    }

    pub fn alter_window_attributes(mut self, alter_func: impl FnOnce(WindowConfig) -> WindowConfig) -> Self {
        self.window_config = alter_func(self.window_config);
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

    pub fn with_frame_rate_limit(mut self, framerate_limit: FramerateLimit) -> Self {
        self.framerate_limit = framerate_limit;
        self
    }

    pub fn with_plugin(mut self, plugin: impl Plugin) -> Self {
        self.plugins.add_plugin(plugin);
        self
    }

    pub fn with_plugins(mut self, plugins: impl PluginsGroup) -> Self {
        plugins.add_plugins(&mut self.plugins);
        self
    }

    pub fn build(self) -> Application<P> {
        Application::from_builder(self)
    }   
}