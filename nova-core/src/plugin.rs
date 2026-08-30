use crate::{EngineResult, app::ApplicationContext, assets::defaults::CoreDefaultAssets, graphics::{sampler::{Sampler, SamplerMetadata}, texture::{Texture, TextureMetadata, TextureSize}}};

pub trait Plugin: 'static {
    fn init(&self, ctx: &mut ApplicationContext) -> EngineResult<()>;
}

pub trait PluginsGroup {
    fn add_plugins(&self, plugins: &mut Plugins);
}

pub struct Plugins {
    plugins: Vec<Box<dyn Plugin>>,
}

impl Plugins {
    pub(crate) fn new() -> Self {
        Self {
            plugins: Vec::new()
        }
    }

    pub fn add_plugin(&mut self, plugin: impl Plugin) {
        self.plugins.push(Box::new(plugin));
    }

    pub(crate) fn init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()> {
        for plugin in &self.plugins {
            plugin.init(ctx)?;
        }
        self.plugins.clear();
        Ok(())
    }
}

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn init(&self, ctx: &mut ApplicationContext) -> EngineResult<()> {
        let default_assets = &mut ctx.default_assets;
        let assets_manager = &mut ctx.assets_manager;

        let sampler = assets_manager.load::<Sampler>(SamplerMetadata::default())?;
        default_assets.insert(CoreDefaultAssets::DefaultSampler, sampler)?;

        let texture = assets_manager.load::<Texture>(TextureMetadata::from_raw(
            "White texture",
            vec![0xFF, 0xFF, 0xFF, 0xFF],
            TextureSize::new_texture2d(1, 1),
            sampler
        ))?;

        default_assets.insert(CoreDefaultAssets::WhiteTexture, texture)?;

        Ok(())
    }
}

