use nova_core::{assets::defaults::CoreDefaultAssets, graphics::{material::{Material, MaterialTemplate}, shader::Shader}, plugin::Plugin};

use crate::defaults::{Nova2dDefaults, default_material, default_material_template, default_shader};

pub struct Nova2DPlugin;

impl Plugin for Nova2DPlugin {
    fn init(&self, ctx: &mut nova_core::app::ApplicationContext) -> nova_core::EngineResult<()> {
        let shader = ctx.assets_manager.load::<Shader>(default_shader())?;
        let template = ctx
            .assets_manager
            .load::<MaterialTemplate>(
                default_material_template(shader)
            )?;
        let white_texture = ctx.default_assets.expect(CoreDefaultAssets::WhiteTexture);
        
        let material = ctx.assets_manager.load::<Material>(default_material(template, white_texture))?;
        
        ctx.default_assets.insert(Nova2dDefaults::TexturedQuadShader, shader)?;
        ctx.default_assets.insert(Nova2dDefaults::TexturedQuadMaterialTemplate, template)?;
        ctx.default_assets.insert(Nova2dDefaults::WhiteTextureMaterial, material)?;

        Ok(())
    }
}