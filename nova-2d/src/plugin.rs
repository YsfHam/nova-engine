use nova_core::{assets::defaults::CoreDefaultAssets, graphics::{material::{Material, MaterialTemplate}, sampler::Sampler, shader::Shader}, plugin::Plugin};

use crate::{batcher::set_quad_geometry, defaults::pixelated_sampler};
use crate::defaults::{Nova2dDefaults, default_material, default_material_template, default_shader};
use crate::vertex::BaseVertex2D;

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

        let sampler = ctx.assets_manager.load::<Sampler>(pixelated_sampler())?;
        
        ctx.default_assets.insert(Nova2dDefaults::TexturedQuadShader, shader)?;
        ctx.default_assets.insert(Nova2dDefaults::TexturedQuadMaterialTemplate, template)?;
        ctx.default_assets.insert(Nova2dDefaults::WhiteTextureMaterial, material)?;
        ctx.default_assets.insert(Nova2dDefaults::PixelatedSampler, sampler)?;

        // Register the shared base quad geometry (4 vertices + 6 indices).
        // This is uploaded once to the persistent geometry buffer and reused
        // by all instanced quad batches — zero per-frame vertex upload.
        let base_vertices: [BaseVertex2D; 4] = [
            BaseVertex2D { position: [-0.5, -0.5] }, // TL
            BaseVertex2D { position: [-0.5,  0.5] }, // BL
            BaseVertex2D { position: [ 0.5,  0.5] }, // BR
            BaseVertex2D { position: [ 0.5, -0.5] }, // TR
        ];
        let base_indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let geo_ref = ctx.render_ctx.insert_geometry(
            bytemuck::cast_slice(&base_vertices),
            &base_indices,
        );

        // Set the global quad geometry reference so Batcher2D can use it.
        set_quad_geometry(geo_ref);

        Ok(())
    }
}