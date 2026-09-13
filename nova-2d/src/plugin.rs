use nova_core::plugin::Plugin;

use crate::{
    batcher::set_quad_geometry,
    defaults::{default_color_material, default_sprite_material, default_white_texture, Nova2dDefaults},
    materials::{CircleMaterial, ColorMaterial, SpriteMaterial},
    vertex::BaseVertex2D,
};

pub struct Nova2DPlugin;

impl Plugin for Nova2DPlugin {
    fn init(&self, ctx: &mut nova_core::app::ApplicationContext) -> nova_core::EngineResult<()> {
        // Register material types so the renderer can resolve them at draw time.
        ctx.render_ctx.register_material::<ColorMaterial>();
        ctx.render_ctx.register_material::<SpriteMaterial>();
        ctx.render_ctx.register_material::<CircleMaterial>();

        // Create and insert default assets.
        let white_texture = ctx.assets_manager.insert_asset(default_white_texture());
        let color_material = ctx.assets_manager.insert_asset(default_color_material());
        let sprite_material = ctx.assets_manager.insert_asset(default_sprite_material(white_texture));
        let circle_material = ctx.assets_manager.insert_asset(CircleMaterial::new());

        ctx.default_assets.insert(Nova2dDefaults::WhiteTexture, white_texture)?;
        ctx.default_assets.insert(Nova2dDefaults::DefaultColorMaterial, color_material)?;
        ctx.default_assets.insert(Nova2dDefaults::DefaultSpriteMaterial, sprite_material)?;
        ctx.default_assets.insert(Nova2dDefaults::DefaultCircleMaterial, circle_material)?;

        // Register the shared base quad geometry (4 vertices + 6 indices).
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

        set_quad_geometry(geo_ref);

        Ok(())
    }
}