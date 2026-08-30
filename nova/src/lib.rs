use nova_2d::plugin::Nova2DPlugin;
use nova_core::plugin::{CorePlugin, PluginsGroup};

pub mod core {
    pub use nova_core::*;
}

pub mod nova2d {
    pub use nova_2d::*;
}

pub struct DefaultPlugins;

impl PluginsGroup for DefaultPlugins {
    fn add_plugins(&self, plugins: &mut nova_core::plugin::Plugins) {
        plugins.add_plugin(CorePlugin);
        plugins.add_plugin(Nova2DPlugin);
    }
}